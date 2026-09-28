use agent_progress::{
    dashboard::{self, View},
    live::{self, Feed, Goal, Snapshot, StepState},
};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use tempfile::tempdir;
use uuid::Uuid;

fn append(path: &Path, value: Value) {
    writeln!(
        OpenOptions::new().append(true).open(path).unwrap(),
        "{value}"
    )
    .unwrap();
}
fn fixture(path: &Path, session: Uuid) {
    fs::write(
        path,
        format!(
            "{}\n",
            json!({"type":"session_meta", "payload":{"id":session,"cwd":path.parent().unwrap()}})
        ),
    )
    .unwrap();
}
fn plan(path: &Path, steps: Value) {
    append(
        path,
        json!({"timestamp":"2026-09-24T11:22:33Z", "type":"event_msg","payload":{"type":"plan_update","plan":steps}}),
    );
}
fn step(title: &str, status: &str) -> Value {
    json!({"step":title,"status":status})
}
fn message(path: &Path, role: &str, text: &str) {
    append(
        path,
        json!({"type":"response_item","timestamp":"2026-09-24T11:22:33Z","payload":{"type":"message","role":role,"content":[{"type":"output_text","text":text}]}}),
    );
}

#[test]
fn no_plan_is_unknown_not_zero_or_complete() {
    let s = Snapshot::empty(Uuid::new_v4(), "project".into());
    assert_eq!(s.counts(), (0, 0));
    assert!(!s.plan_seen);
    let output = render(&s, 80, 14);
    assert!(output.contains("체크율 미정"));
    assert!(output.contains("아직 계획이 없습니다"));
}

#[test]
fn exact_session_required_and_user_or_tool_prose_never_becomes_plan() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let id = Uuid::new_v4();
    fixture(&path, id);
    let text = "### 진행 계획\n목표: 잘못된 목표\n- [x] 사용자 예시";
    message(&path, "user", text);
    append(
        &path,
        json!({"type":"response_item","payload":{"type":"function_call_output","output":text}}),
    );
    let mut feed = Feed::open(path.clone(), Some(id), None).unwrap();
    feed.refresh().unwrap();
    assert!(!feed.snapshot.plan_seen);
    assert!(feed.snapshot.goal.is_none());
    assert!(Feed::open(path, Some(Uuid::new_v4()), None).is_err());
}

#[test]
fn explicit_agent_plan_generates_live_checklist_without_registration() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let id = Uuid::new_v4();
    fixture(&path, id);
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 로그인 완성\n- [x] 구조 확인\n- [>] API 구현\n- [ ] 테스트",
    );
    let mut feed = Feed::open(path.clone(), Some(id), None).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (1, 3));
    assert_eq!(feed.snapshot.plan_goal.as_deref(), Some("로그인 완성"));
    assert_eq!(
        feed.snapshot.goal.as_ref().unwrap().objective,
        "로그인 완성"
    );
    let old_id = feed.snapshot.entries[1].id;
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 로그인 완성\n- [x] 구조 확인\n- [x] API 구현\n- [>] 테스트",
    );
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (2, 3));
    assert_eq!(old_id, feed.snapshot.entries[1].id);
    // Reusing an earlier plan text is a real regression, not a forever-deduped message.
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 로그인 완성\n- [x] 구조 확인\n- [>] API 구현\n- [ ] 테스트",
    );
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (1, 3));
}

#[test]
fn failed_tool_calls_do_not_change_plan_and_successful_calls_do() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    fixture(&path, Uuid::new_v4());
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    for (call, result) in [("bad", "Error: invalid plan"), ("good", "Plan updated")] {
        append(
            &path,
            json!({"type":"response_item","payload":{"type":"function_call","name":"update_plan","call_id":call,"arguments":json!({"plan":[step("구현","in_progress")]}).to_string()}}),
        );
        append(
            &path,
            json!({"type":"response_item","payload":{"type":"function_call_output","call_id":call,"output":result}}),
        );
        feed.refresh().unwrap();
        assert_eq!(feed.snapshot.entries.len(), usize::from(call == "good"));
    }
}

