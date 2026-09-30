//! Contract fixture for pane routing. Does not connect to the user's Herdr.
#![cfg(unix)]
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use tempfile::tempdir;
use uuid::Uuid;

fn executable(path: &std::path::Path, source: &str) {
    fs::write(path, source).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn default_command_follows_exact_upper_pane_not_focused_pane_or_stale_thread_env() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let session = Uuid::new_v4();
    let path = dir.path().join(format!("rollout-{session}.jsonl"));
    fs::write(&path,format!("{}\n{}\n",json!({"type":"session_meta","payload":{"id":session,"cwd":dir.path()}}),json!({"type":"event_msg","payload":{"type":"plan_update","plan":[{"step":"실제 위 pane 계획","status":"completed"}]}}))).unwrap();
    let calls = dir.path().join("calls.jsonl");
    executable(
        &bin.join("herdr"),
        r#"#!/usr/bin/env python3
import os,sys,json
a=sys.argv[1:]
with open(os.environ['AP_TEST_CALLS'],'a') as f:f.write(json.dumps(a)+'\n')
if a==['pane','neighbor','--direction','up','--pane','w1:p2']:
 result={'neighbor':{'neighbor_pane_id':None if os.environ.get('AP_TEST_AGENT_BELOW') else 'w1:p1'}}
elif a==['pane','neighbor','--direction','down','--pane','w1:p2']:
 result={'neighbor':{'neighbor_pane_id':'w1:p1'}} if os.environ.get('AP_TEST_AGENT_BELOW') else {'neighbor':{'neighbor_pane_id':None}}
elif a==['agent','get','w1:p1']:
 result={'agent':{'agent':'codex','terminal_id':'fixed','agent_status':'working'}}
elif a==['pane','process-info','--pane','w1:p1']:
 result={'process_info':{'foreground_processes':[{'name':'codex','pid':123}]}}
elif a==['pane','get','w1:p1']:
 result={'pane':{'workspace_id':'w1','tab_id':'w1:t1'}}
elif a==['pane','get','w1:p3']:
 result={'pane':{'terminal_id':'progress-terminal'}}
elif a==['pane','process-info','--pane','w1:p3']:
 result={'process_info':{'foreground_processes':[{'name':'zsh','pid':999}]}}
elif a==['tab','list','--workspace','w1']:
 result={} if os.environ.get('AP_TEST_UNKNOWN_TABS') else {'tabs':[{'tab_id':'w1:t1','label':os.environ.get('AP_TEST_TAB_LABEL','work')}]}
elif a[:2]==['pane','split']:
 expected='0.60' if os.environ.get('AP_TEST_SIZE')=='40' else '0.70'
 assert a[2:8]==['--pane','w1:p1','--direction','down','--ratio',expected] and a[-1]=='--no-focus'
 result={'pane':{'pane_id':'w1:p3'}}
elif a==['pane','layout','--pane','w1:p1']:
 calls=[json.loads(x) for x in open(os.environ['AP_TEST_CALLS'])]
 split_at=max((i for i,x in enumerate(calls) if x[:2]==['pane','split']),default=-1)
 calls=calls[split_at+1:]
 swapped=any(x[:2]==['pane','swap'] for x in calls)
 ratio=.9 if os.environ.get('AP_TEST_NARROW') else (.6 if os.environ.get('AP_TEST_SIZE')=='40' else .7)
 for x in calls:
  if x[:2]==['pane','resize']:
   step=min(.5,float(x[x.index('--amount')+1]))
   ratio+=step if x[x.index('--direction')+1]=='down' else -step
 top=round(ratio*40)
 source={'x':0,'y':top if swapped else 0,'width':80,'height':40-top if swapped else top}
 observer={'x':0,'y':0 if swapped else top,'width':80,'height':top if swapped else 40-top}
 result={'layout':{'focused_pane_id':'w1:p1','panes':[{'pane_id':'w1:p1','rect':source},{'pane_id':'w1:p3','rect':observer}],'splits':[{'direction':'down','rect':{'x':0,'y':0,'width':80,'height':40},'ratio':ratio}]}}
elif a[:2] in [['pane','swap'],['pane','resize']]:result={}
elif a[:3]==['pane','run','w1:p3']:
 assert "follow --pane 'w1:p1' --session " in a[3]
 result={}
else:sys.exit(2)
print(json.dumps({'result':result}))
"#,
    );
    executable(
        &bin.join("lsof"),
        r#"#!/usr/bin/env python3
import os
print('n'+os.environ['AP_TEST_ROLLOUT'])
"#,
    );
    let out = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["follow", "--once"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("HERDR_PANE_ID", "w1:p2")
        .env("CODEX_THREAD_ID", Uuid::new_v4().to_string())
        .env("CODEX_HOME", dir.path())
        .env("AP_TEST_CALLS", &calls)
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["session"], session.to_string());
    assert_eq!(value["entries"][0]["state"], "Done");
    let above = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["follow", "--once"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("HERDR_PANE_ID", "w1:p2")
        .env("AP_TEST_AGENT_BELOW", "1")
        .env("CODEX_HOME", dir.path())
        .env("AP_TEST_CALLS", &calls)
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        above.status.success(),
        "{}",
        String::from_utf8_lossy(&above.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&above.stdout).unwrap()["session"],
        session.to_string()
    );
    let calls_text = fs::read_to_string(&calls).unwrap();
    assert!(!calls_text.contains("--current"));
    assert!(
        !dir.path().join(".agent-progress").exists(),
        "--once must not write state"
    );
    let opened = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["open", "--pane", "w1:p1"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("AP_TEST_CALLS", dir.path().join("calls.jsonl"))
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        opened.status.success(),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    assert!(String::from_utf8_lossy(&opened.stdout).contains("w1:p3 ← w1:p1"));
    let live_size = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["config", "set", "--pane-size", "40"])
        .current_dir(dir.path())
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("HERDR_PANE_ID", "w1:p1")
        .env("AP_TEST_CALLS", &calls)
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        live_size.status.success(),
        "{}",
        String::from_utf8_lossy(&live_size.stderr)
    );
    let requests: Vec<Value> = fs::read_to_string(&calls)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(requests.iter().any(|args| {
        args[0] == "pane"
            && args[1] == "resize"
            && args
                .as_array()
                .is_some_and(|parts| parts.iter().any(|value| value == "up"))
    }));
    let minimum = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["config", "set", "--pane-size", "10"])
        .current_dir(dir.path())
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("HERDR_PANE_ID", "w1:p1")
        .env("AP_TEST_CALLS", &calls)
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        minimum.status.success(),
        "{}",
        String::from_utf8_lossy(&minimum.stderr)
    );
    assert_eq!(
        agent_progress::settings::load(dir.path())
            .unwrap()
            .pane_size_percent,
        10
    );
    // Saved from another terminal: no foreign pane resize; the next opening uses it.
    assert!(
        Command::new(env!("CARGO_BIN_EXE_ap"))
            .args(["config", "set", "--pane-size", "40"])
            .current_dir(dir.path())
            .env("HERDR_ENV", "0")
            .output()
            .unwrap()
            .status
            .success()
    );
    // New top placement uses the same explicit source with the inverse split ratio.
    fs::remove_file(
        dir.path()
            .join(format!(".agent-progress/window-{session}.json")),
    )
    .unwrap();
    let top = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["open", "--pane", "w1:p1", "--position", "above"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("AP_TEST_TOP", "1")
        .env("AP_TEST_SIZE", "40")
        .env("AP_TEST_CALLS", dir.path().join("calls.jsonl"))
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        top.status.success(),
        "{}; recent mock calls: {:?}",
        String::from_utf8_lossy(&top.stderr),
        fs::read_to_string(&calls)
            .unwrap()
            .lines()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
    );
    let receipt: Value = serde_json::from_slice(
        &fs::read(
            dir.path()
                .join(format!(".agent-progress/window-{session}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["position"], "above");
    fs::remove_file(
        dir.path()
            .join(format!(".agent-progress/window-{session}.json")),
    )
    .unwrap();
    let narrow_calls = dir.path().join("narrow-calls.jsonl");
    let narrow = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["open", "--pane", "w1:p1", "--position", "above"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("AP_TEST_NARROW", "1")
        .env("AP_TEST_SIZE", "40")
        .env("AP_TEST_CALLS", &narrow_calls)
        .env("AP_TEST_ROLLOUT", &path)
        .output()
        .unwrap();
    assert!(
        narrow.status.success(),
        "{}",
        String::from_utf8_lossy(&narrow.stderr)
    );
    let requests: Vec<Value> = fs::read_to_string(narrow_calls)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        requests
            .iter()
            .filter(|args| args[0] == "pane" && args[1] == "resize")
            .count(),
        3
    );
    for (flag, value) in [
        ("AP_TEST_UNKNOWN_TABS", "1"),
        ("AP_TEST_TAB_LABEL", "lobby"),
    ] {
        let guarded_calls = dir.path().join(format!("{flag}.jsonl"));
        let refused = Command::new(env!("CARGO_BIN_EXE_ap"))
            .args(["open", "--pane", "w1:p1"])
            .env(
                "PATH",
                format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
            )
            .env("HERDR_ENV", "1")
            .env("AP_TEST_CALLS", &guarded_calls)
            .env("AP_TEST_ROLLOUT", &path)
            .env(flag, value)
            .output()
            .unwrap();
        assert!(!refused.status.success());
        assert!(!fs::read_to_string(guarded_calls).unwrap().contains("split"));
    }
}

#[test]
fn single_quotes_and_shell_substitutions_are_literal_arguments() {
    let text = "a'$(printf SHOULD_NOT_RUN)`echo no`";
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!(
            "printf '%s' {}",
            agent_progress::herdr::shell_quote(text)
        ))
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(out.stdout).unwrap(), text);
}

#[test]
fn herdr_exact_session_follows_codex_without_an_open_rollout_fd() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let session = Uuid::new_v4();
    let sessions = dir.path().join("sessions/2026/09/25");
    fs::create_dir_all(&sessions).unwrap();
    let rollout = sessions.join(format!("rollout-test-{session}.jsonl"));
    fs::write(
        &rollout,
        format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":session,"cwd":dir.path()}}),
            json!({"type":"event_msg","payload":{"type":"plan_update","plan":[{"step":"정확한 세션 작업","status":"in_progress"}]}})
        ),
    )
    .unwrap();
    executable(
        &bin.join("herdr"),
        r#"#!/usr/bin/env python3
import json,os,sys
a=sys.argv[1:]
if a==['agent','get','w1:p1']:
 result={'agent':{'agent':'codex','terminal_id':'fixed','agent_status':'working','foreground_cwd':os.environ['AP_TEST_CWD'],'agent_session':{'agent':'codex','kind':'id','source':'herdr:codex','value':os.environ['AP_TEST_SESSION']}}}
elif a==['pane','process-info','--pane','w1:p1']:
 result={'process_info':{'foreground_processes':[{'name':'codex','pid':123}]}}
else:sys.exit(2)
print(json.dumps({'result':result}))
"#,
    );
    executable(&bin.join("lsof"), "#!/bin/sh\nexit 0\n");
    let out = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["follow", "--pane", "w1:p1", "--once"])
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
        )
        .env("HERDR_ENV", "1")
        .env("CODEX_HOME", dir.path())
        .env("AP_TEST_CWD", dir.path())
        .env("AP_TEST_SESSION", session.to_string())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["session"], session.to_string());
    assert_eq!(value["entries"][0]["title"], "정확한 세션 작업");
}
