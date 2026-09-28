use agent_progress::{live::Snapshot, mcp, project::Project};
use serde_json::{Value, json};
use std::{fs, io::Cursor};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn handshake_and_readonly_boundary_do_not_mutate_and_optin_uses_shared_policy() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("ap.project.json"),json!({"schema":1,"project_id":Uuid::new_v4(),"objective":"test","roadmap":"plan.md","required":{"AP-01":"automated"}}).to_string()).unwrap();
    fs::write(root.path().join("plan.md"), "- [ ] AP-01 Work\n").unwrap();
    let p = Project::discover(root.path()).unwrap().unwrap();
    p.project(&Snapshot::empty(
        Uuid::new_v4(),
        root.path().display().to_string(),
    ))
    .unwrap();
    let before = fs::read(p.state_path()).unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"plan_read"}}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"task_status","arguments":{"key":"AP-01","status":"done","reason":"claim"}}}),
    ];
    let stream = messages
        .iter()
        .map(|m| format!("{m}\n"))
        .collect::<String>();
    let mut output = Vec::new();
    mcp::serve(p.clone(), false, Cursor::new(&stream), &mut output).unwrap();
    let out: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(out[0].get("error").is_some());
    assert_eq!(out[2]["result"]["tools"].as_array().unwrap().len(), 3);
    assert_eq!(
        out[3]["result"]["structuredContent"]["tasks"][0]["status"],
        "planned"
    );
    assert_eq!(out[4]["result"]["isError"], true);
    assert_eq!(before, fs::read(p.state_path()).unwrap());
    let mut output = Vec::new();
    mcp::serve(p.clone(), true, Cursor::new(stream), &mut output).unwrap();
    assert_eq!(
        p.plan().unwrap().tasks[0].status,
        agent_progress::model::Status::Review
    );
    let invalid = b"{not json}\n";
    let mut output = Vec::new();
    mcp::serve(p, false, Cursor::new(invalid), &mut output).unwrap();
    let error: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(error["error"]["code"], -32700);
}