#[test]
fn omitted_unfinished_entries_stay_in_denominator_and_completed_ids_survive() {
    let mut s = Snapshot::empty(Uuid::new_v4(), "p".into());
    s.apply_plan(
        vec![
            ("완료".into(), StepState::Done),
            ("할 일".into(), StepState::Pending),
        ],
        "test",
        "1",
        "",
    )
    .unwrap();
    let id = s.entries[0].id;
    s.apply_plan(vec![("추가".into(), StepState::Active)], "test", "2", "")
        .unwrap();
    assert_eq!(s.counts(), (1, 3));
    assert_eq!(s.entries[1].id, id);
    assert!(!s.entries[2].present);
    s.apply_plan(vec![("완료".into(), StepState::Pending)], "test", "3", "")
        .unwrap();
    assert_eq!(s.entries[0].id, id);
    assert!(s.entries[0].was_done);
    assert_eq!(s.counts(), (0, 3));
}

#[test]
fn invalid_plan_does_not_partially_mutate_progress() {
    let mut s = Snapshot::empty(Uuid::new_v4(), "p".into());
    s.apply_plan(vec![("first".into(), StepState::Done)], "test", "1", "")
        .unwrap();
    let before = serde_json::to_string(&s).unwrap();
    assert!(
        s.apply_plan(
            vec![
                ("same".into(), StepState::Done),
                (" same ".into(), StepState::Pending)
            ],
            "test",
            "2",
            ""
        )
        .is_err()
    );
    assert_eq!(serde_json::to_string(&s).unwrap(), before);
}

#[test]
fn new_native_goal_identity_does_not_inherit_completion_even_with_same_title() {
    let mut s = Snapshot::empty(Uuid::new_v4(), "p".into());
    s.set_goal(
        Goal {
            id: "first".into(),
            objective: "same".into(),
            status: "active".into(),
        },
        "1",
    );
    s.apply_plan(vec![("step".into(), StepState::Done)], "native", "1", "")
        .unwrap();
    s.set_goal(
        Goal {
            id: "second".into(),
            objective: "same".into(),
            status: "active".into(),
        },
        "2",
    );
    assert_eq!(s.counts(), (0, 0));
    assert_eq!(s.archives.len(), 1);
}

#[test]
fn split_json_write_waits_then_applies_once_and_corruption_retains_last_good() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    fixture(&path, Uuid::new_v4());
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    let bytes=json!({"type":"event_msg","payload":{"type":"plan_update","plan":[step("실제 변화","completed")]}}).to_string();
    let mut file = OpenOptions::new().append(true).open(&path).unwrap();
    let mid = bytes.len() / 2;
    file.write_all(&bytes.as_bytes()[..mid]).unwrap();
    feed.refresh().unwrap();
    assert!(!feed.snapshot.plan_seen);
    file.write_all(&bytes.as_bytes()[mid..]).unwrap();
    file.write_all(b"\n").unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (1, 1));
    let count = feed.snapshot.changes.len();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.changes.len(), count);
    file.write_all(b"{broken}\n").unwrap();
    assert!(feed.refresh().is_err());
    assert_eq!(feed.snapshot.counts(), (1, 1));
}

#[test]
fn session_deletion_replacement_and_truncation_never_select_other_sessions() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    fixture(&path, Uuid::new_v4());
    plan(&path, json!([step("retained", "completed")]));
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    fs::remove_file(&path).unwrap();
    assert!(feed.refresh().is_err());
    fixture(&path, Uuid::new_v4());
    assert!(feed.refresh().is_err());
    assert_eq!(feed.snapshot.counts(), (1, 1));
}

