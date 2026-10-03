//! 版本更新检测：查询 GitHub Release，判断是否需要更新。
//!
//! 仓库发布走 GitHub Release（见 AGENTS.md：`wcmusic-windows-x64.zip` /
//! `wcmusic-android-arm64.apk`），所以拿 `/releases/latest` 的最新 tag 和当前
//! 版本比较即可。查询是只读的，且要能在「仓库还没有发布任何版本」时安静地
//! 退回「无法判断」，而不是报错打扰用户。

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// 项目仓库（与「关于」页展示的地址一致）。
pub const REPOSITORY_URL: &str = "https://github.com/YMRwithNoworry/wcmusic";

/// 发布仓库：安装包（`wcmusic-windows-x64.zip` / `wcmusic-android-arm64.apk`）
/// 走 GitHub Release 放在这里，和源码仓库分开，避免大文件拖慢 clone。
pub const RELEASES_REPOSITORY_URL: &str = "https://github.com/YMRwithNoworry/wcmusic-releases";

/// GitHub 发布列表接口（取第一条即最新发布）。
///
/// 不用 `/releases/latest`：仓库还没发布过版本时它返回 404，而列表接口会正常
/// 返回 `[]`，两种情况都能走同一条「解析不出 tag 就是暂无版本」的路径。
const LATEST_RELEASE_ENDPOINT: &str =
    "https://api.github.com/repos/YMRwithNoworry/wcmusic-releases/releases?per_page=1";

/// 检测结果。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateCheck {
    /// 当前版本（`CARGO_PKG_VERSION`）。
    pub current_version: String,
    /// 远端最新版本（已去掉前缀 `v`）。接口没有发布版本时为 None。
    pub latest_version: Option<String>,
    /// 最新发布页地址，便于用户手动下载。
    pub release_url: Option<String>,
}

impl UpdateCheck {
    /// 是否确实发现了更新的版本。
    pub fn update_available(&self) -> bool {
        self.latest_version
            .as_deref()
            .is_some_and(|latest| is_newer_version(latest, &self.current_version))
    }

    /// 面向用户的提示文案。
    pub fn summary(&self) -> String {
        match &self.latest_version {
            Some(latest) if self.update_available() => {
                format!("发现新版本 {latest}（当前 {}）", self.current_version)
            }
            Some(_) => "已是最新版本".to_owned(),
            None => "暂未发布版本".to_owned(),
        }
    }
}

/// 查询最新发布。任何网络/解析问题都收敛成 `None`，调用方按「暂时无法判断」处理。
pub fn check_latest_release(current_version: &str, use_proxy: bool) -> UpdateCheck {
    let mut result = UpdateCheck {
        current_version: current_version.to_owned(),
        latest_version: None,
        release_url: None,
    };
    let Ok(body) = fetch_latest_release(use_proxy) else {
        return result;
    };
    result.latest_version = parse_latest_tag(&body).map(|tag| normalize_version(&tag));
    result.release_url = parse_release_url(&body);
    result
}

fn fetch_latest_release(use_proxy: bool) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .timeout_write(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let body = agent
        .get(LATEST_RELEASE_ENDPOINT)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "WCMusic/1.0")
        .call()
        .map_err(|error| error.to_string())?
        .into_string()
        .map_err(|error| error.to_string())?;
    Ok(body)
}

/// 从接口返回里取出最新发布（列表中第一个可用的正式发布，或单个发布对象）。
///
/// 跳过草稿与预发布：它们不该提示普通用户升级。
fn parse_latest_release(body: &str) -> Option<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_str(body.trim_start_matches('\u{feff}')).ok()?;
    match value {
        serde_json::Value::Array(entries) => entries.into_iter().find(is_published_release),
        // 兼容单个发布对象（例如 /releases/latest 的返回）。
        object @ serde_json::Value::Object(_) => Some(object),
        _ => None,
    }
}

