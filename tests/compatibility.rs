#![cfg(unix)]
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, process::Command};
use tempfile::tempdir;

fn mock(dir: &std::path::Path) {
    let path = dir.join("codex");
    fs::write(&path, r#"#!/usr/bin/python3
import sys,json,os,pathlib
a=sys.argv[1:]
if a==['--version']: print('codex-cli '+os.environ.get('MOCK_VERSION','1'))
elif a==['--help']: print('resume app-server'+(' --remote' if os.environ.get('MOCK_BROKEN')!='1' else ''))
elif a[:2]==['app-server','generate-json-schema']:
 p=pathlib.Path(a[a.index('--out')+1])/'v2';p.mkdir()
 for name in ['ThreadStartResponse','ThreadResumeResponse','ThreadForkResponse']:
  (p/(name+'.json')).write_text(json.dumps({'thread':{'cwd':{},'id':{},'path':{}}}))
elif a[0]=='exec':
 if os.environ.get('MOCK_FAILURE')=='1': print('PRIVATE-credential-should-not-escape',file=sys.stderr);sys.exit(1)
 print(json.dumps({'type':'item.completed','item':{'type':'agent_message','text':'### 진행 계획\n- [x] [AP-01] Compatibility observed\n- [ ] [AP-02] Next action'}}))
else: print(json.dumps({'native_args':a}))
"#).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn public_checker_catches_interface_breaks_and_native_failures_without_leaking_logs() {
    let dir = tempdir().unwrap();
    mock(dir.path());
    let call = |live: bool, broken: &str, failure: &str| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ap"));
        command.args(["compatibility", "--agent", "codex"]);
        if live {
            command.arg("--live");
        }
        command
            .env("PATH", format!("{}:/usr/bin:/bin", dir.path().display()))
            .env("MOCK_BROKEN", broken)
            .env("MOCK_FAILURE", failure)
            .output()
            .unwrap()
    };
    let output = call(true, "0", "0");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["checks"][0]["evidence"]["counts"], json!([1, 2]));
    assert_eq!(
        result["checks"][0]["evidence"]["native_task_tools"],
        "not_checked"
    );
    assert!(!call(false, "1", "0").status.success());
    let output = call(true, "0", "1");
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("PRIVATE"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE"));
}

#[test]
fn version_change_invalidates_launch_cache_and_warns_without_blocking_native() {
    let dir = tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    mock(&bin);
    let call = |version: &str, broken: &str| {
        Command::new(env!("CARGO_BIN_EXE_ap"))
            .args([
                "launch",
                "--agent",
                "codex",
                "--",
                "--config",
                "user.setting=true",
            ])
            .current_dir(dir.path())
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .env("HERDR_ENV", "1")
            .env("MOCK_VERSION", version)
            .env("MOCK_BROKEN", broken)
            .output()
            .unwrap()
    };
    assert!(call("1", "0").status.success());
    let output = call("2", "0");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("version changed"));
    let output = call("3", "1");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("compatibility failed"));
    let cached: Value = serde_json::from_slice(
        &fs::read(dir.path().join(".agent-progress/compatibility/codex.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cached["status"], "failed");
    assert_eq!(cached["version"], "codex-cli 3");
}