#[test]
fn goal_database_is_optional_read_only_and_other_thread_is_not_read() {
    let dir = tempdir().unwrap();
    let id = Uuid::new_v4();
    assert!(live::read_goal(dir.path(), id).unwrap().is_none());
    let conn = rusqlite::Connection::open(dir.path().join("goals_1.sqlite")).unwrap();
    conn.execute_batch(
        "CREATE TABLE thread_goals(thread_id TEXT,goal_id TEXT,objective TEXT,status TEXT,created_at_ms INTEGER);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO thread_goals VALUES (?1,'goal','real objective','active',0)",
        [id.to_string()],
    )
    .unwrap();
    assert!(
        live::read_goal(dir.path(), Uuid::new_v4())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        live::read_goal(dir.path(), id).unwrap().unwrap().objective,
        "real objective"
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM thread_goals", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn explicit_plan_goal_is_visible_without_replacing_native_goal() {
    let dir = tempdir().unwrap();
    let id = Uuid::new_v4();
    let path = dir.path().join("session.jsonl");
    fixture(&path, id);
    let conn = rusqlite::Connection::open(dir.path().join("goals_1.sqlite")).unwrap();
    conn.execute_batch("CREATE TABLE thread_goals(thread_id TEXT,goal_id TEXT,objective TEXT,status TEXT,created_at_ms INTEGER);").unwrap();
    conn.execute(
        "INSERT INTO thread_goals VALUES (?1,'native-goal','제품 핵심 연결','active',0)",
        [id.to_string()],
    )
    .unwrap();
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 진행 창 UX 개선\n- [>] 원본 경로 표시",
    );
    let mut feed = Feed::open(path, Some(id), Some(dir.path().into())).unwrap();
    feed.refresh().unwrap();
    assert_eq!(
        feed.snapshot.goal.as_ref().unwrap().objective,
        "제품 핵심 연결"
    );
    assert_eq!(feed.snapshot.plan_goal.as_deref(), Some("진행 창 UX 개선"));
    assert_eq!(feed.snapshot.entries[0].title, "원본 경로 표시");
}

#[test]
fn optional_goal_warning_clears_after_database_recovers() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    fixture(&path, Uuid::new_v4());
    let db = dir.path().join("goals_1.sqlite");
    fs::write(&db, "broken database").unwrap();
    let mut feed = Feed::open(path, None, Some(dir.path().to_owned())).unwrap();
    feed.refresh().unwrap();
    assert!(feed.snapshot.warning.is_some());
    fs::remove_file(db).unwrap();
    feed.refresh().unwrap();
    assert!(feed.snapshot.warning.is_none());
}

#[test]
fn goal_changes_archive_completed_work_and_checkpoint_survives_restart() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let cache = dir.path().join("checkpoint.json");
    fixture(&path, Uuid::new_v4());
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 첫 목표\n- [x] 이전 완료",
    );
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    feed.save_checkpoint(&cache).unwrap();
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 다음 목표\n- [>] 다음 작업",
    );
    let mut resumed = Feed::open(path.clone(), None, None).unwrap();
    resumed.restore_checkpoint(&cache).unwrap();
    resumed.refresh().unwrap();
    assert_eq!(resumed.snapshot.archives.len(), 1);
    assert_eq!(
        resumed.snapshot.archives[0].entries[0].state,
        StepState::Done
    );
    resumed.save_checkpoint(&cache).unwrap();
    assert!(feed.save_checkpoint(&cache).is_err());
    assert_eq!(resumed.snapshot.counts(), (0, 1));
}

#[test]
fn checkpoints_do_not_contain_raw_user_or_tool_output() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("session.jsonl");
    let cache = dir.path().join("checkpoint.json");
    fixture(&path, Uuid::new_v4());
    message(&path, "user", "PRIVATE_SENTINEL");
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: current\n- [ ] legitimate",
    );
    let mut feed = Feed::open(path, None, None).unwrap();
    feed.refresh().unwrap();
    feed.save_checkpoint(&cache).unwrap();
    let text = fs::read_to_string(cache).unwrap();
    assert!(!text.contains("PRIVATE_SENTINEL"));
    assert!(text.contains("legitimate"));
}

fn render(s: &Snapshot, width: u16, height: u16) -> String {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| dashboard::render(f, s, &mut View::default()))
        .unwrap();
    format!("{:?}", terminal.backend().buffer())
}

#[test]
fn dashboard_wraps_korean_and_exposes_counts_in_small_pane() {
    let mut s = Snapshot::empty(Uuid::new_v4(), "/dev/agent-progress".into());
    s.set_goal(
        Goal {
            id: "g".into(),
            objective: "현재 세션의 진행 상황을 자동으로 보여주기".into(),
            status: "active".into(),
        },
        "now",
    );
    s.apply_plan(
        vec![
            ("세션 연결".into(), StepState::Done),
            (
                "긴 한국어 계획 항목을 잘라 버리지 않고 다음 줄까지 읽을 수 있도록 만들기".into(),
                StepState::Active,
            ),
            ("검증".into(), StepState::Pending),
        ],
        "Codex plan",
        "2026-09-24T11:22:33Z",
        "",
    )
    .unwrap();
    for (w, h) in [(1, 1), (20, 5), (40, 12), (80, 14), (120, 18)] {
        render(&s, w, h);
    }
    let screen = render(&s, 40, 12);
    assert!(screen.contains("33%"));
    assert!(screen.contains("세션 연결"));
    let wrapped = dashboard::wrapped("한국어👩‍💻 긴 이름", 6);
    assert!(wrapped.len() > 1);
    assert_eq!(wrapped.concat(), "한국어👩‍💻 긴 이름");
    println!("40x12 progress: {screen}");
}

