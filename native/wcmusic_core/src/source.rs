use std::collections::BTreeMap;
use std::time::Duration;

use regex::Regex;
use rquickjs::{Context, Runtime, function::Func};
use serde::Deserialize;

use crate::{CoreError, SourceCapability, SourceEnvironment, SourceManifest, SourceScriptMetadata};

const SCRIPT_MEMORY_LIMIT: usize = 32 * 1024 * 1024;
const MAX_INITIALIZATION_JOBS: usize = 128;

pub fn parse_script_metadata(script: &str) -> Result<SourceScriptMetadata, CoreError> {
    let header = script
        .split_once("*/")
        .map(|(header, _)| header)
        .unwrap_or(script);
    let field = |name: &str| -> Option<String> {
        let pattern = format!(r"(?m)^\s*\*?\s*@{}\s+(.+?)\s*$", regex::escape(name));
        Regex::new(&pattern)
            .ok()?
            .captures(header)?
            .get(1)
            .map(|value| value.as_str().trim().to_owned())
    };
    let name = field("name").ok_or(CoreError::MissingSourceName)?;
    Ok(SourceScriptMetadata {
        name,
        description: field("description"),
        version: field("version"),
        author: field("author"),
        homepage: field("homepage"),
    })
}

pub fn validate_source_script(
    script: &str,
    environment: SourceEnvironment,
) -> Result<SourceManifest, CoreError> {
    let metadata = parse_script_metadata(script)?;
    let runtime =
        Runtime::new().map_err(|error| CoreError::SourceInitialization(error.to_string()))?;
    runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
    runtime.set_max_stack_size(512 * 1024);
    let context = Context::full(&runtime)
        .map_err(|error| CoreError::SourceInitialization(error.to_string()))?;
    install_http_guest(&context, false)?;

    let bootstrap = source_bootstrap(script, environment, &metadata)?;

    context.with(|ctx| {
        ctx.eval::<(), _>(bootstrap)
            .map_err(|error| error.to_string())
            .and_then(|_| eval_script_with_message(&ctx, script))
            .map_err(|error| CoreError::SourceInitialization(format!("脚本执行失败：{error}")))
    })?;

    drain_jobs(&runtime, &context)?;

    context.with(|ctx| {
        let messages_json: String = ctx
            .eval("JSON.stringify(globalThis.__wcmusicMessages)")
            .map_err(|error| CoreError::SourceInitialization(error.to_string()))?;
        let messages: Vec<RuntimeMessage> = serde_json::from_str(&messages_json)?;
        let init = messages
            .into_iter()
            .find(|message| message.name == "inited")
            .ok_or_else(|| CoreError::SourceInitialization("脚本没有发送 inited 事件".into()))?;
        let init: InitPayload = serde_json::from_value(init.data)?;
        let mut sources = Vec::with_capacity(init.sources.len());
        for (key, source) in init.sources {
            if source.source_type.as_deref().unwrap_or("music") != "music" {
                return Err(CoreError::SourceInitialization(format!(
                    "音源 {key} 的类型必须为 music"
                )));
            }
            sources.push(SourceCapability {
                key,
                name: source.name.unwrap_or_else(|| "未命名音源".into()),
                actions: source.actions,
                qualities: source.qualities,
            });
        }
        Ok(SourceManifest {
            metadata,
            sources,
            requests_dev_tools: init.open_dev_tools,
        })
    })
}

pub fn resolve_source_url(
    script: &str,
    environment: SourceEnvironment,
    source: &str,
    song_id: &str,
    quality: &str,
) -> Result<String, CoreError> {
    resolve_source_url_with_proxy(script, environment, source, song_id, quality, false)
}

