use agent_progress::{bridge, connection, live::Feed, project::Project};
use serde_json::json;
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn automatic_window_hook_stdout_is_one_valid_json_document() {
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };
    for opt_out in [false, true] {
        let temp = tempdir().unwrap();
        fixture(temp.path());
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let mock = bin.join("herdr");
        fs::write(&mock,r#"#!/usr/bin/env python3
import os,sys,json
a=sys.argv[1:];root=os.environ['AP_MOCK_ROOT'];pid=int(os.environ['AP_MOCK_PARENT'])
if a==['agent','get','w1:p1']: r={'agent':{'agent':'claude','terminal_id':'source','foreground_cwd':root,'agent_status':'idle'}}
elif a==['pane','process-info','--pane','w1:p1']:r={'process_info':{'foreground_processes':[{'name':'claude','pid':pid}]}}
elif a==['pane','get','w1:p1']:r={'pane':{'workspace_id':'w1','tab_id':'w1:t1','terminal_id':'source'}}
elif a==['tab','list','--workspace','w1']:r={'tabs':[{'tab_id':'w1:t1','label':'work'}]}
elif a[:2]==['pane','split']:r={'pane':{'pane_id':'w1:p2'}}
elif a==['pane','get','w1:p2']:r={'pane':{'terminal_id':'observer'}}
elif a[:3]==['pane','run','w1:p2']:sys.exit(0)
else:sys.exit(2)
print(json.dumps({'result':r}))
"#).unwrap();
        fs::set_permissions(mock, fs::Permissions::from_mode(0o755)).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_ap"));
        command
            .args(["bridge", "--agent", "claude"])
            .env(
                "PATH",
                format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
            )
            .env("HERDR_ENV", "1")
            .env("HERDR_PANE_ID", "w1:p1")
            .env_remove("AP_AUTO_OPEN")
            .env_remove("AP_TERMINAL_SLOT")
            .env("AP_MOCK_ROOT", temp.path())
            .env("AP_MOCK_PARENT", std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if opt_out {
            command.env("AP_AUTO_OPEN", "0");
        }
        let mut empty = command.spawn().unwrap();
        empty.stdin.take().unwrap().write_all(json!({"cwd":temp.path(),"session_id":"stdout-contract","hook_event_name":"SessionStart"}).to_string().as_bytes()).unwrap();
        let empty_output = empty.wait_with_output().unwrap();
        assert!(
            empty_output.status.success(),
            "{}",
            String::from_utf8_lossy(&empty_output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&empty_output.stdout).unwrap()["continue"],
            true
        );
        assert!(
            !fs::read_dir(temp.path().join(".agent-progress"))
                .unwrap()
                .any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("window-"))
        );
        let mut child = command.spawn().unwrap();
        let event = json!({"cwd":temp.path(),"session_id":"stdout-contract","last_assistant_message":"### 진행 계획\n- [x] AP-01 Observe\n- [ ] AP-02 Preserve"});
        child
            .stdin
            .take()
            .unwrap()
            .write_all(event.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["continue"], true);
        let opened = fs::read_dir(temp.path().join(".agent-progress"))
            .unwrap()
            .any(|e| {
                let name = e.unwrap().file_name();
                let name = name.to_string_lossy();
                name.starts_with("window-") && name.ends_with(".json")
            });
        assert_eq!(opened, !opt_out);
    }
}

fn fixture(root: &std::path::Path) {
    fs::write(root.join("ap.project.json"), json!({"schema":1,"project_id":Uuid::new_v4(),"objective":"Shared product","roadmap":"plan.md"}).to_string()).unwrap();
    fs::write(
        root.join("plan.md"),
        "- [ ] AP-01 Observe\n- [ ] AP-02 Preserve\n",
    )
    .unwrap();
}