/// 是否是可以提示用户升级的正式发布。
fn is_published_release(release: &serde_json::Value) -> bool {
    let flag = |key: &str| {
        release
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    !flag("draft") && !flag("prerelease")
}

/// 取发布对象里的 `tag_name`。
fn parse_latest_tag(body: &str) -> Option<String> {
    parse_latest_release(body)?
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

/// 取发布对象里的发布页地址；没有就退回仓库地址。
fn parse_release_url(body: &str) -> Option<String> {
    let release = parse_latest_release(body)?;
    match release.get("html_url").and_then(serde_json::Value::as_str) {
        Some(url) => Some(url.to_owned()),
        None => Some(RELEASES_REPOSITORY_URL.to_owned()),
    }
}

/// 去掉发布标签的常见前缀（`v1.2.3` / `release-1.2.3`）与空白。
fn normalize_version(tag: &str) -> String {
    let tag = tag.trim();
    tag.strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag)
        .trim()
        .to_owned()
}

/// 判断 `candidate` 是否比 `current` 新。
///
/// 只按点分数字逐段比较，缺失的段按 0 处理，因此 `1.2` 与 `1.2.0` 等价；
/// 解析不出版本号的标签（例如带 `-beta` 后缀或纯日期）一律视为「不比当前新」，
/// 避免误报提示用户升级。
pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    let (Some(candidate), Some(current)) = (parse_version(candidate), parse_version(current)) else {
        return false;
    };
    let length = candidate.len().max(current.len());
    for index in 0..length {
        let left = candidate.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

/// 把 `1.2.3` 解析成数字段；出现非数字内容（预发布后缀等）就返回 None。
fn parse_version(value: &str) -> Option<Vec<u64>> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value
        .split('.')
        .map(|part| part.trim().parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_by_numeric_segments() {
        assert!(is_newer_version("1.2.3", "1.2.2"));
        assert!(is_newer_version("1.3.0", "1.2.9"));
        assert!(is_newer_version("2.0.0", "1.99.99"));
        assert!(!is_newer_version("1.2.2", "1.2.2"));
        assert!(!is_newer_version("1.2.1", "1.2.2"));
    }

    #[test]
    fn treats_same_version_with_extra_zero_segments_as_equal() {
        // 1.2 与 1.2.0 是同一个版本，不能提示更新。
        assert!(!is_newer_version("1.2", "1.2.0"));
        assert!(!is_newer_version("1.2.0", "1.2"));
        // 但真正更大的段仍然算更新。
        assert!(is_newer_version("1.2.1", "1.2"));
    }

    #[test]
    fn ignores_unparsable_tags() {
        // 预发布 / 日期标签解析不出数字段，不能误报。
        assert!(!is_newer_version("1.3.0-beta.1", "1.2.2"));
        assert!(!is_newer_version("nightly", "1.2.2"));
        assert!(!is_newer_version("", "1.2.2"));
    }

    #[test]
    fn normalizes_release_tags() {
        assert_eq!(normalize_version("v1.2.3"), "1.2.3");
        assert_eq!(normalize_version(" V1.2.3 "), "1.2.3");
        assert_eq!(normalize_version("1.2.3"), "1.2.3");
    }

    #[test]
    fn parses_the_latest_release_payload() {
        // 列表接口：取第一条（最新）。
        let body = r#"[{"tag_name":"v1.3.0","html_url":"https://github.com/YMRwithNoworry/wcmusic/releases/tag/v1.3.0"},
                       {"tag_name":"v1.2.0","html_url":"https://github.com/YMRwithNoworry/wcmusic/releases/tag/v1.2.0"}]"#;
        let tag = parse_latest_tag(body).unwrap();
        assert_eq!(normalize_version(&tag), "1.3.0");
        assert!(parse_release_url(body).unwrap().ends_with("v1.3.0"));

        // 单个发布对象（/releases/latest 的形状）同样能解析。
        let single = r#"{"tag_name":"v1.4.0","html_url":"https://example.com/v1.4.0"}"#;
        assert_eq!(normalize_version(&parse_latest_tag(single).unwrap()), "1.4.0");
    }

    #[test]
    fn skips_drafts_and_prereleases() {
        // 最新的两条是草稿/预发布：要跳到后面那条正式发布。
        let body = r#"[{"tag_name":"v9.9.9","draft":true},
                       {"tag_name":"v9.0.0-beta.1","prerelease":true},
                       {"tag_name":"v1.3.0","html_url":"https://example.com/v1.3.0"}]"#;
        assert_eq!(normalize_version(&parse_latest_tag(body).unwrap()), "1.3.0");
        // 全是草稿时视为没有可提示的版本。
        assert!(parse_latest_tag(r#"[{"tag_name":"v9.9.9","draft":true}]"#).is_none());
    }

    #[test]
    fn accepts_tags_without_a_v_prefix() {
        // GitHub 上不少项目直接用纯版本号作 tag。
        let body = r#"[{"tag_name":"1.3.0","html_url":"https://example.com/1.3.0"}]"#;
        assert_eq!(normalize_version(&parse_latest_tag(body).unwrap()), "1.3.0");
    }

    #[test]
    fn empty_release_list_means_no_published_version() {
        // 仓库还没有发布过任何版本：列表为空，不能当作「有更新」。
        assert!(parse_latest_tag("[]").is_none());
        assert!(parse_release_url("[]").is_none());
    }

    #[test]
    fn reports_no_release_when_the_repository_has_none() {
        // 仓库还没有任何 Release 时接口返回 404，这里对应「解析不出 tag」。
        let result = UpdateCheck {
            current_version: "1.2.2".to_owned(),
            latest_version: None,
            release_url: None,
        };
        assert!(!result.update_available());
        assert_eq!(result.summary(), "暂未发布版本");
    }

    #[test]
    fn summarizes_available_and_current_states() {
        let outdated = UpdateCheck {
            current_version: "1.2.2".to_owned(),
            latest_version: Some("1.3.0".to_owned()),
            release_url: Some(RELEASES_REPOSITORY_URL.to_owned()),
        };
        assert!(outdated.update_available());
        assert_eq!(outdated.summary(), "发现新版本 1.3.0（当前 1.2.2）");

        let up_to_date = UpdateCheck {
            current_version: "1.2.2".to_owned(),
            latest_version: Some("1.2.2".to_owned()),
            release_url: None,
        };
        assert!(!up_to_date.update_available());
        assert_eq!(up_to_date.summary(), "已是最新版本");
    }
}
