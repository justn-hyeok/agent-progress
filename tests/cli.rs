use agent_progress::{model::Status, store};
use std::{
    path::Path,
    process::{Command, Output},
};
use tempfile::tempdir;

fn call(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ap"))
        .arg("--file")
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}
fn ok(path: &Path, args: &[&str]) -> String {
    let out = call(path, args);
    assert!(
        out.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn init(path: &Path) {
    ok(
        path,
        &["init", "--project", "test", "--goal", "실제 CLI 복귀 흐름"],
    );
}

#[test]
fn real_cli_resume_verify_block_and_invalidate_flow() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("ap/2026-09-24/checklist_001.md");
    init(&path);
    let id = ok(
        &path,
        &[
            "add",
            "저장 구현",
            "--criterion",
            "충돌 시 원본 보존",
            "--require",
            "automated",
            "--reason",
            "제품 범위 구현",
        ],
    );
    let id = id.split_whitespace().next().unwrap();
    ok(
        &path,
        &["bind", id, "--agent", "codex", "--session", "session-one"],
    );
    ok(
        &path,
        &[
            "status",
            id,
            "active",
            "--reason",
            "시작",
            "--next",
            "충돌 테스트",
        ],
    );
    let out = call(&path, &["status", id, "done", "--reason", "완료 주장"]);
    assert!(!out.status.success());
    ok(&path, &["evidence", id, "reported", "에이전트 완료 보고"]);
    assert!(
        !call(
            &path,
            &["status", id, "done", "--reason", "근거 종류 불일치"]
        )
        .status
        .success()
    );
    ok(
        &path,
        &["evidence", id, "automated", "cargo test: passed (fixture)"],
    );
    ok(
        &path,
        &["status", id, "done", "--reason", "지정 근거 기록됨"],
    );
    let child = ok(
        &path,
        &[
            "add",
            "연결 구현",
            "--criterion",
            "세션 유지",
            "--depends-on",
            id,
            "--reason",
            "다음 단계",
        ],
    );
    let child = child.split_whitespace().next().unwrap();
    ok(
        &path,
        &["status", child, "active", "--reason", "선행 작업 완료"],
    );
    ok(
        &path,
        &["bind", id, "--agent", "codex", "--session", "session-two"],
    );
    let show = ok(&path, &["show"]);
    assert!(show.contains("체크율 1/2"));
    assert!(show.contains("알 수 없음"));
    ok(&path, &["invalidate", id, "--reason", "관련 코드 변경"]);
    let p = store::read(&path).unwrap();
    assert_eq!(p.tasks[0].status, Status::Review);
    assert_eq!(p.tasks[1].status, Status::Blocked);
    assert_eq!(p.tasks[0].session.as_ref().unwrap().id, "session-two");
    assert!(p.tasks[0].evidence.iter().all(|e| e.stale));
    assert!(ok(&path, &["history"]).contains("관련 코드 변경"));
    ok(&path, &["validate"]);
}

#[test]
fn cli_invalid_inputs_fail_and_never_overwrite() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    init(&path);
    let original = std::fs::read(&path).unwrap();
    for args in [
        vec!["init", "--project", "other", "--goal", "overwrite"],
        vec!["add", "missing criteria"],
        vec![
            "--expect-revision",
            "0",
            "add",
            "task",
            "--criterion",
            "check",
            "--reason",
            "stale",
        ],
    ] {
        assert!(!call(&path, &args).status.success());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
    assert!(!call(&path, &["watch"]).status.success());
    assert!(call(&path, &["--help"]).status.success());
}

#[test]
fn concurrent_processes_with_same_revision_cannot_both_commit() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    init(&path);
    let spawn = || {
        Command::new(env!("CARGO_BIN_EXE_ap"))
            .arg("--file")
            .arg(&path)
            .args([
                "--expect-revision",
                "1",
                "add",
                "task",
                "--criterion",
                "checked",
                "--reason",
                "concurrent writer",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    let a = spawn();
    let b = spawn();
    let a = a.wait_with_output().unwrap();
    let b = b.wait_with_output().unwrap();
    assert_ne!(a.status.success(), b.status.success());
    let p = store::read(&path).unwrap();
    assert_eq!(p.revision, 2);
    assert_eq!(p.tasks.len(), 1);
}
