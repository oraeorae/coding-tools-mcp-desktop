//! GPT Actions file return is a top-level protocol, separate from MCP tool envelopes.
use std::{collections::HashMap, fs::File, io::Read, sync::Arc, time::{Duration, Instant}};

use axum::{
    body::{Body, Bytes},
    extract::{Path, State},
    http::{header, StatusCode},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::tools::Workspace;
use super::auth::{require_actions_auth, AuthConfig};

const MAX_PDF_BYTES: usize = 10_000_000;
const MAX_CACHE_BYTES: usize = 32_000_000;
const MAX_CACHE_FILES: usize = 64;
const TTL: Duration = Duration::from_secs(300);

type ApiError = (StatusCode, Json<Value>);

fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({"error": message})))
}

struct PdfFile {
    bytes: Bytes,
    disposition: String,
    expires: Instant,
}

#[derive(Default)]
struct PdfCache {
    entries: HashMap<String, PdfFile>,
}

impl PdfCache {
    fn prune(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, file| file.expires > now);
    }

    fn insert(&mut self, file: PdfFile) -> Result<String, ApiError> {
        self.prune();
        let used: usize = self.entries.values().map(|v| v.bytes.len()).sum();
        if self.entries.len() >= MAX_CACHE_FILES || used + file.bytes.len() > MAX_CACHE_BYTES {
            return Err(error(StatusCode::TOO_MANY_REQUESTS, "PDF cache full; retry after existing links expire"));
        }
        let token = Uuid::new_v4().to_string();
        self.entries.insert(token.clone(), file);
        Ok(token)
    }
}

#[derive(Clone)]
struct PdfState {
    workspace: Workspace,
    base_url: String,
    cache: Arc<Mutex<PdfCache>>,
}

pub(super) fn router(workspace: Workspace, auth: Arc<AuthConfig>) -> Router {
    let base_url = if auth.configured_public_url.trim().is_empty() {
        format!("http://127.0.0.1:{}", auth.bind_port)
    } else {
        auth.configured_public_url.trim().trim_end_matches('/').to_owned()
    };
    let cache = Arc::new(Mutex::new(PdfCache::default()));
    // A weak reference lets the cleanup task stop when its listener/router is dropped.
    let weak = Arc::downgrade(&cache);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let Some(cache) = weak.upgrade() else { break };
            cache.lock().await.prune();
        }
    });
    let state = PdfState { workspace, base_url, cache };
    let protected = Router::new()
        .route("/actions/return_pdf", post(return_pdf))
        .layer(middleware::from_fn(require_actions_auth))
        .layer(Extension(auth));
    Router::new()
        .merge(protected)
        .route("/action-files/{token}", get(download_pdf))
        .with_state(state)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PdfRequest {
    path: String,
}

fn validate_base_url(raw: &str) -> Result<(), ApiError> {
    let url = reqwest::Url::parse(raw)
        .map_err(|_| error(StatusCode::SERVICE_UNAVAILABLE, "Configure a valid Actions public URL"))?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some()
        || url.query().is_some() || url.fragment().is_some()
    {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE, "Actions file URLs require HTTPS (HTTP allowed only on loopback)"));
    }
    Ok(())
}

async fn return_pdf(State(state): State<PdfState>, Json(request): Json<PdfRequest>) -> Result<Response, ApiError> {
    validate_base_url(&state.base_url)?;
    // Serialize preparation as well as insertion to bound concurrent snapshot memory.
    let mut cache = state.cache.lock().await;
    let file = tokio::task::spawn_blocking(move || read_pdf(&state.workspace, &request.path))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "PDF preparation failed"))??;
    let token = cache.insert(file)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(json!({
        "openaiFileResponse": [format!("{}/action-files/{token}", state.base_url)]
    }))).into_response())
}

