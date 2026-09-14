//! Restricted access to an explicitly paired, per-workspace loopback browser bridge.
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::workspace::{Workspace, WorkspaceError};

const MAX_RESPONSE_BYTES: usize = 3 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
pub const METHODS: &[&str] = &["browser_list_tabs", "browser_read_page", "browser_screenshot", "browser_open"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BridgeConfig {
    version: u32,
    workspace: String,
    port: u16,
    token: String,
}

fn error(code: &'static str, message: impl Into<String>) -> WorkspaceError {
    WorkspaceError::Tool {
        code,
        message: message.into(),
        category: "browser",
        retryable: matches!(code, "BROWSER_BUSY" | "BROWSER_NOT_CONNECTED" | "BROWSER_UNAVAILABLE"),
    }
}

fn normalized_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let text = text.strip_prefix(r"\\?\UNC\").map(|p| format!("//{p}"))
        .unwrap_or_else(|| text.strip_prefix(r"\\?\").unwrap_or(&text).to_string());
    text.replace('\\', "/")
}

fn config_path(home: &Path, root: &Path) -> PathBuf {
    let key = format!("{:x}", Sha256::digest(normalized_path(root).as_bytes()));
    home.join(".coding-tools-mcp").join("browser").join(format!("{key}.json"))
}

fn validate_args(method: &str, args: &Value) -> Result<(), WorkspaceError> {
    if !METHODS.contains(&method) {
        return Err(WorkspaceError::invalid_argument("Unknown browser tool"));
    }
    let object = args.as_object().ok_or_else(|| WorkspaceError::invalid_argument("arguments must be an object"))?;
    let allowed: &[&str] = match method {
        "browser_open" => &["url"],
        "browser_list_tabs" => &["cursor", "limit"],
        "browser_read_page" => &["tab_id", "cursor", "max_chars"],
        _ => &["tab_id"],
    };
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(WorkspaceError::invalid_argument(format!("Unknown browser argument: {key}")));
        }
    }
    if method == "browser_open" {
        let raw = object.get("url").and_then(Value::as_str)
            .ok_or_else(|| WorkspaceError::invalid_argument("url is required"))?;
        let lower = raw.to_ascii_lowercase();
        if raw.len() > 2048 || raw.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\')
            || !(lower.starts_with("http://") || lower.starts_with("https://")) {
            return Err(WorkspaceError::invalid_argument("Invalid HTTP/HTTPS URL"));
        }
        let url = reqwest::Url::parse(raw).map_err(|_| WorkspaceError::invalid_argument("Invalid URL"))?;
        if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
            return Err(WorkspaceError::invalid_argument("URL credentials are not allowed"));
        }
    }
    if let Some(id) = object.get("tab_id") {
        let id = id.as_str().filter(|v| !v.is_empty() && v.len() <= 80 && v.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'));
        if id.is_none() { return Err(WorkspaceError::invalid_argument("Invalid tab_id")); }
    }
    for (key, min, max) in [("cursor", 0, 10_000_000), ("limit", 1, 100), ("max_chars", 1, 32_000)] {
        if let Some(value) = object.get(key) {
            if !value.as_u64().is_some_and(|n| n >= min && n <= max) {
                return Err(WorkspaceError::invalid_argument(format!("Invalid {key}")));
            }
        }
    }
    Ok(())
}

pub fn call(ws: &Workspace, method: &str, args: &Value) -> Result<Value, WorkspaceError> {
    validate_args(method, args)?;
    let home = dirs::home_dir().ok_or_else(|| error("BROWSER_NOT_CONNECTED", "Cannot locate local browser configuration"))?;
    let path = config_path(&home, ws.root());
    let metadata = std::fs::symlink_metadata(&path).map_err(|_| error("BROWSER_NOT_CONNECTED", "Start the local browser bridge with --workspace set to this workspace, then select a tab in Chrome. See docs/browser-bridge.md."))?;
    if !metadata.is_file() || metadata.len() > 4096 {
        return Err(error("BROWSER_CONFIG_INVALID", "Invalid local browser configuration"));
    }
    let bytes = std::fs::read(&path).map_err(|_| error("BROWSER_CONFIG_INVALID", "Cannot read browser configuration"))?;
    let config: BridgeConfig = serde_json::from_slice(&bytes).map_err(|_| error("BROWSER_CONFIG_INVALID", "Invalid browser configuration JSON"))?;
    validate_config(&config, ws.root())?;
    tauri::async_runtime::block_on(request(&config, method, args))
}

fn validate_config(config: &BridgeConfig, root: &Path) -> Result<(), WorkspaceError> {
    if config.version != 1 || config.port == 0 || config.workspace != normalized_path(root)
        || config.token.len() != 64 || !config.token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(error("BROWSER_CONFIG_INVALID", "Browser bridge is not bound to this workspace or configuration is invalid"));
    }
    Ok(())
}

