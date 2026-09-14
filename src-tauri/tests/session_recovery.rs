use coding_tools_mcp_desktop_lib::tools::{ToolContext, call_tool};
use serde_json::{json,Value};
fn run(ctx: &ToolContext,name: &str,args: Value)->Value { call_tool(ctx,name,&args) }
fn payload(value: &Value)->&Value { if value.get("result").is_some() { &value["result"] } else { value } }
#[test]
fn completed_output_and_failure_survive_context_recreation() {
    let work=tempfile::tempdir().unwrap(); let state=tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("job.py"), "import sys\nprint('saved-output')\nprint('saved-error',file=sys.stderr)\nsys.exit(7)\n").unwrap();
    let ctx=ToolContext::for_test(work.path().into(),state.path().into()).unwrap();
    let value=run(&ctx,"exec_command",json!({"cmd":"python3 job.py","yield_time_ms":5000}));
    let value=payload(&value); assert_eq!(value["exit_code"],7,"{value}");
    let id=value["session_id"].as_str().unwrap().to_string();
    let output=value["output_refs"]["stdout"].clone(); drop(ctx);
    let ctx=ToolContext::for_test(work.path().into(),state.path().into()).unwrap();
    let value=run(&ctx,"read_output",json!({"output_ref":output}));
    assert!(payload(&value)["content"].as_str().unwrap().contains("saved-output"),"{value}");
    let value=run(&ctx,"write_stdin",json!({"session_id":id}));
    assert_eq!(payload(&value)["exit_code"],7,"{value}"); assert_eq!(payload(&value)["command_ok"],false);
    let value=run(&ctx,"write_stdin",json!({"session_id":id,"chars":"unsafe"}));
    assert!(value.to_string().contains("SESSION_CLOSED"),"{value}");
    let other=tempfile::tempdir().unwrap();
    let other_ctx=ToolContext::for_test(other.path().into(),state.path().into()).unwrap();
    let value=run(&other_ctx,"read_output",json!({"output_ref":output}));
    assert!(value.to_string().contains("SESSION_NOT_FOUND"));
}
#[test]
fn background_job_can_be_polled_from_a_new_context() {
    let work=tempfile::tempdir().unwrap();let state=tempfile::tempdir().unwrap();
    std::fs::write(work.path().join("job.py"),"import time\nprint('begin',flush=True)\ntime.sleep(0.3)\nprint('end',flush=True)\n").unwrap();
    let ctx=ToolContext::for_test(work.path().into(),state.path().into()).unwrap();
    let value=run(&ctx,"exec_command",json!({"cmd":"python3 job.py","yield_time_ms":0,"timeout_ms":5000}));
    let id=payload(&value)["session_id"].clone();drop(ctx);
    std::thread::sleep(std::time::Duration::from_millis(900));
    let ctx=ToolContext::for_test(work.path().into(),state.path().into()).unwrap();
    let value=run(&ctx,"write_stdin",json!({"session_id":id}));
    assert_eq!(payload(&value)["exit_code"],0,"{value}");assert!(payload(&value)["stdout"].as_str().unwrap().contains("end"));
}
