use tauri::{Manager, State};
use crate::{app_state::AppState, browser_runtime::{self, BrowserStatus}};
fn root(state: &AppState, id: &str) -> Result<std::path::PathBuf,String> {
    state.with_workspaces(|store| store.get(id).map(|p| p.path.clone().into()).ok_or_else(|| crate::error::AppError::Message("工作区不存在".into()))).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn browser_status(state: State<'_,AppState>, id: String) -> Result<BrowserStatus,String> {
    let root=root(&state,&id)?;
    tauri::async_runtime::spawn_blocking(move || browser_runtime::status(&root)).await.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn start_browser_bridge(app: tauri::AppHandle,state: State<'_,AppState>,id: String) -> Result<BrowserStatus,String> {
    let root=root(&state,&id)?; let resources=app.path().resource_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || browser_runtime::start(&root,&resources)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn stop_browser_bridge(state: State<'_,AppState>,id: String) -> Result<BrowserStatus,String> {
    let root=root(&state,&id)?;
    tauri::async_runtime::spawn_blocking(move || browser_runtime::stop(&root)).await.map_err(|e| e.to_string())?
}
