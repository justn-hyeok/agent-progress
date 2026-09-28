use agent_progress::{
    model::{Evidence, Plan, Status, Task, Verification, now},
    store,
    ui::{self, View},
};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::tempdir;

fn plan() -> Plan {
    Plan::new(
        "프로젝트".into(),
        "여러 세션에서 이어갈 제품 만들기".into(),
        "test",
    )
    .unwrap()
}
fn task() -> Task {
    Task::new(
        "긴 한국어 작업 이름과 완료 조건 확인".into(),
        vec!["동작을 실제 확인한다".into()],
        Verification::Automated,
        vec![],
    )
}
fn done(t: &mut Task) {
    t.evidence.push(Evidence {
        kind: Verification::Automated,
        reference: "cargo test: passed".into(),
        actor: "test".into(),
        recorded_at: now(),
        stale: false,
        code_revision: None,
    });
    t.status = Status::Done;
}

#[test]
fn empty_and_cancelled_are_not_completion() {
    let mut p = plan();
    assert!(p.progress().contains("미정"));
    let mut t = task();
    t.status = Status::Cancelled;
    p.tasks.push(t);
    assert_eq!(p.counts(), (0, 0));
    assert!(p.progress().contains("미정"));
    let mut t = task();
    done(&mut t);
    p.tasks.push(t);
    assert_eq!(p.counts(), (1, 1));
}

#[test]
fn completion_requires_the_right_fresh_evidence() {
    let mut p = plan();
    let mut t = task();
    t.status = Status::Done;
    p.tasks.push(t);
    assert!(p.validate().is_err());
    done(&mut p.tasks[0]);
    assert!(p.validate().is_ok());
    p.tasks[0].evidence[0].stale = true;
    assert!(p.validate().is_err());
    p.tasks[0].evidence[0].stale = false;
    p.tasks[0].evidence[0].kind = Verification::Reported;
    assert!(p.validate().is_err());
}

#[test]
fn dependency_cycle_and_unfinished_prerequisite_are_rejected() {
    let mut p = plan();
    let a = task();
    let mut b = task();
    b.depends_on.push(a.id);
    p.tasks.extend([a, b]);
    assert!(p.validate().is_ok());
    p.tasks[1].status = Status::Active;
    assert!(p.validate().is_err());
    p.tasks[1].status = Status::Planned;
    let b = p.tasks[1].id;
    p.tasks[0].depends_on.push(b);
    assert!(p.validate().is_err());
}

#[test]
fn blocked_task_requires_reason_and_action() {
    let mut p = plan();
    let t = task();
    let id = t.id;
    p.tasks.push(t);
    p.set_status(id, Status::Blocked, "권한 필요", None)
        .unwrap();
    assert!(p.validate().is_err());
    p.tasks[0].next_action = "사용자가 권한 승인".into();
    assert!(p.validate().is_ok());
}

#[test]
fn malformed_unknown_schema_duplicate_ids_and_control_sequences_fail() {
    let p = plan();
    let text = store::encode(&p).unwrap();
    let schema = format!("\"schema_version\": {}", p.schema_version);
    assert!(store::decode(&text.replace(&schema, "\"schema_version\": 99")).is_err());
    assert!(store::decode(&text.replace(&schema, &format!("\"surprise\": 1, {schema}"))).is_err());
    assert!(store::decode(&text[..text.len() - 4]).is_err());
    let mut p = p;
    let t = task();
    p.tasks.extend([t.clone(), t]);
    assert!(p.validate().is_err());
    p.tasks.clear();
    p.goal = "escape\u{1b}[2J".into();
    assert!(p.validate().is_err());
}

#[test]
fn revision_conflict_and_invalid_mutation_preserve_exact_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    store::create(&path, &plan()).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(store::create(&path, &plan()).is_err());
    assert!(store::update(&path, Some(0), |_| panic!("must not run")).is_err());
    assert!(
        store::update(&path, Some(1), |p| {
            p.goal.clear();
            p.record("test", "note", None, "invalid")
        })
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn uncooperative_edit_during_mutation_is_preserved() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    store::create(&path, &plan()).unwrap();
    assert!(
        store::update(&path, None, |p| {
            std::fs::write(&path, "external partial edit").unwrap();
            p.record("test", "note", None, "racing edit")
        })
        .is_err()
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "external partial edit"
    );
}

#[test]
fn lock_contention_fails_without_hanging_and_lock_recovers() {
    use fs2::FileExt;
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    store::create(&path, &plan()).unwrap();
    let lock = std::fs::File::open(dir.path().join("plan.md.ap-lock")).unwrap();
    lock.lock_exclusive().unwrap();
    assert!(store::update(&path, None, |_| panic!("locked mutation ran")).is_err());
    drop(lock);
    let p = store::update(&path, Some(1), |p| {
        p.record("test", "note", None, "after release")
    })
    .unwrap();
    assert_eq!(p.revision, 2);
    assert_eq!(store::read(&path).unwrap().history.len(), 2);
}

#[test]
fn corrupted_file_is_not_replaced() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    std::fs::write(&path, "# incomplete").unwrap();
    assert!(store::update(&path, None, |_| panic!("invalid mutation ran")).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "# incomplete");
}

#[cfg(unix)]
#[test]
fn symlink_writes_are_refused() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("plan.md");
    store::create(&path, &plan()).unwrap();
    let link = dir.path().join("link.md");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(store::update(&link, None, |_| panic!("symlink mutation ran")).is_err());
    assert_eq!(store::read(&path).unwrap().revision, 1);
}

fn screen(p: &Plan, view: &mut View, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::render(f, p, view)).unwrap();
    format!("{:?}", terminal.backend().buffer())
}

#[test]
fn hud_renders_korean_small_large_and_stale_without_panics() {
    let mut p = plan();
    p.tasks.push(task());
    let mut view = View::default();
    let output = screen(&p, &mut view, 80, 24);
    println!("80x24 Korean HUD: {output}");
    assert!(output.contains("알 수 없음"));
    assert!(output.contains("한국어"));
    view.error = Some("file deleted".into());
    let stale = screen(&p, &mut view, 40, 12);
    println!("40x12 stale HUD: {stale}");
    assert!(stale.contains("오래된 상태"));
    for (w, h) in [(1, 1), (20, 5), (24, 8), (40, 12), (120, 40)] {
        screen(&p, &mut view, w, h);
        view.detail = true;
        screen(&p, &mut view, w, h);
        view.detail = false;
    }
}

#[test]
fn hud_search_filter_navigation_and_detail_are_keyboard_driven() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
    let mut p = plan();
    let mut cancelled = task();
    cancelled.status = Status::Cancelled;
    p.tasks.extend([task(), cancelled]);
    let mut v = View::default();
    v.key(key(KeyCode::Char('f')));
    assert_eq!(v.visible(&p).len(), 1);
    v.key(key(KeyCode::Char('/')));
    v.key(key(KeyCode::Char('없')));
    assert_eq!(v.visible(&p).len(), 0);
    v.key(key(KeyCode::Esc));
    v.key(key(KeyCode::Esc));
    assert_eq!(v.visible(&p).len(), 1);
    v.key(key(KeyCode::Enter));
    assert!(v.detail);
    assert!(v.key(key(KeyCode::Char('q'))));
}
