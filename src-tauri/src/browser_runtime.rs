//! Desktop-owned bridge processes. Never takes ownership of an external bridge.
use std::{collections::HashMap, fs, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}, process::{Child, Stdio}, sync::{Mutex, OnceLock}, time::{Duration, Instant}};
use serde::Serialize;
use crate::tools::browser::bridge_health;

struct Entry { child: Child, started: Instant, log: PathBuf }
fn entries() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static ENTRIES: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    ENTRIES.get_or_init(Default::default)
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct BrowserStatus { pub state: String, pub message: String, pub managed: bool, pub node_available: bool, pub app_version: String, pub tool_count: usize }
fn node_path() -> Option<PathBuf> {
    which::which("node").ok().or_else(|| ["/opt/homebrew/bin/node", "/usr/local/bin/node"].iter().map(PathBuf::from).find(|p| p.is_file()))
}
fn result(state: &str, message: impl Into<String>, managed: bool) -> BrowserStatus {
    BrowserStatus {state:state.into(),message:message.into(),managed,node_available:node_path().is_some(),app_version:env!("CARGO_PKG_VERSION").into(),tool_count:crate::tools::list_tools_for_profile("core").len()}
}
fn log_tail(path: &Path) -> String {
    let Ok(mut file)=fs::File::open(path) else { return String::new() };
    let size=file.metadata().map(|m| m.len()).unwrap_or(0);
    let _=file.seek(SeekFrom::Start(size.saturating_sub(3000)));
    let mut bytes=Vec::new(); let _=file.take(3000).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}
