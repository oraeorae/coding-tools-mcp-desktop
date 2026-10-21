use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::app_state::AppState;
use crate::commands::runtime::{start_actions_service, start_mcp_service};
use crate::error::AppResult;
use crate::runtime::ServiceKind;
use crate::tunnel::TunnelServiceKind;

const STATE_FILE: &str = "startup-restore.json";
const LOCAL_UPDATE_RESULT_FILE: &str = "local-update-restore-result.json";
const STARTUP_DELAY: Duration = Duration::from_millis(1500);
const TRACK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct StartupRestoreState {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    services: Vec<ServiceSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ServiceSnapshot {
    workspace_id: String,
    #[serde(default)]
    mcp: bool,
    #[serde(default)]
    actions: bool,
}

fn state_path() -> AppResult<PathBuf> {
    Ok(crate::platform::platform()
        .app_config_dir()?
        .join("data")
        .join(STATE_FILE))
}

fn read_state(path: &Path) -> AppResult<StartupRestoreState> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(StartupRestoreState::default())
        }
        Err(error) => Err(error.into()),
    }
}

fn write_state(path: &Path, state: &StartupRestoreState) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(state)?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    }
    atomic_replace(&temp, path)?;
    Ok(())
}

#[cfg(not(windows))]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn atomic_replace(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| std::io::Error::other(error.to_string()))
    }
}

fn snapshots_from_active(
    active: impl IntoIterator<Item = (String, TunnelServiceKind)>,
) -> Vec<ServiceSnapshot> {
    let mut snapshots = BTreeMap::<String, ServiceSnapshot>::new();
    for (workspace_id, kind) in active {
        let snapshot = snapshots
            .entry(workspace_id.clone())
            .or_insert(ServiceSnapshot {
                workspace_id,
                mcp: false,
                actions: false,
            });
        match kind {
            TunnelServiceKind::Mcp => snapshot.mcp = true,
            TunnelServiceKind::Actions => snapshot.actions = true,
        }
    }
    snapshots.into_values().collect()
}

fn apply_enabled(state: &mut StartupRestoreState, enabled: bool) {
    state.enabled = enabled;
    if !enabled {
        state.services.clear();
    }
}

pub fn get_enabled() -> AppResult<bool> {
    Ok(read_state(&state_path()?)?.enabled)
}

pub fn set_enabled(enabled: bool) -> AppResult<()> {
    let path = state_path()?;
    let mut state = read_state(&path)?;
    apply_enabled(&mut state, enabled);
    write_state(&path, &state)
}

pub fn capture_running_services(app: &AppHandle) -> AppResult<()> {
    let path = state_path()?;
    let mut state = read_state(&path)?;
    if !state.enabled {
        return Ok(());
    }
    let active = app
        .state::<AppState>()
        .with_runtime(|runtime| Ok(runtime.active_tunnel_service_keys()))?;
    let services = snapshots_from_active(active);
    if state.services == services {
        return Ok(());
    }
    state.services = services;
    write_state(&path, &state)
}

async fn track_running_services(app: AppHandle) {
    loop {
        tokio::time::sleep(TRACK_INTERVAL).await;
        // prepare_for_exit has already saved the final non-empty snapshot. Do
        // not let this background tracker overwrite it after listeners stop.
        if crate::commands::window_chrome::is_exit_prepared() {
            return;
        }
        if let Err(error) = capture_running_services(&app) {
            eprintln!("failed to refresh startup restore state: {error}");
        }
    }
}

fn is_running(app: &AppHandle, workspace_id: &str, kind: ServiceKind) -> AppResult<bool> {
    app.state::<AppState>()
        .with_runtime(|runtime| Ok(runtime.is_running(workspace_id, kind)))
}

async fn restore(app: AppHandle) -> AppResult<()> {
    let state = read_state(&state_path()?)?;
    if !state.enabled || state.services.is_empty() {
        return Ok(());
    }

    for snapshot in state.services {
        if snapshot.mcp && !is_running(&app, &snapshot.workspace_id, ServiceKind::Mcp)? {
            let result = {
                let app_state = app.state::<AppState>();
                start_mcp_service(&app_state, &snapshot.workspace_id).await
            };
            if let Err(error) = result {
                eprintln!(
                    "startup restore failed for workspace {} MCP: {error}",
                    snapshot.workspace_id
                );
            }
        }

        if snapshot.actions && !is_running(&app, &snapshot.workspace_id, ServiceKind::Actions)? {
            let result = {
                let app_state = app.state::<AppState>();
                start_actions_service(&app_state, &snapshot.workspace_id).await
            };
            if let Err(error) = result {
                eprintln!(
                    "startup restore failed for workspace {} Actions: {error}",
                    snapshot.workspace_id
                );
            }
        }
    }
    Ok(())
}

async fn wait_for_local_update_restore() {
    let Ok(result_path) = crate::platform::platform()
        .app_config_dir()
        .map(|path| path.join("data").join(LOCAL_UPDATE_RESULT_FILE))
    else {
        return;
    };
    for _ in 0..150 {
        if result_path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    eprintln!("startup restore continued after local update restore wait timed out");
}

pub fn schedule_restore(app: AppHandle, local_update_pending: bool) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        if local_update_pending {
            wait_for_local_update_restore().await;
        }
        if let Err(error) = restore(app.clone()).await {
            eprintln!("startup restore skipped: {error}");
        }
        // Start tracking only after the saved snapshot has been consumed;
        // otherwise an initially empty runtime would erase it before restore.
        track_running_services(app).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;

    #[test]
    fn missing_state_defaults_to_disabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = read_state(&dir.path().join(STATE_FILE)).expect("read default");
        assert!(!state.enabled);
        assert!(state.services.is_empty());
    }

    #[test]
    fn state_round_trips_with_private_permissions() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(STATE_FILE);
        let state = StartupRestoreState {
            enabled: true,
            services: vec![ServiceSnapshot {
                workspace_id: "alpha".into(),
                mcp: true,
                actions: false,
            }],
        };
        write_state(&path, &state).expect("write state");
        assert_eq!(read_state(&path).expect("read state"), state);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(path).expect("metadata").permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn active_services_are_grouped_by_workspace() {
        let snapshots = snapshots_from_active([
            ("beta".to_string(), TunnelServiceKind::Actions),
            ("alpha".to_string(), TunnelServiceKind::Mcp),
            ("beta".to_string(), TunnelServiceKind::Mcp),
        ]);
        assert_eq!(
            snapshots,
            vec![
                ServiceSnapshot {
                    workspace_id: "alpha".into(),
                    mcp: true,
                    actions: false,
                },
                ServiceSnapshot {
                    workspace_id: "beta".into(),
                    mcp: true,
                    actions: true,
                },
            ]
        );
    }

    #[test]
    fn disabling_clears_stale_services() {
        let mut state = StartupRestoreState {
            enabled: true,
            services: vec![ServiceSnapshot {
                workspace_id: "alpha".into(),
                mcp: true,
                actions: true,
            }],
        };
        apply_enabled(&mut state, false);
        assert!(!state.enabled);
        assert!(state.services.is_empty());
    }

    #[test]
    fn malformed_state_is_not_treated_as_enabled() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(STATE_FILE);
        fs::write(&path, b"not-json").expect("write malformed state");
        assert!(matches!(read_state(&path), Err(AppError::Json(_))));
    }
}
