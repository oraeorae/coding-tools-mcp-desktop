use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::path::{Path, PathBuf};
use super::session_archive::{Archive, Record};
use std::time::Instant;

use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStdin};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use crate::tools::workspace::WorkspaceError;
#[path = "session_io.rs"]
mod io;
pub use io::{read_output, write_stdin, kill_session};
use serde_json::{json, Value};

const SESSION_BUFFER_BYTES: usize = 1_048_576;

#[derive(Default)]
pub struct SessionStore {
    sessions: Mutex<HashMap<String, Arc<ExecSession>>>,
    archive: Option<Archive>,
    saved_versions: Mutex<HashMap<String, (usize, usize, bool)>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared(base: &Path, workspace: &Path) -> Arc<Self> {
        static STORES: OnceLock<Mutex<HashMap<PathBuf, Weak<SessionStore>>>> = OnceLock::new();
        let archive = Archive::new(base, workspace);
        let mut stores = STORES.get_or_init(Default::default).lock().expect("session stores");
        stores.retain(|_, value| value.strong_count() > 0);
        if let Some(store) = stores.get(&archive.key()).and_then(Weak::upgrade) { return store; }
        let store = Arc::new(Self { sessions: Mutex::new(HashMap::new()), archive: Some(archive.clone()), saved_versions: Mutex::new(HashMap::new()) });
        stores.insert(archive.key(), Arc::downgrade(&store));
        store
    }

    pub(crate) fn persist(&self, session: &ExecSession) {
        if let Some(archive) = &self.archive {
            let version = (*session.stdout_total.lock().expect("stdout total"), *session.stderr_total.lock().expect("stderr total"), session.has_exited());
            let mut saved = self.saved_versions.lock().expect("saved versions");
            if saved.get(&session.session_id) == Some(&version) { return; }
            match archive.save(session) {
                Ok(()) => {
                    saved.insert(session.session_id.clone(), version);
                    *session.archive_ok.lock().expect("archive state") = Some(true);
                },
                Err(error) => {
                    *session.archive_ok.lock().expect("archive state") = Some(false);
                    eprintln!("Session archive unavailable: {error}");
                }
            }
        }
    }

    fn archived(&self, id: &str) -> Result<Record, WorkspaceError> {
        self.archive.as_ref().and_then(|a| a.read(id).ok()).ok_or_else(|| WorkspaceError::Tool {
            code: "SESSION_NOT_FOUND", message: format!("Session not found: {id}"), category:"not_found", retryable:false,
        })
    }

    pub fn insert(&self, session: ExecSession) -> Arc<ExecSession> {
        self.persist(&session);
        let arc = Arc::new(session);
        self.sessions
            .lock()
            .expect("sessions lock")
            .insert(arc.session_id.clone(), arc.clone());
        arc
    }

    pub fn get(&self, session_id: &str) -> Result<Arc<ExecSession>, WorkspaceError> {
        self.sessions
            .lock()
            .expect("sessions lock")
            .get(session_id)
            .cloned()
            .ok_or_else(|| WorkspaceError::Tool {
                code: "SESSION_NOT_FOUND",
                message: format!("Session not found: {session_id}"),
                category: "not_found",
                retryable: false,
            })
    }

    pub fn remove(&self, session_id: &str) {
        if let Some(session) = self.sessions.lock().expect("sessions lock").remove(session_id) {
            self.persist(&session);
            self.saved_versions.lock().expect("saved versions").remove(session_id);
        }
    }
}

pub struct ExecSession {
    pub session_id: String,
    archive_ok: Mutex<Option<bool>>,
    pub(crate) child: AsyncMutex<Child>,
    pub stdin: AsyncMutex<Option<ChildStdin>>,
    stdin_open: Mutex<bool>,
    interactive: bool,
    stdout: Mutex<Vec<u8>>,
    stderr: Mutex<Vec<u8>>,
    stdout_total: Mutex<usize>,
    stderr_total: Mutex<usize>,
    pub started_at: Instant,
    pub exit_code: Mutex<Option<i32>>,
    exited: AtomicBool,
    termination_reason: Mutex<Option<String>>,
    reader_tasks: AsyncMutex<Vec<tauri::async_runtime::JoinHandle<()>>>,
}

impl ExecSession {
    pub fn new(child: Child) -> Self {
        Self::new_with_mode(child, false)
    }

    pub fn new_with_mode(mut child: Child, interactive: bool) -> Self {
        let session_id = Uuid::new_v4().to_string();
        let stdin = child.stdin.take();
        let stdin_open = stdin.is_some();
        Self {
            session_id,
            archive_ok: Mutex::new(None),
            child: AsyncMutex::new(child),
            stdin: AsyncMutex::new(stdin),
            stdin_open: Mutex::new(stdin_open),
            interactive,
            stdout: Mutex::new(Vec::new()),
            stderr: Mutex::new(Vec::new()),
            stdout_total: Mutex::new(0),
            stderr_total: Mutex::new(0),
            started_at: Instant::now(),
            exit_code: Mutex::new(None),
            exited: AtomicBool::new(false),
            termination_reason: Mutex::new(None),
            reader_tasks: AsyncMutex::new(Vec::new()),
        }
    }

    pub async fn spawn_readers(self: &Arc<Self>) {
        let stdout = {
            let mut guard = self.child.lock().await;
            guard.stdout.take()
        };
        let stderr = {
            let mut guard = self.child.lock().await;
            guard.stderr.take()
        };
        if let Some(stream) = stdout {
            let session = Arc::clone(self);
            let task = tauri::async_runtime::spawn(async move {
                session.read_stream(stream, true).await;
            });
            self.reader_tasks.lock().await.push(task);
        }
        if let Some(stream) = stderr {
            let session = Arc::clone(self);
            let task = tauri::async_runtime::spawn(async move {
                session.read_stream(stream, false).await;
            });
            self.reader_tasks.lock().await.push(task);
        }
    }

