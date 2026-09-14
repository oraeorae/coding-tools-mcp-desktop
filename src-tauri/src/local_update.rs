use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::commands::runtime::{start_actions_service, start_mcp_service};

const PLAN_FILE: &str = "local-update-restore.json";
const RESULT_FILE: &str = "local-update-restore-result.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestorePlan {
    target_version: String,
    services: Vec<RestoreRequest>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreRequest {
    workspace_id: String,
    mcp: bool,
    actions: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreResult {
    target_version: String,
    success: bool,
    services: Vec<ServiceResult>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceResult {
    workspace_id: String,
    service: &'static str,
    success: bool,
    error: Option<String>,
}

fn take_matching_plan(path: &Path, current_version: &str) -> Result<Option<RestorePlan>, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("无法读取本地升级接力文件: {error}")),
    };
    fs::remove_file(path).map_err(|error| format!("无法消费本地升级接力文件: {error}"))?;
    let plan: RestorePlan = serde_json::from_slice(&bytes)
        .map_err(|error| format!("无法解析本地升级接力文件: {error}"))?;
    if plan.target_version != current_version {
        return Err(format!(
            "接力目标版本 {} 与当前版本 {} 不一致",
            plan.target_version, current_version
        ));
    }
    Ok(Some(plan))
}

fn write_result(path: &Path, result: &RestoreResult) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(result)
        .map_err(|error| format!("无法序列化本地升级恢复结果: {error}"))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, bytes).map_err(|error| format!("无法写入恢复结果: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("无法限制恢复结果权限: {error}"))?;
    }
    fs::rename(&temp, path).map_err(|error| format!("无法提交恢复结果: {error}"))
}

async fn restore(app: AppHandle, data_dir: PathBuf) {
    let plan_path = data_dir.join(PLAN_FILE);
    let result_path = data_dir.join(RESULT_FILE);
    let plan = match take_matching_plan(&plan_path, env!("CARGO_PKG_VERSION")) {
        Ok(Some(plan)) => plan,
        Ok(None) => return,
        Err(error) => {
            let _ = write_result(
                &result_path,
                &RestoreResult {
                    target_version: env!("CARGO_PKG_VERSION").into(),
                    success: false,
                    services: Vec::new(),
                    error: Some(error),
                },
            );
            return;
        }
    };

    let mut services = Vec::new();
    for request in plan.services {
        if request.mcp {
            let outcome = {
                let state = app.state::<AppState>();
                start_mcp_service(&state, &request.workspace_id).await
            };
            services.push(ServiceResult {
                workspace_id: request.workspace_id.clone(),
                service: "mcp",
                success: outcome.is_ok(),
                error: outcome.err().map(|error| error.to_string()),
            });
        }
        if request.actions {
            let outcome = {
                let state = app.state::<AppState>();
                start_actions_service(&state, &request.workspace_id).await
            };
            services.push(ServiceResult {
                workspace_id: request.workspace_id,
                service: "actions",
                success: outcome.is_ok(),
                error: outcome.err().map(|error| error.to_string()),
            });
        }
    }

    let success = services.iter().all(|result| result.success);
    let _ = write_result(
        &result_path,
        &RestoreResult {
            target_version: plan.target_version,
            success,
            services,
            error: None,
        },
    );
}

pub fn schedule_restore(app: AppHandle) {
    let Ok(data_dir) = app.path().app_data_dir() else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(350)).await;
        restore(app, data_dir).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_plan_is_consumed_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(PLAN_FILE);
        fs::write(
            &path,
            r#"{"targetVersion":"1.2.3","services":[{"workspaceId":"w1","mcp":true,"actions":false}]}"#,
        )
        .expect("write");
        let plan = take_matching_plan(&path, "1.2.3")
            .expect("take")
            .expect("plan");
        assert_eq!(plan.services.len(), 1);
        assert!(!path.exists());
        assert!(take_matching_plan(&path, "1.2.3").expect("second").is_none());
    }

    #[test]
    fn mismatched_plan_is_consumed_without_starting_services() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(PLAN_FILE);
        fs::write(&path, r#"{"targetVersion":"9.9.9","services":[]}"#).expect("write");
        assert!(take_matching_plan(&path, "1.2.3").is_err());
        assert!(!path.exists());
    }
}
