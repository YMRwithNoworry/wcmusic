use std::collections::BTreeMap;

use regex::Regex;
use rquickjs::{Context, Runtime};
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

    let metadata_json = serde_json::to_string(&metadata)?;
    let script_json = serde_json::to_string(script)?;
    let bootstrap = format!(
        r#"
        globalThis.__wcmusicHandlers = Object.create(null);
        globalThis.__wcmusicMessages = [];
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
            const error = new Error('network requests are disabled during validation');
            if (typeof callback === 'function') {{
              Promise.resolve().then(() => callback(error, {{
                statusCode: 0, body: null, headers: {{}}
              }}));
              return;
            }}
            return Promise.reject(error);
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
    );

    context.with(|ctx| {
        ctx.eval::<(), _>(bootstrap)
            .and_then(|_| ctx.eval::<(), _>(script))
            .map_err(|error| CoreError::SourceInitialization(error.to_string()))
    })?;

    for _ in 0..MAX_INITIALIZATION_JOBS {
        match runtime.execute_pending_job() {
            Ok(true) => {}
            Ok(false) => break,
            Err(error) => {
                return Err(CoreError::SourceInitialization(error.to_string()));
            }
        }
    }
    if runtime.is_job_pending() {
        return Err(CoreError::SourceInitialization(
            "脚本异步初始化任务过多".into(),
        ));
    }

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

#[derive(Deserialize)]
struct RuntimeMessage {
    name: String,
    data: serde_json::Value,
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
}