#[test]
fn review_code_examples_and_foreign_thread_events_are_not_live_plan() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let id = Uuid::new_v4();
    fixture(&path, id);
    message(
        &path,
        "assistant",
        "형식 예시입니다:\n```markdown\n### 진행 계획\n목표: 예시\n- [x] 가짜 완료\n```\n",
    );
    append(
        &path,
        json!({"type":"event_msg","payload":{"type":"plan_update","thread_id":Uuid::new_v4(),"plan":[step("다른 세션","completed")]}}),
    );
    let mut feed = Feed::open(path, None, None).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (0, 0));
}

#[test]
fn review_failed_invalid_plan_call_does_not_poison_following_valid_plan() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    fixture(&path, Uuid::new_v4());
    append(
        &path,
        json!({"type":"response_item","payload":{"type":"function_call","name":"update_plan","call_id":"bad","arguments":"{bad"}}),
    );
    append(
        &path,
        json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"bad","output":"Error: invalid JSON"}}),
    );
    plan(&path, json!([step("정상 계획", "in_progress")]));
    let mut feed = Feed::open(path, None, None).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.entries[0].title, "정상 계획");
}

#[test]
fn review_new_goal_does_not_replay_old_plan_as_its_own_completed_work() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let id = Uuid::new_v4();
    fixture(&path, id);
    plan(&path, json!([step("이전 목표 작업", "completed")])); // 11:22:33Z
    let conn = rusqlite::Connection::open(dir.path().join("goals_1.sqlite")).unwrap();
    conn.execute_batch("CREATE TABLE thread_goals(thread_id TEXT,goal_id TEXT,objective TEXT,status TEXT,created_at_ms INTEGER);").unwrap();
    conn.execute(
        "INSERT INTO thread_goals VALUES (?1,'new','새 목표','active',1790251200000)",
        [id.to_string()],
    )
    .unwrap(); // 2026-09-24 12:00Z
    let mut feed = Feed::open(path, None, Some(dir.path().to_owned())).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.goal.as_ref().unwrap().objective, "새 목표");
    assert_eq!(
        feed.snapshot.counts(),
        (0, 0),
        "old plan cannot be 100% under a new goal"
    );
}

#[test]
fn review_checkpoint_detects_same_inode_same_length_source_rewrite() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let cache = dir.path().join("cache.json");
    fixture(&path, Uuid::new_v4());
    plan(&path, json!([step("old title", "completed")]));
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    feed.save_checkpoint(&cache).unwrap();
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("old title", "new title");
    fs::write(&path, text).unwrap();
    assert!(
        feed.refresh().is_err(),
        "live source rewrite must not look unchanged"
    );
    let mut resumed = Feed::open(path, None, None).unwrap();
    assert!(
        resumed.restore_checkpoint(&cache).is_err(),
        "checkpoint must match source bytes, not just inode and size"
    );
}

#[test]
fn review_checkpoint_valid_json_corruption_is_detected_and_legacy_is_preserved() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let cache = dir.path().join("cache.json");
    fixture(&path, Uuid::new_v4());
    plan(&path, json!([step("current", "pending")]));
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    feed.save_checkpoint(&cache).unwrap();
    let original = fs::read(&cache).unwrap();
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    value["snapshot"]["entries"][0]["state"] = "Done".into();
    fs::write(&cache, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        Feed::open(path.clone(), None, None)
            .unwrap()
            .restore_checkpoint(&cache)
            .is_err()
    );
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    for key in ["format_version", "state_hash", "prefix_hash", "goal_epoch"] {
        value.as_object_mut().unwrap().remove(key);
    }
    let legacy = serde_json::to_vec(&value).unwrap();
    fs::write(&cache, &legacy).unwrap();
    let mut resumed = Feed::open(path, None, None).unwrap();
    resumed.restore_checkpoint(&cache).unwrap();
    resumed.refresh_all().unwrap();
    resumed.save_checkpoint(&cache).unwrap();
    let backup = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().contains("legacy-"))
        .unwrap();
    assert_eq!(fs::read(backup.path()).unwrap(), legacy);
    assert_eq!(resumed.snapshot.counts(), (0, 1));
}