#[test]
fn explicit_goal_and_plan_survive_bridge_updates_and_resume() {
    for agent in ["claude", "opencode"] {
        let root = tempdir().unwrap();
        let event = |goal: &str, mark: &str| {
            json!({
                "cwd":root.path(),"session_id":"goal-plan-contract",
                "last_assistant_message":format!("unrelated-secret\n### 진행 계획\n목표: {goal}\n- [x] 구조 확인\n- [{mark}] API 구현\n- [ ] 검증")
            })
        };
        let first = bridge::ingest(agent, &event("로그인 완성", ">"), Some(root.path())).unwrap();
        let path = std::path::PathBuf::from(first["rollout"].as_str().unwrap());
        let mut feed = Feed::open(path.clone(), None, None).unwrap();
        feed.refresh_all().unwrap();
        assert_eq!(
            feed.snapshot.goal.as_ref().unwrap().objective,
            "로그인 완성"
        );
        assert_eq!(feed.snapshot.plan_goal.as_deref(), Some("로그인 완성"));
        assert_eq!(feed.snapshot.counts(), (1, 3));
        let id = feed.snapshot.entries[1].id;
        let unchanged = fs::read(&path).unwrap();
        bridge::ingest(agent, &event("로그인 완성", ">"), Some(root.path())).unwrap();
        assert_eq!(unchanged, fs::read(&path).unwrap());
        bridge::ingest(agent, &event("로그인 완성", "x"), Some(root.path())).unwrap();
        feed.refresh_all().unwrap();
        assert_eq!(feed.snapshot.counts(), (2, 3));
        assert_eq!(feed.snapshot.entries[1].id, id);
        bridge::ingest(agent, &event("가입 완성", "x"), Some(root.path())).unwrap();
        let mut resumed = Feed::open(path.clone(), None, None).unwrap();
        resumed.refresh_all().unwrap();
        assert_eq!(resumed.snapshot.session, feed.snapshot.session);
        assert_eq!(
            resumed.snapshot.goal.as_ref().unwrap().objective,
            "가입 완성"
        );
        assert_eq!(resumed.snapshot.counts(), (2, 3));
        assert_eq!(
            resumed
                .snapshot
                .archives
                .last()
                .unwrap()
                .goal
                .as_ref()
                .unwrap()
                .objective,
            "로그인 완성"
        );
        assert!(
            !fs::read_to_string(path)
                .unwrap()
                .contains("unrelated-secret")
        );
    }
}

#[test]
fn shared_daemon_codex_hook_is_nonfatal_and_never_registers_an_unowned_pane() {
    use std::{
        io::Write,
        os::unix::fs::PermissionsExt,
        process::{Command, Stdio},
    };
    let root = tempdir().unwrap();
    let bin = root.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let mock = bin.join("herdr");
    fs::write(
        &mock,
        r#"#!/usr/bin/env python3
import sys,json,os
a=sys.argv[1:]
if a==['agent','get','w1:p1']:
 r={'agent':{'agent':'codex','terminal_id':'unowned','foreground_cwd':os.environ['AP_TEST_ROOT']}}
elif a==['pane','process-info','--pane','w1:p1']:
 r={'process_info':{'foreground_processes':[{'name':'codex','pid':4294967294}]}}
else:sys.exit(2)
print(json.dumps({'result':r}))
"#,
    )
    .unwrap();
    fs::set_permissions(&mock, fs::Permissions::from_mode(0o700)).unwrap();
    let session = Uuid::new_v4();
    let native = Uuid::new_v4().to_string();
    let rollout = root.path().join("rollout.jsonl");
    fs::write(&rollout, format!("{}\n{}\n",json!({"type":"session_meta","payload":{"id":session,"cwd":root.path()}}),json!({"type":"event_msg","payload":{"type":"plan_update","plan":[{"step":"원본 계획","status":"in_progress"}]}}))).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args([
            "bridge",
            "--agent",
            "codex",
            "--root",
            root.path().to_str().unwrap(),
        ])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("HERDR_PANE_ID", "w1:p1")
        .env("AP_TEST_ROOT", root.path())
        .env("CODEX_HOME", root.path())
        .env_remove("AP_TERMINAL_SLOT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(json!({"cwd":root.path(),"session_id":native,"transcript_path":rollout,"hook_event_name":"UserPromptSubmit"}).to_string().as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["continue"],
        true
    );
    let bridge_dir = root.path().join(".agent-progress/bridges");
    assert!(!bridge_dir.join("pane-w1_p1.json").exists());
    assert!(!fs::read_dir(&bridge_dir).unwrap().any(|e| {
        e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("pane-")
    }));
    let key = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("agent-progress:codex:{native}").as_bytes(),
    );
    let status: serde_json::Value = serde_json::from_slice(
        &fs::read(bridge_dir.join(format!("codex-{key}.status.json"))).unwrap(),
    )
    .unwrap();
    assert_eq!(status["result"]["registered_pane"], false);
    assert_eq!(status["result"]["source_identity_unverified"], true);
    assert!(
        status["result"]["pending"]
            .as_str()
            .unwrap()
            .contains("ap launch")
    );
}

#[test]
fn invalid_presentation_does_not_fail_a_native_plan_hook() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let root = tempdir().unwrap();
    fixture(root.path());
    fs::create_dir(root.path().join(".agent-progress")).unwrap();
    let settings = root.path().join(".agent-progress/ui.json");
    fs::write(&settings, "{").unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["bridge", "--agent", "claude"])
        .env("HERDR_ENV", "0")
        .env_remove("AP_TERMINAL_SLOT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(json!({"cwd":root.path(),"session_id":"invalid-theme","last_assistant_message":"### 진행 계획\n- [>] AP-01 Observe"}).to_string().as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["continue"],
        true
    );
    assert_eq!(fs::read_to_string(settings).unwrap(), "{");
}

