use std::{
    path::Path,
    process::{Command, Output},
};

fn ap(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(args)
        .current_dir(dir)
        .env_remove("HERDR_ENV")
        .env_remove("HERDR_PANE_ID")
        .env_remove("TMUX_PANE")
        .env_remove("AP_PLAN")
        .env_remove("CODEX_THREAD_ID")
        .env("AP_AUTO_OPEN", "0")
        .output()
        .unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = ap(dir, args);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn json(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&ok(dir, &["status", "--json"])).unwrap()
}

#[test]
fn empty_plan_is_undetermined_not_complete() {
    let dir = tempfile::tempdir().unwrap();
    assert!(ok(dir.path(), &["status"]).contains("미정"));
    ok(dir.path(), &["goal", "목표"]);
    assert!(ok(dir.path(), &["status"]).contains("미정"));
}

#[test]
fn numbers_stay_stable_after_removal() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "하나", "둘", "셋"]);
    ok(dir.path(), &["rm", "1"]);
    ok(dir.path(), &["add", "넷"]);
    ok(dir.path(), &["done", "3"]);
    let plan = json(dir.path());
    let items: Vec<(u64, &str, &str)> = plan["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["id"].as_u64().unwrap(),
                i["title"].as_str().unwrap(),
                i["state"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        items,
        vec![(2, "둘", "todo"), (3, "셋", "done"), (4, "넷", "todo")]
    );
    assert!(!ap(dir.path(), &["done", "1"]).status.success());
}

#[test]
fn select_by_exact_title_and_reject_ambiguity() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "테스트", "배포", "배포"]);
    ok(dir.path(), &["start", "테스트"]);
    assert_eq!(json(dir.path())["items"][0]["state"], "doing");
    assert!(!ap(dir.path(), &["done", "배포"]).status.success());
}

#[test]
fn cancelled_items_leave_the_denominator() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a", "b", "c"]);
    ok(dir.path(), &["done", "1"]);
    ok(dir.path(), &["cancel", "2", "범위 밖"]);
    assert!(ok(dir.path(), &["status"]).contains("1/2 (50%)"));
}

#[test]
fn block_requires_reason_and_records_who_acts() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "배포"]);
    ok(
        dir.path(),
        &["block", "1", "스테이징 키 필요", "--needs", "user"],
    );
    let item = &json(dir.path())["items"][0];
    assert_eq!(item["state"], "blocked");
    assert_eq!(item["needs"], "user");
    assert!(ok(dir.path(), &["status"]).contains("[!] 1. 배포 — 스테이징 키 필요 (필요: user)"));
    ok(dir.path(), &["start", "1"]);
    assert!(json(dir.path())["items"][0].get("reason").is_none());
}

#[test]
fn repeated_done_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a"]);
    ok(dir.path(), &["done", "1"]);
    ok(dir.path(), &["done", "1"]);
    assert!(ok(dir.path(), &["status"]).contains("1/1 (100%)"));
}

#[test]
fn subdirectory_uses_project_root_plan() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    ok(dir.path(), &["add", "a"]);
    ok(&dir.path().join("sub"), &["done", "1"]);
    assert!(ok(dir.path(), &["status"]).contains("1/1"));
}

#[test]
fn named_plans_are_separate_and_new_archives() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["--plan", "x", "add", "a"]);
    ok(dir.path(), &["--plan", "y", "add", "b"]);
    assert!(ok(dir.path(), &["--plan", "x", "status"]).contains("1. a"));
    assert!(!ok(dir.path(), &["--plan", "x", "status"]).contains("b"));
    ok(dir.path(), &["--plan", "x", "new"]);
    assert!(ok(dir.path(), &["--plan", "x", "status"]).contains("미정"));
    let archived = std::fs::read_dir(dir.path().join(".agent-progress/plans"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".archived.json")
        })
        .count();
    assert_eq!(archived, 1);
}

#[test]
fn every_change_appends_history() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a", "b"]);
    ok(dir.path(), &["done", "1"]);
    let history = ok(dir.path(), &["history"]);
    let lines: Vec<serde_json::Value> = history
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[1]["event"], "done 1");
    assert_eq!(lines[1]["progress"], "1/2 (50%)");
}

#[test]
fn concurrent_adds_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let handles: Vec<_> = (0..8)
        .map(|n| {
            let path = dir.path().to_path_buf();
            std::thread::spawn(move || ok(&path, &["add", &format!("item{n}")]))
        })
        .collect();
    handles.into_iter().for_each(|h| drop(h.join().unwrap()));
    let plan = json(dir.path());
    let mut ids: Vec<u64> = plan["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_u64().unwrap())
        .collect();
    ids.sort();
    assert_eq!(ids, (1..=8).collect::<Vec<_>>());
}

#[test]
fn corrupt_file_is_reported_not_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a"]);
    let file = dir.path().join(".agent-progress/plans/default.json");
    std::fs::write(&file, "{broken").unwrap();
    let out = ap(dir.path(), &["done", "1"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("손상"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "{broken");
}
