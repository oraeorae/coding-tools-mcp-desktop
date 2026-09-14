//! Bounded local execution records. These are observations, never instructions to rerun.
use std::{fs, io::Write, path::{Path, PathBuf}, sync::{OnceLock, Arc, Mutex}, time::{SystemTime, UNIX_EPOCH}};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use super::session::ExecSession;

const MAX_RECORD_BYTES: u64 = 3_000_000;
const MAX_COMPLETED: usize = 100;
fn owner() -> &'static str {
    static OWNER: OnceLock<String> = OnceLock::new();
    OWNER.get_or_init(|| Uuid::new_v4().to_string())
}
#[derive(Clone)]
pub(crate) struct Archive { root: PathBuf, write_lock: Arc<Mutex<()>> }
#[derive(Serialize, Deserialize)]
pub(crate) struct Record {
    owner: String,
    updated_at: u64,
    #[serde(default)]
    owner_pid: u32,
    pub snapshot: Value,
    stdout: String,
    stderr: String,
    stdout_total: usize,
    stderr_total: usize,
}
impl Archive {
    pub fn new(base: &Path, workspace: &Path) -> Self {
        let key = format!("{:x}", Sha256::digest(workspace.to_string_lossy().as_bytes()));
        Self { root: base.join("exec-sessions").join(key), write_lock: Arc::new(Mutex::new(())) }
    }
    pub fn key(&self) -> PathBuf { self.root.clone() }
    fn path(&self, id: &str) -> std::io::Result<PathBuf> {
        if Uuid::parse_str(id).is_err() || id.len() != 36 {
            return Err(std::io::Error::other("Invalid session ID"));
        }
        Ok(self.root.join(format!("{id}.json")))
    }
    fn prepare(&self) -> std::io::Result<()> {
        if let Some(parent)=self.root.parent() {
            if fs::symlink_metadata(parent).is_ok_and(|m| !m.is_dir()) { return Err(std::io::Error::other("Invalid archive parent")); }
        }
        fs::create_dir_all(&self.root)?;
        if !fs::symlink_metadata(&self.root)?.is_dir() { return Err(std::io::Error::other("Invalid archive directory")); }
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    pub fn save(&self, session: &ExecSession) -> std::io::Result<()> {
        let _guard = self.write_lock.lock().map_err(|_| std::io::Error::other("Archive lock unavailable"))?;
        self.prepare()?;
        let (out, stdout_total) = session.retained_stream_bytes("stdout");
        let (err, stderr_total) = session.retained_stream_bytes("stderr");
        let record = Record {
            owner: owner().into(), owner_pid: std::process::id(), updated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            snapshot: session.snapshot(0), stdout: STANDARD.encode(out), stderr: STANDARD.encode(err), stdout_total, stderr_total,
        };
        let path = self.path(&session.session_id)?;
        let tmp = self.root.join(format!("{}.tmp", Uuid::new_v4()));
        let mut options = fs::OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let result = (|| {
            let mut file = options.open(&tmp)?;
            file.write_all(&serde_json::to_vec(&record)?)?;
            file.sync_all()?;
            fs::rename(&tmp, &path)
        })();
        if result.is_err() { let _ = fs::remove_file(&tmp); }
        if result.is_ok() && session.has_exited() { self.prune(); }
        result
    }
    pub fn read(&self, id: &str) -> std::io::Result<Record> {
        let path = self.path(id)?;
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.len() > MAX_RECORD_BYTES { return Err(std::io::Error::other("Invalid session record")); }
        let mut record: Record = serde_json::from_slice(&fs::read(path)?)?;
        if record.snapshot["session_id"] != id { return Err(std::io::Error::other("Session ID mismatch")); }
        if record.snapshot["status"] == "running" && record.owner != owner() {
            record.snapshot["status"] = json!("interrupted");
            record.snapshot["termination_reason"] = json!("server_restart");
            record.snapshot["command_ok"] = Value::Null;
            record.snapshot["exit_code"] = Value::Null;
            record.snapshot["stdin_open"] = json!(false);
            record.snapshot["recoverable"] = json!(false);
            record.snapshot["suggestion"] = json!("服务已重启，无法接管旧进程；输出为最后快照。先核对实际进程和产物，再决定是否重新执行。");
        }
        record.snapshot["archived"] = json!(true);
        record.snapshot["output_persistence_ok"] = json!(true);
        record.snapshot["updated_at_ms"] = json!(record.updated_at);
        Ok(record)
    }
    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.root) else { return };
        let mut completed = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
            let Some(id) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            if let Ok(record) = self.read(id) {
                let foreign_live = record.owner != owner() && record.snapshot["termination_reason"] == "server_restart" && process_alive(record.owner_pid);
                if record.snapshot["status"] != "running" && !foreign_live { completed.push((record.updated_at, path)); }
            }
        }
        completed.sort_by_key(|r| r.0);
        let remove_count = completed.len().saturating_sub(MAX_COMPLETED);
        for (_, path) in completed.into_iter().take(remove_count) { let _ = fs::remove_file(path); }
    }
}
fn process_alive(pid: u32) -> bool {
    if pid == 0 { return false; }
    #[cfg(unix)] { unsafe { libc::kill(pid as i32, 0) == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) } }
    #[cfg(not(unix))] { true }
}
impl Record {
    pub fn stream(&self, stream: &str) -> (Vec<u8>, usize) {
        let (bytes, total) = if stream == "stderr" { (&self.stderr, self.stderr_total) } else { (&self.stdout, self.stdout_total) };
        (STANDARD.decode(bytes).unwrap_or_default(), total)
    }
    pub fn snapshot(mut self, max: usize) -> Value {
        for stream in ["stdout", "stderr"] {
            let (bytes, total) = self.stream(stream);
            let start = bytes.len().saturating_sub(max.min(1_048_576));
            self.snapshot[stream] = json!(String::from_utf8_lossy(&bytes[start..]));
            self.snapshot[format!("{stream}_truncated")] = json!(total > bytes.len() - start);
        }
        self.snapshot
    }
}