#[test]
fn version_one_checkpoint_replays_plan_goal_and_preserves_prior_bytes() {
    let dir = tempdir().unwrap();
    let id = Uuid::new_v4();
    let path = dir.path().join("rollout.jsonl");
    let cache = dir.path().join("cache.json");
    fixture(&path, id);
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 현재 UX 작업\n- [>] 화면 표시",
    );
    let mut feed = Feed::open(path.clone(), Some(id), None).unwrap();
    feed.refresh_all().unwrap();
    feed.save_checkpoint(&cache).unwrap();
    let mut old: Value = serde_json::from_slice(&fs::read(&cache).unwrap()).unwrap();
    old["format_version"] = 1.into();
    old["snapshot"].as_object_mut().unwrap().remove("plan_goal");
    old.as_object_mut().unwrap().remove("state_hash");
    let hash = Uuid::new_v5(&Uuid::NAMESPACE_OID, &serde_json::to_vec(&old).unwrap());
    old["state_hash"] = hash.to_string().into();
    let old_bytes = serde_json::to_vec(&old).unwrap();
    fs::write(&cache, &old_bytes).unwrap();

    let mut resumed = Feed::open(path, Some(id), None).unwrap();
    resumed.restore_checkpoint(&cache).unwrap();
    resumed.refresh_all().unwrap();
    assert_eq!(resumed.snapshot.plan_goal.as_deref(), Some("현재 UX 작업"));
    resumed.save_checkpoint(&cache).unwrap();
    let backup = fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().contains("legacy-"))
        .unwrap();
    assert_eq!(fs::read(backup.path()).unwrap(), old_bytes);
    let upgraded: Value = serde_json::from_slice(&fs::read(cache).unwrap()).unwrap();
    assert_eq!(upgraded["format_version"], 2);
}

#[test]
fn review_native_plan_does_not_disable_new_goal_in_later_explicit_plan() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    fixture(&path, Uuid::new_v4());
    plan(&path, json!([step("old", "completed")]));
    message(
        &path,
        "assistant",
        "### 진행 계획\n목표: 새 작업\n- [>] new",
    );
    let mut feed = Feed::open(path, None, None).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (0, 1));
    assert_eq!(feed.snapshot.entries[0].title, "new");
    assert_eq!(feed.snapshot.archives.len(), 1);
}

#[test]
fn review_large_rollout_once_does_not_silently_omit_latest_plan() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    fixture(&path, Uuid::new_v4());
    let ignored = json!({"type":"ignored", "payload":"x".repeat(1024*1024)});
    for _ in 0..17 {
        append(&path, ignored.clone());
    }
    plan(&path, json!([step("latest", "completed")]));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["follow", "--rollout"])
        .arg(&path)
        .arg("--codex-home")
        .arg(dir.path())
        .arg("--once")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(result["entries"][0]["title"], "latest");
}

#[test]
fn review_narrow_view_keeps_percent_and_error_visible_with_long_project_name() {
    let mut s = Snapshot::empty(Uuid::new_v4(), "긴프로젝트이름".repeat(20));
    s.apply_plan(
        vec![
            ("a".into(), StepState::Done),
            ("b".into(), StepState::Active),
        ],
        "native",
        "",
        "",
    )
    .unwrap();
    s.last_plan_at = "2026-09-24T20:22:33+09:00".into();
    assert!(render(&s, 80, 14).contains("11:22:33 UTC"));
    let screen = render(&s, 24, 12);
    assert!(screen.contains("50%"));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 12)).unwrap();
    let mut view = View::default();
    view.error = Some("connection lost".into());
    terminal
        .draw(|f| dashboard::render(f, &s, &mut view))
        .unwrap();
    let first_row = format!("{:?}", terminal.backend().buffer())
        .lines()
        .find(|s| s.trim_start().starts_with('"'))
        .unwrap()
        .to_owned();
    assert!(first_row.contains("오래된 상태"));
}

