use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::Semaphore;

pub(super) const HISTORY_DEADLINE: Duration = Duration::from_secs(5);

// The permit lives in the worker, not the HTTP future. A timed-out or disconnected
// caller must not allow another history worker to pile up behind blocked OS I/O.
pub(super) async fn run_history(
    gate: Arc<Semaphore>,
    deadline: Duration,
    work: impl FnOnce() -> Value + Send + 'static,
) -> Result<Value, &'static str> {
    let permit = gate.try_acquire_owned().map_err(|_| "HISTORY_BUSY")?;
    let worker = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work()
    });
    match tokio::time::timeout(deadline, worker).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err("HISTORY_WORKER_FAILED"),
        Err(_) => Err("HISTORY_TIMEOUT"),
    }
}

pub(super) fn error_response(id: Value, code: &str) -> Value {
    let message = if code == "HISTORY_BUSY" {
        "A history operation is still in progress; this request was not started. Do not automatically retry. Independent tools remain available."
    } else {
        "History did not finish within the response deadline or its worker failed. The operation may still complete; its outcome is unknown. Do not automatically retry or claim progress was saved. Independent tools remain available."
    };
    let structured = json!({"ok": false, "error": {"code": code, "message": message,
        "category": "runtime", "details": {}, "retryable": false, "outcome": if code == "HISTORY_BUSY" { "not_started" } else { "unknown" }}});
    json!({"jsonrpc": "2.0", "id": id, "result": {
        "isError": true, "structuredContent": structured,
        "content": [{"type": "text", "text": structured.to_string()}]
    }})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timeout_keeps_permit_until_worker_finishes_and_rejects_retries() {
        let gate = Arc::new(Semaphore::new(1));
        let (release, blocked) = std::sync::mpsc::channel();
        let result = run_history(gate.clone(), Duration::from_millis(30), move || {
            blocked.recv_timeout(Duration::from_secs(2)).unwrap();
            json!({"ok": true})
        }).await;
        assert_eq!(result, Err("HISTORY_TIMEOUT"));
        assert_eq!(run_history(gate.clone(), HISTORY_DEADLINE, || panic!("must not run")).await, Err("HISTORY_BUSY"));
        release.send(()).unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(2), gate.clone().acquire_owned()).await.unwrap().unwrap();
        drop(permit);
        assert_eq!(run_history(gate, HISTORY_DEADLINE, || json!(42)).await, Ok(json!(42)));
    }

    #[tokio::test]
    async fn client_cancellation_does_not_release_running_worker() {
        let gate = Arc::new(Semaphore::new(1));
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, running) = tokio::sync::oneshot::channel();
        let gate_for_task = gate.clone();
        let request = tokio::spawn(async move {
            run_history(gate_for_task, HISTORY_DEADLINE, move || {
                started.send(()).unwrap();
                blocked.recv_timeout(Duration::from_secs(2)).unwrap();
                json!(null)
            }).await
        });
        running.await.unwrap();
        request.abort();
        assert_eq!(run_history(gate.clone(), HISTORY_DEADLINE, || panic!("must not run")).await, Err("HISTORY_BUSY"));
        release.send(()).unwrap();
        let _permit = tokio::time::timeout(Duration::from_secs(2), gate.acquire_owned()).await.unwrap().unwrap();
    }
}