pub fn status(root: &Path) -> BrowserStatus {
    let Ok(root)=root.canonicalize() else { return result("error","工作区目录不存在",false) };
    let mut entries=entries().lock().unwrap_or_else(|e| e.into_inner());
    let managed=entries.contains_key(&root);
    if let Some(entry)=entries.get_mut(&root) {
        match entry.child.try_wait() {
            Ok(Some(_)) => return result("error",format!("浏览器连接已退出，可重新连接。{}",log_tail(&entry.log)),true),
            Err(_) => return result("error","无法查询浏览器进程，请停止后重连",true),
            _ => {}
        }
    }
    match bridge_health(&root,false) {
        Ok(state) if state=="connected" => result("connected",if managed {"浏览器已连接，可打开网址、读取和截图。"} else {"浏览器由外部终端管理；此处不会终止它。"},managed),
        Ok(state) if state=="legacy" => result("external","检测到旧版桥接，请先在原终端停止，再在这里连接。",false),
        Ok(_) if managed => {
            let expired=entries.get(&root).is_some_and(|e| e.started.elapsed()>Duration::from_secs(140));
            result(if expired {"error"} else {"starting"},if expired {"等待授权超时，请停止后重新连接。"} else {"正在连接，请在 Chrome 的 Playwright 扩展页面选择要分享的标签页。"},true)
        }
        Ok(state) if state=="stale" => result("disconnected","旧连接已失效，点击连接会清理失效配置。",false),
        Ok(_) => result("disconnected","点击连接，在 Chrome 扩展中选择要分享的标签页。",false),
        Err(error) => result("error",error,managed),
    }
}
pub fn start(root: &Path, resource_dir: &Path) -> Result<BrowserStatus,String> {
    let root=root.canonicalize().map_err(|_| "工作区目录不存在")?;
    let mut map=entries().lock().map_err(|_| "浏览器管理器不可用")?;
    if let Some(entry)=map.get_mut(&root) {
        if entry.child.try_wait().map_err(|_| "无法查询浏览器进程")?.is_none() {
            return Ok(result("starting","连接已启动，请等待扩展授权或刷新状态。",true));
        }
    }
    map.remove(&root);
    match bridge_health(&root,true)?.as_str() {
        "disconnected" => {},
        "connected" => return Ok(result("connected","已有外部管理的浏览器连接。",false)),
        _ => return Err("已有浏览器桥接，请先在原终端停止后再连接。".into()),
    }
    let node=node_path().ok_or("未找到 Node.js，请安装 Node.js 20 或更高版本后重试。")?;
    let version=std::process::Command::new(&node).arg("--version").output().map_err(|_| "无法运行 Node.js")?;
    let major=String::from_utf8_lossy(&version.stdout).trim().trim_start_matches('v').split('.').next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    if major<20 { return Err("需要 Node.js 20 或更高版本。".into()); }
    let bundled=resource_dir.join("browser/server.mjs");
    let script=if bundled.is_file() { bundled } else if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bridge/browser/server.mjs")
    } else { return Err("安装包缺少浏览器组件，请重新安装。".into()) };
    let parent=script.parent().ok_or("浏览器组件路径无效")?;
    if !parent.join("node_modules/@playwright/mcp/package.json").is_file() { return Err("浏览器组件依赖缺失，请重新构建安装包。".into()); }
    let log_dir=dirs::home_dir().ok_or("无法找到用户目录")?.join(".coding-tools-mcp/browser-logs");
    fs::create_dir_all(&log_dir).map_err(|_| "无法创建浏览器日志目录")?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&log_dir,fs::Permissions::from_mode(0o700)).map_err(|_| "无法设置日志权限")?; }
    use sha2::{Digest,Sha256};
    let log=log_dir.join(format!("{:x}.log",Sha256::digest(root.to_string_lossy().as_bytes())));
    let mut options=fs::OpenOptions::new();options.write(true).create(true).truncate(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let file=options.open(&log).map_err(|_| "无法打开浏览器日志")?;
    let mut command=std::process::Command::new(node);
    command.arg(script).arg("--workspace").arg(&root).env("CODING_TOOLS_DESKTOP_BRIDGE","1")
        .stdin(Stdio::piped()).stdout(file.try_clone().map_err(|_| "无法复制日志句柄")?).stderr(file);
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let child=command.spawn().map_err(|e| format!("启动浏览器连接失败：{e}"))?;
    map.insert(root,Entry {child,started:Instant::now(),log});
    Ok(result("starting","连接已启动，请在 Chrome 中完成 Playwright 扩展授权。",true))
}
pub fn stop(root: &Path) -> Result<BrowserStatus,String> {
    let root=root.canonicalize().map_err(|_| "工作区目录不存在")?;
    let mut map=entries().lock().map_err(|_| "浏览器管理器不可用")?;
    if let Some(mut entry)=map.remove(&root) {
        // EOF asks our bridge to close its own adapter/configuration gracefully.
        drop(entry.child.stdin.take());
        let deadline=Instant::now()+Duration::from_secs(3);
        while Instant::now()<deadline {
            if entry.child.try_wait().ok().flatten().is_some() { break; }
            std::thread::sleep(Duration::from_millis(50));
        }
        if entry.child.try_wait().ok().flatten().is_none() { let _=entry.child.kill(); let _=entry.child.wait(); }
        let _=bridge_health(&root,true);
    }
    drop(map);
    Ok(status(&root))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_owns_one_child_and_stops_it_with_stdin_eof() {
        if node_path().is_none() { return; }
        let work=tempfile::tempdir().unwrap();
        let resources=tempfile::tempdir().unwrap();
        let browser=resources.path().join("browser");
        fs::create_dir_all(browser.join("node_modules/@playwright/mcp")).unwrap();
        fs::write(browser.join("node_modules/@playwright/mcp/package.json"),"{}").unwrap();
        fs::write(browser.join("server.mjs"),"process.stdin.resume(); process.stdin.on('end',()=>process.exit(0)); setInterval(()=>{},1000);").unwrap();
        assert_eq!(start(work.path(),resources.path()).unwrap().state,"starting");
        let root=work.path().canonicalize().unwrap();
        let (pid,log)={let map=entries().lock().unwrap();let e=map.get(&root).unwrap();(e.child.id(),e.log.clone())};
        assert_eq!(start(work.path(),resources.path()).unwrap().state,"starting");
        assert_eq!(entries().lock().unwrap().get(&root).unwrap().child.id(),pid);
        assert_eq!(stop(work.path()).unwrap().state,"disconnected");
        assert!(!entries().lock().unwrap().contains_key(&root));
        let _=fs::remove_file(log);
    }
}