#[test]
fn client_selection_changes_invalidate_old_binding_even_while_its_transcript_exists() {
    let root = tempdir().unwrap();
    let dir = root.path().join(".agent-progress/bridges");
    fs::create_dir_all(&dir).unwrap();
    let session = Uuid::new_v4();
    let pid = std::process::id();
    let rollout = root.path().join("session.jsonl");
    fs::write(
        &rollout,
        format!(
            "{}\n",
            json!({"type":"session_meta","payload":{"id":session,"cwd":root.path()}})
        ),
    )
    .unwrap();
    let marker = root.path().join("selection.json");
    fs::write(&marker, json!({"session":session,"owner":pid}).to_string()).unwrap();
    let binding = bridge::Registration {
        schema: 3,
        agent: "codex".into(),
        native_session: session.to_string(),
        session,
        rollout,
        cwd: root.path().into(),
        pane: "w1:p1".into(),
        terminal: "fixed".into(),
        pid: pid.into(),
        client_owner: Some(pid),
        client_marker: Some(marker.clone()),
    };
    fs::write(
        dir.join(format!(
            "pane-{}.json",
            agent_progress::recovery::hash(b"w1:p1")
        )),
        serde_json::to_vec(&binding).unwrap(),
    )
    .unwrap();
    assert!(bridge::lookup(root.path(), "w1:p1", "fixed", pid.into(), "codex").is_ok());
    fs::write(
        &marker,
        json!({"session":Uuid::new_v4(),"owner":pid}).to_string(),
    )
    .unwrap();
    assert!(bridge::lookup(root.path(), "w1:p1", "fixed", pid.into(), "codex").is_err());
    fs::remove_file(marker).unwrap();
    assert!(bridge::lookup(root.path(), "w1:p1", "fixed", pid.into(), "codex").is_err());
}

#[test]
fn native_adapters_keep_sessions_separate_and_never_store_unrelated_prose() {
    let temp = tempdir().unwrap();
    fixture(temp.path());
    let event = json!({"cwd":temp.path(),"session_id":"one","last_assistant_message":"secret-not-a-plan\n### 진행 계획\n- [x] AP-01 Observe\n- [>] AP-02 Preserve"});
    let first = bridge::ingest("claude", &event, None).unwrap();
    let path = std::path::PathBuf::from(first["rollout"].as_str().unwrap());
    let bytes = fs::read(&path).unwrap();
    bridge::ingest("claude", &event, None).unwrap();
    assert_eq!(bytes, fs::read(&path).unwrap());
    assert!(
        !String::from_utf8(bytes)
            .unwrap()
            .contains("secret-not-a-plan")
    );
    assert_eq!(
        Project::discover(temp.path())
            .unwrap()
            .unwrap()
            .resume()
            .unwrap()["counts"],
        json!([1, 2])
    );
    let other=bridge::ingest("opencode",&json!({"cwd":temp.path(),"session_id":"one","todos":[{"content":"AP-02 Preserve","status":"in_progress"}]}),None).unwrap();
    assert_ne!(first["session"], other["session"]);
    assert_eq!(
        Project::discover(temp.path())
            .unwrap()
            .unwrap()
            .resume()
            .unwrap()["counts"],
        json!([1, 2])
    );
    let mut feed = Feed::open(path, None, None).unwrap();
    feed.refresh_all().unwrap();
    assert!(feed.snapshot.source.contains("claude"));
    let foreign = tempdir().unwrap();
    fixture(foreign.path());
    assert!(bridge::ingest("claude", &event, Some(foreign.path())).is_err());
}

#[test]
fn native_task_identity_and_status_survive_updates_and_settings_removal_is_narrow() {
    let temp = tempdir().unwrap();
    fixture(temp.path());
    bridge::ingest("claude",&json!({"cwd":temp.path(),"session_id":"tasks","tool_name":"TaskCreate","tool_input":{"subject":"Observe"},"tool_response":{"task":{"id":"1"}}}),None).unwrap();
    let updated=bridge::ingest("claude",&json!({"cwd":temp.path(),"session_id":"tasks","tool_name":"TaskUpdate","tool_input":{"taskId":"1","subject":"Renamed","status":"completed"}}),None).unwrap();
    let mut feed = Feed::open(updated["rollout"].as_str().unwrap().into(), None, None).unwrap();
    feed.refresh_all().unwrap();
    assert_eq!(feed.snapshot.entries.len(), 1);
    assert_eq!(feed.snapshot.entries[0].title, "[CC-1] Renamed");
    for agent in ["claude", "opencode"] {
        connection::manage_agent(temp.path(), "preview", false, agent).unwrap();
        connection::manage_agent(temp.path(), "apply", false, agent).unwrap();
        if agent == "claude" {
            let path = temp.path().join(".claude/settings.local.json");
            let mut settings: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            settings["user_custom"] = json!("retained");
            fs::write(path, settings.to_string()).unwrap();
        }
        connection::manage_agent(temp.path(), "remove", false, agent).unwrap();
    }
    let settings: serde_json::Value =
        serde_json::from_slice(&fs::read(temp.path().join(".claude/settings.local.json")).unwrap())
            .unwrap();
    assert_eq!(settings["user_custom"], "retained");
}