fn read_pdf(workspace: &Workspace, path: &str) -> Result<PdfFile, ApiError> {
    if path.trim().is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "path is required"));
    }
    let resolved = workspace.resolve_read_path(path)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "PDF path not found or not permitted"))?;
    if !resolved.path.extension().is_some_and(|s| s.eq_ignore_ascii_case("pdf")) {
        return Err(error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Only PDF files can be returned"));
    }
    let metadata = std::fs::metadata(&resolved.path)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "Cannot inspect PDF"))?;
    if !metadata.is_file() {
        return Err(error(StatusCode::BAD_REQUEST, "PDF path must be a regular file"));
    }
    if metadata.len() > MAX_PDF_BYTES as u64 {
        return Err(error(StatusCode::PAYLOAD_TOO_LARGE, "PDF exceeds 10 MB; split it before requesting"));
    }
    let file = File::open(&resolved.path)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "Cannot read PDF"))?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(error(StatusCode::BAD_REQUEST, "PDF path must be a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_PDF_BYTES as u64 + 1).read_to_end(&mut bytes)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "Cannot read PDF"))?;
    if bytes.len() > MAX_PDF_BYTES {
        return Err(error(StatusCode::PAYLOAD_TOO_LARGE, "PDF exceeds 10 MB; split it before requesting"));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(error(StatusCode::UNSUPPORTED_MEDIA_TYPE, "File has no PDF signature"));
    }
    let name = resolved.path.file_name().unwrap_or_default().to_string_lossy();
    let fallback: String = name.chars().map(|c| {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') { c } else { '_' }
    }).collect();
    let encoded: String = name.bytes().map(|b| {
        if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_') { (b as char).to_string() }
        else { format!("%{b:02X}") }
    }).collect();
    Ok(PdfFile {
        bytes: Bytes::from(bytes),
        disposition: format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"),
        expires: Instant::now() + TTL,
    })
}

async fn download_pdf(State(state): State<PdfState>, Path(token): Path<String>) -> Result<Response, ApiError> {
    let mut cache = state.cache.lock().await;
    cache.prune();
    let file = cache.entries.get(&token)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "PDF link missing or expired; request the file again"))?;
    Response::builder()
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(header::CONTENT_DISPOSITION, &file.disposition)
        .header(header::CONTENT_LENGTH, file.bytes.len())
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Body::from(file.bytes.clone()))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Cannot build PDF response"))
}

pub(super) fn openapi_path(api_key: bool) -> Value {
    let mut op = json!({
        "operationId": "coding_return_pdf",
        "summary": "Return a local PDF as a conversation attachment",
        "description": "Return the original PDF requested by the user, up to 10 MB. Use list_files to locate it first if needed. path is workspace-relative or an explicit absolute local path. The response supplies a download URL valid for 5 minutes. This transfers the complete PDF to ChatGPT; do not use read_file for PDFs.",
        "x-openai-isConsequential": false,
        "requestBody": {"required": true, "content": {"application/json": {"schema": {
            "type": "object", "properties": {"path": {"type": "string", "minLength": 1, "description": "Exact local PDF path selected by the user"}},
            "required": ["path"], "additionalProperties": false
        }}}},
        "responses": {
            "200": {"description": "Original PDF attachment", "content": {"application/json": {"schema": {
                "type": "object", "required": ["openaiFileResponse"], "properties": {
                    "openaiFileResponse": {"type": "array", "minItems": 1, "maxItems": 1, "items": {"type": "string", "format": "uri"}}
                }
            }}}},
            "400": {"description": "Invalid or inaccessible path"},
            "401": {"description": "Authentication required"},
            "413": {"description": "PDF exceeds 10 MB"},
            "415": {"description": "Not a PDF"},
            "422": {"description": "Invalid JSON request"},
            "429": {"description": "Temporary PDF cache full"},
            "503": {"description": "Configure the Actions public HTTPS URL"}
        }
    });
    if api_key { op["security"] = json!([{"bearerAuth": []}]); }
    json!({"post": op})
}

#[cfg(test)]
#[path = "pdf_tests.rs"]
mod tests;