#[test]
fn review_same_cursor_concurrent_readers_cannot_overwrite_newer_goal_history() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let cache = dir.path().join("cache.json");
    fixture(&path, Uuid::new_v4());
    let mut first = Feed::open(path.clone(), None, None).unwrap();
    first.refresh().unwrap();
    first.save_checkpoint(&cache).unwrap();
    let mut second = Feed::open(path, None, None).unwrap();
    second.restore_checkpoint(&cache).unwrap();
    first
        .snapshot
        .change("now", "first writer goal update".into());
    first.save_checkpoint(&cache).unwrap();
    let before = fs::read(&cache).unwrap();
    second
        .snapshot
        .change("now", "second writer stale update".into());
    assert!(second.save_checkpoint(&cache).is_err());
    assert_eq!(fs::read(cache).unwrap(), before);
}

#[test]
fn native_plan_item_is_read_and_unstructured_plan_does_not_poison_later_updates() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    fixture(&path, Uuid::new_v4());
    append(
        &path,
        json!({"type":"event_msg","timestamp":"2026-09-25T00:00:00Z","payload":{"type":"item_completed","item":{"type":"Plan","text":"A free prose plan"}}}),
    );
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    assert!(feed.snapshot.warning.is_some());
    append(
        &path,
        json!({"type":"event_msg","timestamp":"2026-09-25T01:00:00Z","payload":{"type":"item_completed","item":{"type":"Plan","text":"# Plan\n- [x] ROOT-01 native item\n- [>] ROOT-02 next item"}}}),
    );
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (1, 2));
    assert_eq!(feed.snapshot.source, "Codex native Plan");
    assert!(feed.snapshot.warning.is_none());
}

#[test]
fn execution_report_advances_native_plan_and_code_examples_do_not() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    fixture(&path, Uuid::new_v4());
    append(
        &path,
        json!({"type":"event_msg","timestamp":"2026-09-25T00:00:00Z","payload":{"type":"item_completed","item":{"type":"Plan","text":"- [ ] AP-01 Native step\n```md\n- [x] EXAMPLE-01 Not a task\n```"}}}),
    );
    let mut feed = Feed::open(path.clone(), None, None).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (0, 1));
    message(&path, "assistant", "### 진행 계획\n- [x] AP-01 Native step");
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (1, 1));
    assert!(feed.snapshot.source.contains("진행 보고"));
}

#[test]
fn explicit_id_rename_preserves_session_identity_and_duplicate_id_is_rejected() {
    let mut snapshot = Snapshot::empty(Uuid::new_v4(), "project".into());
    snapshot
        .apply_plan(
            vec![("[AP-01/native] old label".into(), StepState::Done)],
            "test",
            "2026-09-25T00:00:00Z",
            "",
        )
        .unwrap();
    let id = snapshot.entries[0].id;
    snapshot
        .apply_plan(
            vec![("[AP-01/native] renamed label".into(), StepState::Active)],
            "test",
            "2026-09-25T01:00:00Z",
            "rename",
        )
        .unwrap();
    assert_eq!(snapshot.entries.len(), 1);
    assert_eq!(snapshot.entries[0].id, id);
    assert!(snapshot.entries[0].was_done);
    assert!(
        snapshot
            .apply_plan(
                vec![
                    ("AP-01 One".into(), StepState::Done),
                    ("AP-01 Two".into(), StepState::Pending)
                ],
                "test",
                "now",
                ""
            )
            .is_err()
    );
    assert_eq!(snapshot.entries[0].id, id);
    let mut korean = Snapshot::empty(Uuid::new_v4(), "project".into());
    korean
        .apply_plan(
            vec![("[AP-02/통합] 처음 이름".into(), StepState::Active)],
            "test",
            "old",
            "",
        )
        .unwrap();
    let id = korean.entries[0].id;
    korean
        .apply_plan(
            vec![("[AP-02/통합] 바뀐 이름".into(), StepState::Done)],
            "test",
            "new",
            "",
        )
        .unwrap();
    assert_eq!(korean.entries.len(), 1);
    assert_eq!(korean.entries[0].id, id);
}
