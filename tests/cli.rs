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

fn plans_dir(dir: &Path) -> std::path::PathBuf {
    dir.join(".agent-progress/plans")
}

fn count_suffix(dir: &Path, suffix: &str) -> usize {
    std::fs::read_dir(plans_dir(dir))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(suffix)
        })
        .count()
}

#[test]
fn rapid_archives_never_overwrite_each_other() {
    let dir = tempfile::tempdir().unwrap();
    for n in 0..3 {
        ok(dir.path(), &["add", &format!("plan{n}")]);
        ok(dir.path(), &["new"]);
    }
    assert_eq!(count_suffix(dir.path(), ".archived.json"), 3);
}

#[test]
fn new_plan_starts_with_its_own_history() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "old"]);
    ok(dir.path(), &["done", "1"]);
    ok(dir.path(), &["new"]);
    ok(dir.path(), &["add", "fresh"]);
    let history = ok(dir.path(), &["history"]);
    assert!(!history.contains("done 1"), "{history}");
    assert!(history.contains("add fresh"), "{history}");
    assert_eq!(count_suffix(dir.path(), ".archived.history.jsonl"), 1);
}

#[test]
fn leaving_done_clears_completion_time() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a"]);
    ok(dir.path(), &["done", "1"]);
    assert!(json(dir.path())["items"][0]["done_at"].is_u64());
    ok(dir.path(), &["todo", "1"]);
    assert!(json(dir.path())["items"][0].get("done_at").is_none());
}

#[test]
fn blank_block_reason_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a"]);
    assert!(!ap(dir.path(), &["block", "1", "  "]).status.success());
}

#[test]
fn closed_output_pipe_does_not_panic() {
    use std::io::Read;
    let dir = tempfile::tempdir().unwrap();
    for n in 0..200 {
        ok(dir.path(), &["--no-view", "add", &format!("item {n}")]);
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_ap"))
        .arg("status")
        .current_dir(dir.path())
        .env_remove("HERDR_ENV")
        .env_remove("TMUX_PANE")
        .env_remove("CODEX_THREAD_ID")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    child.wait().unwrap();
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn plan_folder_is_ignored_by_git() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir.path())
            .status()
            .unwrap()
            .success()
    );
    ok(dir.path(), &["add", "a"]);
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".agent-progress/.gitignore")).unwrap(),
        "*\n"
    );
    let out = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn pre_3_2_plan_with_viewer_fields_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a"]);
    let file = plans_dir(dir.path()).join("default.json");
    let mut plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    plan["viewer"] = serde_json::json!({"instance": "old", "pane": "w1:p9", "reused": true});
    plan["view_suppressed"] = true.into();
    std::fs::write(&file, plan.to_string()).unwrap();
    ok(dir.path(), &["done", "1"]);
    let after = json(dir.path());
    assert_eq!(after["items"][0]["state"], "done");
    assert!(
        after.get("viewer").is_none(),
        "viewer state is no longer kept in plans"
    );
}

#[test]
fn close_outside_a_pane_reports_nothing_open() {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["add", "a"]);
    assert!(ok(dir.path(), &["close"]).contains("열린 진행 창이 없습니다"));
}