pub fn resolve_source_url_with_proxy(
    script: &str,
    environment: SourceEnvironment,
    source: &str,
    song_id: &str,
    quality: &str,
    use_proxy: bool,
) -> Result<String, CoreError> {
    let metadata = parse_script_metadata(script)?;
    let runtime =
        Runtime::new().map_err(|error| CoreError::SourceInitialization(error.to_string()))?;
    runtime.set_memory_limit(SCRIPT_MEMORY_LIMIT);
    runtime.set_max_stack_size(512 * 1024);
    let context = Context::full(&runtime)
        .map_err(|error| CoreError::SourceInitialization(error.to_string()))?;
    install_http_guest(&context, use_proxy)?;
    let bootstrap = source_bootstrap(script, environment, &metadata)?;
    context.with(|ctx| {
        ctx.eval::<(), _>(bootstrap)
            .map_err(|error| error.to_string())
            .and_then(|_| eval_script_with_message(&ctx, script))
            .map_err(|error| CoreError::SourceInitialization(format!("脚本执行失败：{error}")))
    })?;
    drain_jobs(&runtime, &context)?;

    let source_json = serde_json::to_string(source)?;
    let song_id_json = serde_json::to_string(song_id)?;
    let quality_json = serde_json::to_string(quality)?;
    let invoke = format!(
        r#"
        globalThis.__wcmusicResolvedUrl = null;
        globalThis.__wcmusicResolveError = null;
        (() => {{
          const handler = globalThis.__wcmusicHandlers.request;
          if (typeof handler !== 'function') throw new Error('音源没有注册 request 处理器');
          const songId = {song_id_json};
          const musicInfo = Object.freeze({{
            songmid: songId, id: songId, hash: songId,
            musicId: songId, rid: songId
          }});
          Promise.resolve(handler({{
            source: {source_json},
            action: 'musicUrl',
            info: {{ type: {quality_json}, musicInfo }}
          }})).then(
            value => {{ globalThis.__wcmusicResolvedUrl = value == null ? null : String(value); }},
            error => {{ globalThis.__wcmusicResolveError = String(error); }}
          );
        }})();
        "#
    );
    context.with(|ctx| {
        eval_invoke_with_message(&ctx, &invoke)
            .map_err(CoreError::SourceInitialization)
    })?;
    drain_jobs(&runtime, &context)?;
    context.with(|ctx| {
        let state_json: String = ctx
            .eval(
                "JSON.stringify({ url: globalThis.__wcmusicResolvedUrl, error: globalThis.__wcmusicResolveError })",
            )
            .map_err(|error| CoreError::SourceInitialization(error.to_string()))?;
        let state: ResolveState = serde_json::from_str(&state_json)?;
        if let Some(error) = state.error {
            return Err(CoreError::SourceInitialization(error));
        }
        let url = state.url.unwrap_or_default();
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(CoreError::SourceInitialization(
                "音源没有返回有效的播放地址".into(),
            ));
        }
        Ok(url)
    })
}

fn source_bootstrap(
    script: &str,
    environment: SourceEnvironment,
    metadata: &SourceScriptMetadata,
) -> Result<String, CoreError> {
    let metadata_json = serde_json::to_string(metadata)?;
    let script_json = serde_json::to_string(script)?;
    Ok(format!(
        r#"
        globalThis.__wcmusicHandlers = Object.create(null);
        globalThis.__wcmusicMessages = [];
        globalThis.setTimeout = () => 0;
        globalThis.clearTimeout = () => {{}};
        globalThis.setInterval = () => 0;
        globalThis.clearInterval = () => {{}};
        globalThis.console = Object.freeze({{
          log() {{}}, info() {{}}, warn() {{}}, error() {{}}, debug() {{}}
        }});
        globalThis.lx = Object.freeze({{
          version: '1',
          env: '{}',
          currentScriptInfo: Object.freeze(Object.assign({{}}, {}, {{ rawScript: {} }})),
          EVENT_NAMES: Object.freeze({{
            inited: 'inited', request: 'request', updateAlert: 'updateAlert'
          }}),
          on(name, handler) {{
            if (typeof handler !== 'function') throw new TypeError('handler must be a function');
            globalThis.__wcmusicHandlers[name] = handler;
          }},
          send(name, data) {{ globalThis.__wcmusicMessages.push({{ name, data }}); }},
          request(url, options, callback) {{
            if (typeof options === 'function') callback = options;
            options = options || {{}};
            const method = String(options.method || 'GET').toUpperCase();
            const headers = options.headers || {{}};
            const body = options.body == null ? null : String(options.body);
            let parsed;
            try {{
              parsed = JSON.parse(globalThis.__wcmusicFetch(
                String(url), method, JSON.stringify(headers), body
              ));
            }} catch (error) {{
              if (typeof callback === 'function') {{
                Promise.resolve().then(() => callback(error, null));
                return;
              }}
              return Promise.reject(error);
            }}
            if (!parsed.ok) {{
              const error = new Error(parsed.error || 'request failed');
              if (typeof callback === 'function') {{
                Promise.resolve().then(() => callback(error, null));
                return;
              }}
              return Promise.reject(error);
            }}
            const response = {{
              statusCode: parsed.statusCode,
              headers: parsed.headers || {{}},
              body: parsed.body
            }};
            if (typeof callback === 'function') {{
              Promise.resolve().then(() => callback(null, response));
              return;
            }}
            return Promise.resolve(response);
          }},
          utils: Object.freeze({{
            buffer: Object.freeze({{ from(value) {{ return value; }}, bufToString(value) {{ return String(value); }} }}),
            crypto: Object.freeze({{
              aesEncrypt() {{ throw new Error('aesEncrypt is unavailable during validation'); }},
              md5() {{ throw new Error('md5 is unavailable during validation'); }},
              randomBytes() {{ throw new Error('randomBytes is unavailable during validation'); }},
              rsaEncrypt() {{ throw new Error('rsaEncrypt is unavailable during validation'); }}
            }}),
            zlib: Object.freeze({{
              inflate() {{ return Promise.reject(new Error('inflate is unavailable during validation')); }},
              deflate() {{ return Promise.reject(new Error('deflate is unavailable during validation')); }}
            }})
          }})
        }});
        "#,
        environment.as_str(),
        metadata_json,
        script_json,
    ))
}

