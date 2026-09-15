use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::tunnel::log_dir_for_profile;

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_MESSAGE_CHARS: usize = 500;
const MAX_FIELD_CHARS: usize = 120;
const MAX_READ_ENTRIES: usize = 200;
static WRITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone)]
pub struct RequestLogContext {
    pub workspace_id: String,
    pub service: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RequestErrorEntry {
    pub id: String,
    pub timestamp_ms: u64,
    pub service: String,
    pub source: String,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub code: String,
    pub message: String,
    pub request_id: String,
    pub tool: String,
}

pub struct RequestError<'a> {
    pub service: &'a str,
    pub source: &'a str,
    pub method: &'a str,
    pub path: &'a str,
    pub status: Option<u16>,
    pub code: &'a str,
    pub message: &'a str,
    pub request_id: &'a str,
    pub tool: &'a str,
}

pub async fn log_http_errors(
    State(context): State<RequestLogContext>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let response = next.run(request).await;
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        record_request_error(
            &context.workspace_id,
            RequestError {
                service: context.service,
                source: "http",
                method: &method,
                path: &path,
                status: Some(status.as_u16()),
                code: status.canonical_reason().unwrap_or("http_error"),
                message: status.canonical_reason().unwrap_or("HTTP request failed"),
                request_id: "",
                tool: "",
            },
        );
    }
    response
}

pub fn record_request_error(workspace_id: &str, error: RequestError<'_>) {
    let entry = RequestErrorEntry {
        id: Uuid::new_v4().to_string(),
        timestamp_ms: now_ms(),
        service: clean_field(error.service, MAX_FIELD_CHARS),
        source: clean_field(error.source, MAX_FIELD_CHARS),
        method: clean_field(error.method, MAX_FIELD_CHARS),
        path: clean_field(error.path, MAX_FIELD_CHARS),
        status: error.status,
        code: clean_field(error.code, 80),
        message: clean_field(error.message, MAX_MESSAGE_CHARS),
        request_id: clean_field(error.request_id, MAX_FIELD_CHARS),
        tool: clean_field(error.tool, MAX_FIELD_CHARS),
    };
    let directory = log_dir_for_profile(workspace_id);
    if let Err(err) = write_entry(&directory, &entry) {
        eprintln!("request error log write failed: {err}");
    }
}

pub fn read_request_errors(workspace_id: &str, service: &str) -> AppResult<Vec<RequestErrorEntry>> {
    validate_service(service)?;
    read_entries(&log_dir_for_profile(workspace_id), service)
}

pub fn clear_request_errors(workspace_id: &str, service: &str) -> AppResult<()> {
    validate_service(service)?;
    let directory = log_dir_for_profile(workspace_id);
    let _guard = write_lock()?;
    remove_if_exists(&current_path(&directory, service))?;
    remove_if_exists(&rotated_path(&directory, service))?;
    Ok(())
}

fn write_entry(directory: &Path, entry: &RequestErrorEntry) -> AppResult<()> {
    validate_service(&entry.service)?;
    let line = serde_json::to_string(entry)?;
    let _guard = write_lock()?;
    fs::create_dir_all(directory)?;
    let current = current_path(directory, &entry.service);
    let next_size = current.metadata().map(|m| m.len()).unwrap_or(0) + line.len() as u64 + 1;
    if next_size > MAX_FILE_BYTES && current.exists() {
        let rotated = rotated_path(directory, &entry.service);
        remove_if_exists(&rotated)?;
        fs::rename(&current, rotated)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(current)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn read_entries(directory: &Path, service: &str) -> AppResult<Vec<RequestErrorEntry>> {
    let _guard = write_lock()?;
    let mut entries = Vec::new();
    for path in [rotated_path(directory, service), current_path(directory, service)] {
        if !path.exists() {
            continue;
        }
        let reader = BufReader::new(File::open(path)?);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(entry) = serde_json::from_str::<RequestErrorEntry>(&line) {
                if entry.service == service {
                    entries.push(entry);
                }
            }
        }
    }
    entries.sort_by(|left, right| right.timestamp_ms.cmp(&left.timestamp_ms));
    entries.truncate(MAX_READ_ENTRIES);
    Ok(entries)
}

fn current_path(directory: &Path, service: &str) -> PathBuf {
    directory.join(format!("request-errors-{service}.jsonl"))
}

fn rotated_path(directory: &Path, service: &str) -> PathBuf {
    directory.join(format!("request-errors-{service}.jsonl.1"))
}

fn validate_service(service: &str) -> AppResult<()> {
    if matches!(service, "mcp" | "actions") {
        Ok(())
    } else {
        Err(AppError::Message(format!("unknown request log service: {service}")))
    }
}

fn remove_if_exists(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn write_lock() -> AppResult<std::sync::MutexGuard<'static, ()>> {
    WRITE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| AppError::Message("request error log lock poisoned".into()))
}

fn clean_field(value: &str, max_chars: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max_chars)
        .collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(service: &str, timestamp_ms: u64, message: &str) -> RequestErrorEntry {
        RequestErrorEntry {
            id: Uuid::new_v4().to_string(),
            timestamp_ms,
            service: service.into(),
            source: "http".into(),
            method: "POST".into(),
            path: "/mcp".into(),
            status: Some(401),
            code: "Unauthorized".into(),
            message: message.into(),
            request_id: String::new(),
            tool: String::new(),
        }
    }

    #[test]
    fn cleans_multiline_and_limits_message_length() {
        let cleaned = clean_field(&format!("first\n{}", "x".repeat(600)), MAX_MESSAGE_CHARS);
        assert!(!cleaned.contains('\n'));
        assert_eq!(cleaned.chars().count(), MAX_MESSAGE_CHARS);
    }

    #[test]
    fn reads_newest_entries_and_ignores_invalid_lines() {
        let temp = tempfile::tempdir().unwrap();
        write_entry(temp.path(), &entry("mcp", 1, "older")).unwrap();
        let path = current_path(temp.path(), "mcp");
        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        writeln!(file, "invalid json").unwrap();
        write_entry(temp.path(), &entry("mcp", 2, "newer")).unwrap();

        let entries = read_entries(temp.path(), "mcp").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message, "newer");
    }

    #[test]
    fn rotates_at_capacity_and_clear_removes_both_files() {
        let temp = tempfile::tempdir().unwrap();
        let current = current_path(temp.path(), "actions");
        fs::write(&current, vec![b'x'; MAX_FILE_BYTES as usize]).unwrap();
        write_entry(temp.path(), &entry("actions", 3, "failure")).unwrap();
        assert!(rotated_path(temp.path(), "actions").exists());

        remove_if_exists(&current).unwrap();
        remove_if_exists(&rotated_path(temp.path(), "actions")).unwrap();
        assert!(!current.exists());
        assert!(!rotated_path(temp.path(), "actions").exists());
    }

    #[test]
    fn rejects_unknown_service_names_before_building_a_path() {
        assert!(validate_service("../outside").is_err());
        assert!(validate_service("desktop").is_err());
        assert!(validate_service("mcp").is_ok());
        assert!(validate_service("actions").is_ok());
    }
}
