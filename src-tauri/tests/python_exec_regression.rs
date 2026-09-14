use coding_tools_mcp_desktop_lib::tools::{call_tool, context::ToolContext};
use serde_json::json;
use std::{fs, time::{Duration, Instant}};

#[cfg(unix)]
#[test]
fn standalone_error_does_not_hash_large_data_files() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::File::create(root.path().join("market-data.pkl")).unwrap()
        .set_len(128 * 1024 * 1024).unwrap();
    let ctx = ToolContext::for_test(root.path().into(), store.path().into()).unwrap();
    let start = Instant::now();
    let result = call_tool(&ctx, "exec_command", &json!({"cmd":"/bin/echo diagnostic"}));
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "EXECUTABLE_OUTSIDE_WORKSPACE");
    assert!(start.elapsed() < Duration::from_secs(1), "error response scanned data: {:?}", start.elapsed());
}

#[test]
fn active_task_still_detects_data_change() {
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    fs::write(root.path().join("data.csv"), "before").unwrap();
    let ctx = ToolContext::for_test(root.path().into(), store.path().into()).unwrap();
    ctx.harness.start_task("baseline protection").unwrap();
    assert!(ctx.harness.status().unwrap().writable);
    fs::write(root.path().join("data.csv"), "after").unwrap();
    let status = ctx.harness.status().unwrap();
    assert!(!status.writable);
    assert_eq!(status.baseline_matches, Some(false));
}

#[cfg(unix)]
#[test]
fn registered_canonical_executable_only_and_symlink_retarget_rejected() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let store = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let target = fs::canonicalize("/bin/echo").unwrap();
    let alias = outside.path().join("python");
    symlink(&target, &alias).unwrap();
    let mut ctx = ToolContext::for_test(root.path().into(), store.path().into()).unwrap();
    let args = json!({"cmd":format!("'{}' registered-probe", alias.display()), "yield_time_ms":1000});
    assert_eq!(call_tool(&ctx,"exec_command",&args)["error"]["code"], "EXECUTABLE_OUTSIDE_WORKSPACE");
    ctx.policy.allowed_commands.insert(target.to_string_lossy().into_owned());
    let result = call_tool(&ctx,"exec_command",&args);
    assert_eq!(result["ok"],true,"{result}");
    assert_eq!(result["exit_code"],0,"{result}");
    assert!(result["stdout"].as_str().unwrap().contains("registered-probe"));
    fs::remove_file(&alias).unwrap();
    symlink("/bin/cat", &alias).unwrap();
    assert_eq!(call_tool(&ctx,"exec_command",&args)["error"]["code"], "EXECUTABLE_OUTSIDE_WORKSPACE");
}
