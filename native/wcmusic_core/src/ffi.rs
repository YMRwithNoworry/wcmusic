use std::{ffi::CString, os::raw::c_char, slice, str};

use serde_json::json;

use crate::{SourceEnvironment, resolve_source_url, validate_source_script};

static VERSION: &[u8] = b"0.1.0\0";

#[unsafe(no_mangle)]
pub extern "C" fn wcmusic_core_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wcmusic_validate_source(
    script: *const u8,
    length: usize,
    mobile: bool,
) -> *mut c_char {
    let response = if script.is_null() {
        json!({"ok": false, "error": "脚本指针为空"})
    } else {
        // SAFETY: The caller promises that script points to `length` readable bytes for this call.
        let bytes = unsafe { slice::from_raw_parts(script, length) };
        match str::from_utf8(bytes) {
            Ok(script) => match validate_source_script(
                script,
                if mobile {
                    SourceEnvironment::Mobile
                } else {
                    SourceEnvironment::Desktop
                },
            ) {
                Ok(manifest) => json!({"ok": true, "data": manifest}),
                Err(error) => json!({"ok": false, "error": error.to_string()}),
            },
            Err(_) => json!({"ok": false, "error": "音源脚本必须使用 UTF-8 编码"}),
        }
    };
    let encoded = response.to_string().replace('\0', "");
    CString::new(encoded)
        .expect("JSON does not contain null bytes")
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wcmusic_resolve_source_url(
    script: *const u8,
    script_length: usize,
    source: *const u8,
    source_length: usize,
    song_id: *const u8,
    song_id_length: usize,
    quality: *const u8,
    quality_length: usize,
    mobile: bool,
) -> *mut c_char {
    let decode = |pointer: *const u8, length: usize| -> Result<&str, &str> {
        if pointer.is_null() {
            return Err("参数指针为空");
        }
        // SAFETY: The caller promises that each pointer has `length` readable bytes.
        str::from_utf8(unsafe { slice::from_raw_parts(pointer, length) })
            .map_err(|_| "参数必须使用 UTF-8 编码")
    };
    let response = match (
        decode(script, script_length),
        decode(source, source_length),
        decode(song_id, song_id_length),
        decode(quality, quality_length),
    ) {
        (Ok(script), Ok(source), Ok(song_id), Ok(quality)) => match resolve_source_url(
            script,
            if mobile {
                SourceEnvironment::Mobile
            } else {
                SourceEnvironment::Desktop
            },
            source,
            song_id,
            quality,
        ) {
            Ok(url) => json!({"ok": true, "data": {"url": url}}),
            Err(error) => json!({"ok": false, "error": error.to_string()}),
        },
        values => json!({
            "ok": false,
            "error": values.0.err().or(values.1.err()).or(values.2.err()).or(values.3.err())
                .unwrap_or("无法读取参数")
        }),
    };
    CString::new(response.to_string().replace('\0', ""))
        .expect("JSON does not contain null bytes")
        .into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn wcmusic_string_free(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: The pointer must have been returned by CString::into_raw in this library.
        drop(unsafe { CString::from_raw(value) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_a_stable_version_string() {
        let version = unsafe { std::ffi::CStr::from_ptr(wcmusic_core_version()) };
        assert_eq!(version.to_str().unwrap(), "0.1.0");
    }
}