fn install_http_guest(context: &Context, use_proxy: bool) -> Result<(), CoreError> {
    context.with(|ctx| {
        let fetch = Func::new(
            move |url: String, method: String, headers_json: String, body: Option<String>| -> String {
                match http_request_json(&url, &method, &headers_json, body.as_deref(), use_proxy) {
                    Ok(value) => value,
                    Err(error) => serde_json::json!({"ok": false, "error": error}).to_string(),
                }
            },
        );
        ctx.globals()
            .set("__wcmusicFetch", fetch)
            .map_err(|error| CoreError::SourceInitialization(error.to_string()))
    })
}

fn eval_script_with_message(ctx: &rquickjs::Ctx<'_>, script: &str) -> Result<(), String> {
    let script_json = serde_json::to_string(script)
        .map_err(|error| error.to_string())?;
    let wrapped = format!(
        "(function() {{ try {{ eval({script_json}) }} catch (error) {{ \
         throw new Error(error && error.message ? error.message : String(error)); }} }})()"
    );
    ctx.eval::<(), _>(wrapped).map_err(|error| error.to_string())
}

fn eval_invoke_with_message(ctx: &rquickjs::Ctx<'_>, invoke: &str) -> Result<(), String> {
    let invoke_json = serde_json::to_string(invoke)
        .map_err(|error| error.to_string())?;
    let wrapped = format!(
        "(function() {{ try {{ eval({invoke_json}) }} catch (error) {{ \
         throw new Error(error && error.message ? error.message : String(error)); }} }})()"
    );
    ctx.eval::<(), _>(wrapped).map_err(|error| error.to_string())
}

fn http_request_json(
    url: &str,
    method: &str,
    headers_json: &str,
    body: Option<&str>,
    use_proxy: bool,
) -> Result<String, String> {
    let headers: BTreeMap<String, String> = serde_json::from_str(headers_json)
        .map_err(|error| format!("请求头格式无效：{error}"))?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .try_proxy_from_env(use_proxy)
        .build();
    let mut request = agent.request(method, url);
    for (name, value) in headers {
        request = request.set(&name, &value);
    }
    let response = match method {
        "POST" | "PUT" | "PATCH" => request
            .send_string(body.unwrap_or(""))
            .map_err(|error| format!("网络请求失败：{error}"))?,
        _ => request
            .call()
            .map_err(|error| format!("网络请求失败：{error}"))?,
    };
    let status_code = response.status();
    let content_type = response
        .header("content-type")
        .unwrap_or_default()
        .to_lowercase();
    let response_headers: BTreeMap<String, String> = response
        .headers_names()
        .iter()
        .filter_map(|name| {
            response
                .header(name)
                .map(|value| (name.to_lowercase(), value.to_string()))
        })
        .collect();
    let text = response
        .into_string()
        .map_err(|error| format!("读取响应失败：{error}"))?;
    let body_value = if content_type.contains("json") {
        serde_json::from_str(&text).unwrap_or_else(|_| serde_json::Value::String(text))
    } else {
        serde_json::Value::String(text)
    };
    Ok(
        serde_json::json!({
            "ok": true,
            "statusCode": status_code,
            "headers": response_headers,
            "body": body_value,
        })
        .to_string(),
    )
}