pub(crate) fn page(data: &[u8], total: usize, requested: usize, limit: usize) -> Value {
    let retained_start = total.saturating_sub(data.len());
    let offset = requested.max(retained_start).min(total);
    let local = offset - retained_start;
    let end = (local + limit).min(data.len());
    json!({"offset":offset, "requested_offset":requested, "content":String::from_utf8_lossy(&data[local..end]),
        "next_offset":if end < data.len() { Some(retained_start + end) } else { None },
        "total_retained_bytes":data.len(), "total_stream_bytes":total,
        "retained_start_offset":retained_start, "prefix_lost":requested < retained_start, "truncated":end < data.len()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paging_after_buffer_rollover_terminates() {
        let first = page(b"456789", 10, 0, 3);
        assert_eq!(first["offset"], 4); assert_eq!(first["next_offset"], 7); assert_eq!(first["prefix_lost"], true);
        let last = page(b"456789", 10, 7, 3);
        assert_eq!(last["content"], "789"); assert!(last["next_offset"].is_null());
    }
    #[test]
    fn old_owner_is_unknown_not_success_and_ids_cannot_escape() {
        let tmp = tempfile::tempdir().unwrap(); let archive = Archive::new(tmp.path(), tmp.path()); archive.prepare().unwrap();
        assert!(archive.read("../../x").is_err());
        let id = Uuid::new_v4().to_string();
        let r = Record { owner:"old-process".into(), owner_pid: 0, updated_at:1, snapshot:json!({"session_id":id,"status":"running","command_ok":true}), stdout:STANDARD.encode(b"saved"),stderr:String::new(),stdout_total:5,stderr_total:0 };
        fs::write(archive.path(&id).unwrap(),serde_json::to_vec(&r).unwrap()).unwrap();
        let value=archive.read(&id).unwrap().snapshot(10);
        assert_eq!(value["status"],"interrupted"); assert!(value["command_ok"].is_null()); assert_eq!(value["stdout"],"saved");
    }
}