async fn request(config: &BridgeConfig, method: &str, args: &Value) -> Result<Value, WorkspaceError> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(25))
        .build().map_err(|_| error("BROWSER_UNAVAILABLE", "Cannot create local browser client"))?;
    // Caller cannot change the bridge endpoint, token, or executable.
    let url = format!("http://127.0.0.1:{}/call", config.port);
    let mut response = client.post(url).bearer_auth(&config.token)
        .json(&json!({ "method": method, "arguments": args }))
        .send().await.map_err(|e| error(if e.is_timeout() { "BROWSER_TIMEOUT" } else { "BROWSER_NOT_CONNECTED" }, "Local browser bridge unavailable. Start/reconnect it locally."))?;
    if response.status().is_redirection() {
        return Err(error("BROWSER_CONFIG_INVALID", "Browser bridge redirects are not allowed"));
    }
    if response.content_length().is_some_and(|n| n > MAX_RESPONSE_BYTES as u64) {
        return Err(error("OUTPUT_TOO_LARGE", "Browser response exceeds 3 MiB"));
    }
    let success = response.status().is_success();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| error("BROWSER_UNAVAILABLE", "Cannot read browser response"))? {
        if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES { return Err(error("OUTPUT_TOO_LARGE", "Browser response exceeds 3 MiB")); }
        bytes.extend_from_slice(&chunk);
    }
    let result: Value = serde_json::from_slice(&bytes).map_err(|_| error("BRIDGE_PROTOCOL_ERROR", "Invalid browser response"))?;
    if result.get("ok") == Some(&Value::Bool(false)) {
        let code = match result["error"]["code"].as_str().unwrap_or("") {
            "NAVIGATION_FAILED" => "NAVIGATION_FAILED",
            "TAB_NOT_FOUND" => "TAB_NOT_FOUND", "UNSUPPORTED_PAGE" => "UNSUPPORTED_PAGE",
            "PAGE_CHANGED" => "PAGE_CHANGED", "OUTPUT_TOO_LARGE" => "OUTPUT_TOO_LARGE",
            "BROWSER_TIMEOUT" => "BROWSER_TIMEOUT", "BROWSER_BUSY" => "BROWSER_BUSY",
            "INVALID_ARGUMENT" => "INVALID_ARGUMENT", "FORBIDDEN" => "BROWSER_CONFIG_INVALID",
            _ => "BROWSER_UNAVAILABLE",
        };
        return Err(error(code, result["error"]["message"].as_str().unwrap_or("Browser read failed")));
    }
    if !success || result.get("ok") != Some(&Value::Bool(true)) {
        return Err(error("BRIDGE_PROTOCOL_ERROR", "Browser bridge rejected the request"));
    }
    if method == "browser_screenshot" {
        let data = result["base64"].as_str().ok_or_else(|| error("BRIDGE_PROTOCOL_ERROR", "Screenshot data missing"))?;
        let decoded = STANDARD.decode(data).map_err(|_| error("BRIDGE_PROTOCOL_ERROR", "Invalid screenshot encoding"))?;
        if decoded.len() > MAX_IMAGE_BYTES { return Err(error("OUTPUT_TOO_LARGE", "Screenshot exceeds 2 MiB")); }
        if result["mime_type"] != "image/jpeg" || !decoded.starts_with(&[0xff, 0xd8, 0xff]) {
            return Err(error("BRIDGE_PROTOCOL_ERROR", "Expected a JPEG screenshot"));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_arguments_are_strict_and_bounded() {
        for method in METHODS { assert_eq!(validate_args(method, &json!({})).is_ok(), *method != "browser_open"); }
        for args in [json!({"url":"https://example.com"}), json!({"code":"evil"}), json!({"cursor":-1}), json!({"max_chars":32001}), json!({"tab_id":"x'; evil"}), json!({"cursor":1.5}), Value::Null] {
            assert!(validate_args("browser_read_page", &args).is_err());
        }
        assert!(validate_args("browser_list_tabs", &json!({"limit":101})).is_err());
        assert!(validate_args("browser_screenshot", &json!({"path":"out.jpg"})).is_err());
    }
    #[test]
    fn local_config_is_workspace_bound() {
        let root = Path::new("/sample/workspace");
        let mut config = BridgeConfig { version:1, workspace:normalized_path(root), port:1234, token:"a".repeat(64) };
        assert!(validate_config(&config, root).is_ok());
        assert!(validate_config(&config, Path::new("/other")).is_err());
        config.token = "bad\nheader".into();
        assert!(validate_config(&config, root).is_err());
        assert_ne!(config_path(Path::new("/home"), root), config_path(Path::new("/home"), Path::new("/other")));
    }
}

/// Local control-plane probe. It never reads page content or changes the selected tab.
pub(crate) fn bridge_health(root: &Path, clear_stale: bool) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("无法找到用户目录")?;
    let path = config_path(&home, root);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok("disconnected".into()),
        Err(_) => return Err("无法读取浏览器连接配置".into()),
    };
    if !metadata.is_file() || metadata.len() > 4096 { return Err("浏览器连接配置无效".into()); }
    let bytes = std::fs::read(&path).map_err(|_| "无法读取浏览器连接配置")?;
    let config: BridgeConfig = serde_json::from_slice(&bytes).map_err(|_| "浏览器连接配置无效")?;
    validate_config(&config, root).map_err(|_| "浏览器连接配置与工作区不匹配")?;
    tauri::async_runtime::block_on(async {
        let client = reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(3)).build().map_err(|_| "无法创建连接检查")?;
        match client.get(format!("http://127.0.0.1:{}/health", config.port)).bearer_auth(&config.token).send().await {
            Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => Ok("legacy".into()),
            Ok(response) if response.status().is_success() => {
                if response.content_length().is_some_and(|n| n > 4096) { return Err("浏览器健康响应过大".into()); }
                let mut response = response; let mut bytes=Vec::new();
                while let Some(chunk)=response.chunk().await.map_err(|_| "连接检查失败")? {
                    if bytes.len()+chunk.len()>4096 { return Err("浏览器健康响应过大".into()); }
                    bytes.extend_from_slice(&chunk);
                }
                let body: Value = serde_json::from_slice(&bytes).map_err(|_| "浏览器健康响应无效")?;
                if body["service"] != "coding-tools-browser" || body["ok"] != true { return Err("浏览器连接不可用，请停止后重连".into()); }
                Ok("connected".into())
            }
            Ok(_) => Err("浏览器连接检查失败，请停止后重连".into()),
            Err(error) => {
                // Only a refused TCP connection proves this local port has no listener.
                let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&error);
                let mut refused = false;
                while let Some(e) = source {
                    if let Some(io) = e.downcast_ref::<std::io::Error>() { refused |= io.kind() == std::io::ErrorKind::ConnectionRefused; }
                    source=e.source();
                }
                if refused && clear_stale && std::fs::read(&path).ok().as_ref() == Some(&bytes) {
                    std::fs::remove_file(&path).map_err(|_| "无法清理失效连接配置")?;
                    return Ok("disconnected".into());
                }
                if refused { Ok("stale".into()) } else { Err("浏览器连接超时或不可达；未修改原连接".into()) }
            }
        }
    })
}