fn drain_jobs(runtime: &Runtime, context: &Context) -> Result<(), CoreError> {
    let mut failure: Option<String> = None;
    for _ in 0..MAX_INITIALIZATION_JOBS {
        match runtime.execute_pending_job() {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                failure = Some(error.to_string());
                break;
            }
        }
    }
    if failure.is_none() && runtime.is_job_pending() {
        failure = Some("脚本异步任务过多".into());
    }
    let Some(error) = failure else {
        return Ok(());
    };
    let detail = context.with(|ctx| {
        if ctx.has_exception() {
            ctx.catch()
                .get::<String>()
                .unwrap_or_else(|_| error.clone())
        } else {
            error
        }
    });
    Err(CoreError::SourceInitialization(format!("异步任务失败：{detail}")))
}

#[derive(Deserialize)]
struct RuntimeMessage {
    name: String,
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct ResolveState {
    url: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitPayload {
    #[serde(default)]
    open_dev_tools: bool,
    sources: BTreeMap<String, InitSource>,
}

#[derive(Deserialize)]
struct InitSource {
    name: Option<String>,
    #[serde(default, rename = "type")]
    source_type: Option<String>,
    #[serde(default)]
    actions: Vec<String>,
    #[serde(default, rename = "qualitys")]
    qualities: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
/**
 * @name 林间测试源
 * @description 只用于协议测试
 * @version 1.0.0
 * @author WCMusic
 */
const { EVENT_NAMES, on, send } = globalThis.lx
on(EVENT_NAMES.request, async () => 'https://example.com/music.flac')
send(EVENT_NAMES.inited, {
  openDevTools: false,
  sources: { kw: { name: '测试源', type: 'music', actions: ['musicUrl'], qualitys: ['320k', 'flac'] } }
})
"#;

    #[test]
    fn reads_script_header() {
        let metadata = parse_script_metadata(SOURCE).unwrap();
        assert_eq!(metadata.name, "林间测试源");
        assert_eq!(metadata.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn validates_lx_initialization_protocol() {
        let manifest = validate_source_script(SOURCE, SourceEnvironment::Desktop).unwrap();
        assert_eq!(manifest.sources[0].key, "kw");
        assert_eq!(manifest.sources[0].qualities, ["320k", "flac"]);
    }

    #[test]
    fn validates_callback_request_before_async_initialization() {
        let source = format!(
            r#"{}
new Promise((resolve) => {{
  lx.request('https://example.com/version.json', {{}}, (error) => {{
    console.error(error)
    resolve()
  }})
}}).then(() => lx.send(lx.EVENT_NAMES.inited, {{
  sources: {{ wy: {{ name: '异步测试源', actions: ['musicUrl'], qualitys: ['flac'] }} }}
}}))
"#,
            SOURCE.split_once("*/").unwrap().0.to_owned() + "*/"
        );

        let manifest = validate_source_script(&source, SourceEnvironment::Desktop).unwrap();
        assert_eq!(manifest.sources[0].key, "wy");
        assert_eq!(manifest.sources[0].qualities, ["flac"]);
    }

    #[test]
    fn resolves_a_music_url_from_the_registered_handler() {
        let url =
            resolve_source_url(SOURCE, SourceEnvironment::Desktop, "kw", "12345", "320k").unwrap();
        assert_eq!(url, "https://example.com/music.flac");
    }

    #[test]
    fn validates_bundled_source_on_mobile() {
        let script = include_str!("../../../assets/sources/paojiao_internal_source.js");
        let manifest = validate_source_script(script, SourceEnvironment::Mobile).unwrap();
        assert!(manifest.sources.len() >= 5, "bundled source lost capabilities");
    }
}
