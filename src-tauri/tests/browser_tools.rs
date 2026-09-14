use coding_tools_mcp_desktop_lib::tools::{call_tool, list_tools_for_profile, wrap_mcp_tool_result, ToolContext};
use serde_json::json;

#[test]
fn browser_tools_are_read_only_and_registered_in_all_profiles() {
    for profile in ["core", "read-only", "advanced"] {
        let tools = list_tools_for_profile(profile);
        for name in ["browser_list_tabs", "browser_read_page", "browser_screenshot"] {
            let tool = tools.iter().find(|t| t["name"] == name).expect("browser tool exposed");
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
            assert_eq!(tool["annotations"]["destructiveHint"], false);
            assert_eq!(tool["annotations"]["openWorldHint"], true);
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        }
    }
}

#[test]
fn unpaired_workspace_fails_closed_and_rejects_executable_arguments() {
    let workspace = tempfile::tempdir().unwrap();
    let harness = tempfile::tempdir().unwrap();
    let ctx = ToolContext::for_test(workspace.path().to_path_buf(), harness.path().to_path_buf()).unwrap();
    let result = call_tool(&ctx, "browser_read_page", &json!({}));
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "BROWSER_NOT_CONNECTED");
    for args in [json!({"code":"process.exit()"}), json!({"endpoint":"http://external.example"}), json!({"cursor":-1})] {
        let result = call_tool(&ctx, "browser_read_page", &args);
        assert_eq!(result["error"]["code"], "INVALID_ARGUMENT");
    }
}

#[test]
fn screenshot_is_mcp_image_and_errors_stay_text() {
    let result = wrap_mcp_tool_result("browser_screenshot", &json!({}), json!({"ok":true,"mime_type":"image/jpeg","base64":"/9j/"}));
    assert_eq!(result["content"][0]["type"], "image");
    assert_eq!(result["content"][0]["mimeType"], "image/jpeg");
    assert_eq!(result["content"][0]["data"], "/9j/");
    let error = wrap_mcp_tool_result("browser_screenshot", &json!({}), json!({"ok":false,"error":{"code":"TAB_NOT_FOUND"}}));
    assert_eq!(error["content"][0]["type"], "text");
    assert_eq!(error["isError"], true);
    let old = wrap_mcp_tool_result("view_image", &json!({}), json!({"ok":true,"mime_type":"image/png","base64":"test"}));
    assert_eq!(old["content"][0]["type"], "image");
}

// Explicitly opt in: this opens an isolated headless Chrome using synthetic local content.
#[test]
#[ignore = "Requires bridge npm ci and BROWSER_TEST_EXECUTABLE or Playwright browser installation"]
fn browser_bridge_end_to_end() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;
    use sha2::{Digest, Sha256};
    let workspace = tempfile::tempdir().unwrap();
    let harness = tempfile::tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../bridge/browser/test/fixture.mjs");
    let mut child = Command::new("node").arg(script).arg(&root).stdout(Stdio::piped()).stderr(Stdio::inherit()).spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || { for line in BufReader::new(stdout).lines().map_while(Result::ok) { if line.starts_with("READY ") { let _ = tx.send(line); break; } } });
    struct Cleanup(std::process::Child, std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            #[cfg(unix)]
            unsafe { libc::kill(self.0.id() as i32, libc::SIGTERM); }
            for _ in 0..50 {
                if self.0.try_wait().ok().flatten().is_some() { break; }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = self.0.kill(); let _ = self.0.wait();
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let normalized = root.to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/");
    let key = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    let path = dirs::home_dir().unwrap().join(".coding-tools-mcp/browser").join(format!("{key}.json"));
    let _cleanup = Cleanup(child, path);
    rx.recv_timeout(Duration::from_secs(45)).expect("fixture ready");
    let ctx = ToolContext::for_test(root, harness.path().to_path_buf()).unwrap();
    let list = call_tool(&ctx, "browser_list_tabs", &json!({}));
    assert_eq!(list["ok"], true, "{list}");
    assert_eq!(list["tabs"].as_array().unwrap().len(), 2);
    let selected = list["selected_tab_id"].as_str().unwrap();
    let read = call_tool(&ctx, "browser_read_page", &json!({}));
    assert_eq!(read["tab_id"], selected);
    assert!(read["text"].as_str().unwrap().contains("selected fixture"));
    let shot = call_tool(&ctx, "browser_screenshot", &json!({}));
    assert_eq!(shot["ok"], true, "{shot}");
    assert_eq!(shot["mime_type"], "image/jpeg");
    assert_eq!(wrap_mcp_tool_result("browser_screenshot", &json!({}), shot)["content"][0]["type"], "image");
    let opened = call_tool(&ctx, "browser_open", &json!({"url":format!("{}?opened=1", list["tabs"][0]["url"].as_str().unwrap())}));
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(opened["selected"], true);
    assert_ne!(opened["tab_id"], selected);
    let after = call_tool(&ctx,"browser_read_page",&json!({}));
    assert_eq!(after["tab_id"],opened["tab_id"]);
    assert_eq!(call_tool(&ctx,"browser_list_tabs",&json!({}))["selected_tab_id"],opened["tab_id"]);
    assert_eq!(call_tool(&ctx,"browser_screenshot",&json!({}))["tab_id"],opened["tab_id"]);
    let other = tempfile::tempdir().unwrap();
    let other_ctx = ToolContext::for_test(other.path().to_path_buf(), harness.path().join("other")).unwrap();
    assert_eq!(call_tool(&other_ctx,"browser_read_page",&json!({}))["error"]["code"],"BROWSER_NOT_CONNECTED");
}

#[test]
fn browser_open_is_mutating_and_excluded_from_read_only() {
    for profile in ["core", "advanced", "compat-readonly-all"] {
        let tools = list_tools_for_profile(profile);
        let tool = tools.iter().find(|t| t["name"] == "browser_open").unwrap();
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["annotations"]["openWorldHint"], true);
        assert_eq!(tool["inputSchema"]["required"], json!(["url"]));
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }
    assert!(coding_tools_mcp_desktop_lib::tools::MUTATING_TOOLS.contains(&"browser_open"));
    assert!(!list_tools_for_profile("read-only").iter().any(|t| t["name"] == "browser_open"));
    let workspace = tempfile::tempdir().unwrap();
    let harness = tempfile::tempdir().unwrap();
    let ctx = ToolContext::for_test(workspace.path().to_path_buf(), harness.path().to_path_buf()).unwrap();
    for args in [json!({}), json!({"url":42}), json!({"url":"javascript:alert(1)"}), json!({"url":"file:///etc/passwd"}), json!({"url":"https://user:pass@example.com"}), json!({"url":"https://example.com/ x"}), json!({"url":format!("https://example.com/{}", "中".repeat(700))}), json!({"url":"https://example.com", "code":"evil"})] {
        assert_eq!(call_tool(&ctx,"browser_open",&args)["error"]["code"],"INVALID_ARGUMENT");
    }
    assert_eq!(call_tool(&ctx,"browser_open",&json!({"url":"https://example.com"}))["error"]["code"],"BROWSER_NOT_CONNECTED");
}
