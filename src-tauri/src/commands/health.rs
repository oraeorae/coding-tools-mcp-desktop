use tauri::State;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::health::{run_health_checks as execute_health_checks, HealthItem};
use crate::request_log::{record_request_error, RequestError};

fn profile_by_id(state: &AppState, id: &str) -> AppResult<crate::workspace::WorkspaceProfile> {
    state.with_workspaces(|store| {
        store
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Message(format!("workspace not found: {id}")))
    })
}

#[tauri::command]
pub async fn run_health_checks(state: State<'_, AppState>, id: String) -> AppResult<Vec<HealthItem>> {
    let profile = profile_by_id(&state, &id)?;
    let items = execute_health_checks(&profile).await;
    for item in items.iter().filter(|item| !item.ok) {
        let service = if item.label.starts_with("Actions") || item.label.contains("Actions") {
            "actions"
        } else {
            "mcp"
        };
        record_request_error(
            &profile.id,
            RequestError {
                service,
                source: "health",
                method: "GET",
                path: &item.label,
                status: parse_http_status(&item.detail),
                code: "health_check_failed",
                message: &item.detail,
                request_id: "",
                tool: "",
            },
        );
    }
    Ok(items)
}

fn parse_http_status(detail: &str) -> Option<u16> {
    detail
        .strip_prefix("HTTP ")
        .and_then(|rest| rest.split(|ch: char| !ch.is_ascii_digit()).next())
        .and_then(|value| value.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::parse_http_status;

    #[test]
    fn parses_health_http_status_without_guessing_network_errors() {
        assert_eq!(parse_http_status("HTTP 403"), Some(403));
        assert_eq!(parse_http_status("HTTP 404; FRP 未挂载代理"), Some(404));
        assert_eq!(parse_http_status("connection refused"), None);
    }
}