    pub async fn wait_for_readers(&self) {
        let mut tasks = self.reader_tasks.lock().await;
        while let Some(task) = tasks.pop() {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(500), task).await;
        }
    }

    async fn read_stream<T>(&self, mut stream: T, is_stdout: bool)
    where
        T: tokio::io::AsyncRead + Unpin,
    {
        let mut buf = [0u8; 4096];
        loop {
            match stream.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = &buf[..n];
                    if is_stdout {
                        let mut data = self.stdout.lock().expect("stdout lock");
                        data.extend_from_slice(chunk);
                        *self.stdout_total.lock().expect("stdout_total lock") += n;
                        trim_buffer(&mut data, SESSION_BUFFER_BYTES);
                    } else {
                        let mut data = self.stderr.lock().expect("stderr lock");
                        data.extend_from_slice(chunk);
                        *self.stderr_total.lock().expect("stderr_total lock") += n;
                        trim_buffer(&mut data, SESSION_BUFFER_BYTES);
                    }
                }
                Err(_) => break,
            }
        }
    }

    pub async fn kill_and_wait(&self) {
        let status = {
            let mut child = self.child.lock().await;
            let _ = child.start_kill();
            child.wait().await.ok()
        };
        if let Some(status) = status {
            self.record_exit_status(status);
        }
    }

    pub async fn refresh_status(&self) {
        let mut child = self.child.lock().await;
        if let Ok(Some(status)) = child.try_wait() {
            self.record_exit_status(status);
        }
    }

    fn record_exit_status(&self, status: std::process::ExitStatus) {
        *self.exit_code.lock().expect("exit_code lock") = status.code();
        self.exited.store(true, Ordering::Release);
        *self.stdin_open.lock().expect("stdin_open lock") = false;
        let mut reason = self.termination_reason.lock().expect("termination lock");
        if reason.is_none() {
            *reason = Some("exited".into());
        }
    }

    pub(crate) fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    pub fn mark_termination_reason(&self, reason: &str) {
        *self.termination_reason.lock().expect("termination lock") = Some(reason.to_string());
    }

    pub(crate) fn mark_stdin_closed(&self) {
        *self.stdin_open.lock().expect("stdin_open lock") = false;
    }

    pub async fn is_running(&self) -> bool {
        self.refresh_status().await;
        !self.has_exited()
    }

    pub fn retained_stream_bytes(&self, stream: &str) -> (Vec<u8>, usize) {
        match stream {
            "stderr" => {
                let guard = self.stderr.lock().expect("stderr lock");
                let data = guard.clone();
                let total = *self.stderr_total.lock().expect("stderr_total lock");
                (data, total)
            }
            _ => {
                let guard = self.stdout.lock().expect("stdout lock");
                let data = guard.clone();
                let total = *self.stdout_total.lock().expect("stdout_total lock");
                (data, total)
            }
        }
    }

    pub fn snapshot(&self, max_output_bytes: usize) -> Value {
        let stdout_bytes = self.stdout.lock().expect("stdout lock").clone();
        let stderr_bytes = self.stderr.lock().expect("stderr lock").clone();
        let stdout = truncate_tail(&stdout_bytes, max_output_bytes);
        let stderr = truncate_tail(&stderr_bytes, max_output_bytes);
        let exit_code = *self.exit_code.lock().expect("exit_code lock");
        let termination_reason = self
            .termination_reason
            .lock()
            .expect("termination lock")
            .clone();
        let status = if self.has_exited() {
            "exited"
        } else {
            "running"
        };
        let reason = termination_reason.as_deref().unwrap_or("running");
        let command_ok = match reason {
            "exited" => Some(exit_code.is_some_and(|code| code == 0)),
            "running" => None,
            _ => Some(false),
        };
        json!({
            "session_id": self.session_id,
            "output_persistence_ok": *self.archive_ok.lock().expect("archive state"),
            "interactive": self.interactive,
            "stdin_open": *self.stdin_open.lock().expect("stdin_open lock"),
            "status": status,
            "termination_reason": reason,
            "recoverable": matches!(reason, "timeout" | "killed" | "spawn_failed" | "server_restart"),
            "suggestion": match reason {
                "timeout" => "读取保留输出，调整 timeout_ms 后重试",
                "killed" => "确认终止原因后重新执行命令",
                "exited" => "检查 exit_code 和 stderr",
                "crashed" => "检查 stderr 后重试或恢复工作区",
                _ => "继续读取 session 或等待进程结束",
            },
            "exit_code": exit_code,
            "transport_ok": true,
            "command_ok": command_ok,
            "stdout": stdout.content,
            "stderr": stderr.content,
            "stdout_truncated": stdout.truncated,
            "stderr_truncated": stderr.truncated,
            "elapsed_ms": self.started_at.elapsed().as_millis(),
            "output_refs": {
                "stdout": format!("session:{}:stdout", self.session_id),
                "stderr": format!("session:{}:stderr", self.session_id)
            }
        })
    }
}

fn trim_buffer(buf: &mut Vec<u8>, limit: usize) {
    if buf.len() > limit {
        let drop = buf.len() - limit;
        buf.drain(..drop);
    }
}

struct Truncated {
    content: String,
    truncated: bool,
}

fn truncate_tail(bytes: &[u8], max_bytes: usize) -> Truncated {
    let truncated = bytes.len() > max_bytes;
    let take = bytes.len().min(max_bytes);
    Truncated {
        content: String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(take)..]).into_owned(),
        truncated,
    }
}
