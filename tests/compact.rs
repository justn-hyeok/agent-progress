use agent_progress::{
    dashboard::{self, View},
    live::{Goal, Snapshot, StepState},
    project::Project,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::fs;
use tempfile::tempdir;
use unicode_width::UnicodeWidthStr;
use uuid::Uuid;

fn sample() -> Snapshot {
    let mut s = Snapshot::empty(Uuid::new_v4(), "agent-progress".into());
    s.set_goal(
        Goal {
            id: "g".into(),
            objective: "작은 진행 상황 창의 가독성 개선".into(),
            status: "active".into(),
        },
        "",
    );
    s.apply_plan(
        vec![
            ("작은 창 전용 레이아웃 적용".into(), StepState::Done),
            ("짧은 화면·상태별 표시 검증".into(), StepState::Active),
            ("실제 아래 창에 적용".into(), StepState::Pending),
        ],
        "에이전트 계획",
        "2026-09-25T01:00:00Z",
        "",
    )
    .unwrap();
    s
}
fn screen(s: &Snapshot, v: &mut View, w: u16, h: u16) -> String {
    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
    t.draw(|f| dashboard::render(f, s, v)).unwrap();
    let b = t.backend().buffer();
    let mut out = String::new();
    for y in 0..h {
        let mut x = 0;
        while x < w {
            let symbol = b[(x, y)].symbol();
            out.push_str(symbol);
            x += symbol.width().max(1) as u16;
        }
        out.push('\n');
    }
    out
}
fn key(v: &mut View, code: KeyCode) {
    v.key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn five_rows_explain_goal_current_work_and_next_without_navigation() {
    let mut s = sample();
    let mut v = View::default();
    let out = screen(&s, &mut v, 133, 5);
    assert!(out.contains("33%"));
    assert!(out.contains(&s.entries[1].title));
    assert!(!out.contains(&s.entries[0].title));
    assert!(out.contains(&s.entries[2].title));
    assert!(out.contains("현재 계획"));
    assert_eq!(out.lines().filter(|row| !row.trim().is_empty()).count(), 5);
    assert!(!out.contains("UTC") && !out.contains("q 종료") && !out.contains("체크리스트"));
    assert_eq!(v.selected, 1);
    let mut paused = s.clone();
    paused.goal.as_mut().unwrap().status = "paused".into();
    let paused_screen = screen(&paused, &mut View::default(), 133, 5);
    assert!(paused_screen.contains("목표 일시 중지"));
    assert!(paused_screen.contains(&s.entries[1].title));
    for step in &mut s.entries {
        step.state = StepState::Done;
    }
    let done = screen(&s, &mut v, 133, 5);
    assert!(done.contains("100%"));
    assert!(done.contains(&s.goal.as_ref().unwrap().objective));
    assert!(done.contains("최근 완료 계획"));
    assert!(!done.contains("대기"));
    println!("133x5 active\n{out}133x5 done\n{done}");
}

#[test]
fn actual_product_five_rows_keep_sources_short_and_avoid_repeated_next_task() {
    let root = tempdir().unwrap();
    let id = Uuid::new_v4();
    fs::write(
        root.path().join("ap.project.json"),
        serde_json::json!({"schema":1,"project_id":id,"objective":"제품 전체 완성","roadmap":"docs/product-plan.md"}).to_string(),
    )
    .unwrap();
    fs::create_dir(root.path().join("docs")).unwrap();
    fs::write(
        root.path().join("docs/product-plan.md"),
        "- [x] AP-01 첫 작업\n- [ ] AP-02 다음 작업\n",
    )
    .unwrap();
    let mut source = Snapshot::empty(Uuid::new_v4(), root.path().display().to_string());
    source.plan_goal = Some(
        "진행 창에서 목표와 계획과 현재 작업을 바로 파악할 수 있도록 아주 긴 계획 설명을 작성한다"
            .into(),
    );
    source
        .apply_plan(
            vec![("[AP-01] 첫 작업".into(), StepState::Done)],
            "에이전트 진행 계획",
            "2026-09-26T00:00:00Z",
            "",
        )
        .unwrap();
    let mut snapshot = Project::discover(root.path())
        .unwrap()
        .unwrap()
        .project(&source)
        .unwrap();
    snapshot.warning = Some("goal 저장소를 읽을 수 없음 · plan은 계속 표시".into());
    let output = screen(&snapshot, &mut View::default(), 159, 5);
    let rows: Vec<_> = output.lines().collect();
    assert!(rows[0].contains("제품 전체 완성") && rows[0].contains("50% · 1/2"));
    assert!(rows[1].contains("최근 완료 계획") && rows[1].contains("아주 긴 계획"));
    assert!(rows[1].trim_end().width() <= 112);
    assert!(rows[0].find("50%").unwrap() < 60);
    assert!(rows[2].contains("AP-02"));
    assert!(!rows[2].contains("goal 저장소"));
    assert!(rows[3].trim().is_empty());
    assert!(rows[4].contains("docs/product-plan.md") && rows[4].contains("ap.project.json"));
    assert!(!output.contains(&root.path().display().to_string()));
    assert!(screen(&snapshot, &mut View::default(), 159, 8).contains("goal 확인 불가"));
}

#[test]
fn compact_selection_detail_help_and_filters_still_work() {
    let s = sample();
    let mut v = View::default();
    screen(&s, &mut v, 40, 5);
    key(&mut v, KeyCode::Up);
    screen(&s, &mut v, 40, 5);
    assert_eq!(v.selected, 0);
    key(&mut v, KeyCode::Enter);
    let detail = screen(&s, &mut v, 40, 5);
    assert!(detail.contains("작은 창 전용 레이아웃 적용"));
    key(&mut v, KeyCode::Enter);
    screen(&s, &mut v, 40, 5);
    assert_eq!(v.selected, 0);
    key(&mut v, KeyCode::Esc);
    screen(&s, &mut v, 40, 5);
    assert_eq!(v.selected, 1);
    key(&mut v, KeyCode::Char('?'));
    assert!(screen(&s, &mut v, 40, 5).contains("도움말"));
    key(&mut v, KeyCode::Esc);
    assert!(!v.help);
    key(&mut v, KeyCode::Char('f'));
    let filtered = screen(&s, &mut v, 40, 5);
    assert!(filtered.contains("미완료"));
    assert!(!filtered.contains("✓"));
}

#[test]
fn product_dashboard_keeps_the_agent_goal_and_checklist_visible() {
    let root = tempdir().unwrap();
    fs::write(
        root.path().join("ap.project.json"),
        serde_json::json!({
            "schema":1,"project_id":Uuid::new_v4(),"objective":"제품 전체 목표","roadmap":"plan.md"
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        root.path().join("plan.md"),
        "- [ ] AP-01 로드맵 작업\n- [ ] AP-02 후속 제품 작업\n",
    )
    .unwrap();
    let mut source = Snapshot::empty(Uuid::new_v4(), root.path().display().to_string());
    source.set_goal(
        Goal {
            id: "native-goal".into(),
            objective: "세션의 실제 목표".into(),
            status: "active".into(),
        },
        "",
    );
    source
        .apply_plan(
            vec![
                ("실제 구조 확인".into(), StepState::Done),
                ("실제 구현".into(), StepState::Active),
                ("실제 검증".into(), StepState::Pending),
            ],
            "에이전트 계획",
            "now",
            "",
        )
        .unwrap();
    let projected = Project::discover(root.path())
        .unwrap()
        .unwrap()
        .project(&source)
        .unwrap();
    let original = serde_json::to_vec(&projected).unwrap();
    let output = screen(&projected, &mut View::default(), 100, 14);
    assert!(output.contains("제품 전체 목표"));
    assert!(output.contains("0% · 0/2"));
    assert!(output.contains("세션의 실제 목표"), "{output}");
    assert!(output.contains("세션 체크리스트 · 1/3"), "{output}");
    for title in ["실제 구조 확인", "실제 구현", "실제 검증"] {
        assert!(output.contains(title), "{output}");
    }
    assert_eq!(serde_json::to_vec(&projected).unwrap(), original);
}

#[test]
fn compact_variants_keep_warning_and_progress_and_fit_cjk_glyphs() {
    let s = sample();
    for (w, h) in [
        (1, 1),
        (20, 2),
        (24, 3),
        (40, 5),
        (60, 7),
        (133, 5),
        (80, 10),
        (40, 12),
        (80, 14),
    ] {
        let mut v = View::default();
        let out = screen(&s, &mut v, w, h);
        assert!(out.lines().all(|row| row.width() <= w as usize));
        if w >= 24 {
            assert!(out.contains("33%"));
        }
        v.error = Some("연결 끊김".into());
        let stale = screen(&s, &mut v, w, h);
        if w >= 40 {
            assert!(stale.contains("오래된 상태"));
        }
        if (w, h) == (40, 5) {
            println!("40x5 active\n{out}40x5 disconnected\n{stale}");
        }
    }
    for w in 0..20 {
        let clipped = dashboard::ellipsize("한국어👩‍💻와 긴 제목", w);
        assert!(clipped.width() <= w);
    }
}

#[test]
fn progress_bar_fill_tracks_total_checklist_and_blocked_state_is_explicit() {
    use ratatui::style::Color;
    let s = sample();
    let mut view = View::default();
    let mut terminal = Terminal::new(TestBackend::new(80, 5)).unwrap();
    terminal
        .draw(|f| dashboard::render(f, &s, &mut view))
        .unwrap();
    let colored = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .filter(|c| c.bg == Color::Rgb(31, 39, 27))
        .count();
    let remaining = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .filter(|c| c.bg == Color::Rgb(15, 18, 20))
        .count();
    assert!(colored > 0 && remaining > colored);
    // Wide-character continuation cells aren't independently drawn by the backend.
    // Check every visible glyph/space, including empty rows and pane edges.
    let buffer = terminal.backend().buffer();
    for y in 0..5 {
        let mut x = 0;
        while x < 80 {
            let cell = &buffer[(x, y)];
            assert!(matches!(
                cell.bg,
                Color::Rgb(31, 39, 27) | Color::Rgb(15, 18, 20)
            ));
            x += cell.symbol().width().max(1) as u16;
        }
    }
    assert!(
        (0..80)
            .filter(|x| buffer[(*x, 0)].bg == Color::Rgb(31, 39, 27))
            .count()
            > 0
    );
    view.connection = Some("입력 대기 · Herdr 관측".into());
    let blocked = screen(&s, &mut view, 80, 5);
    assert!(blocked.contains("사용자 입력 대기") && blocked.contains("33%"));
    let medium = screen(&s, &mut View::default(), 80, 8);
    assert!(medium.contains("33%") && medium.contains("현재 계획"));
}

#[test]
fn compact_empty_plan_is_not_mistaken_for_completed_or_filtered_plan() {
    let s = Snapshot::empty(Uuid::new_v4(), "project".into());
    let mut v = View::default();
    let empty = screen(&s, &mut v, 40, 5);
    assert!(empty.contains("미정"));
    assert!(!empty.contains("100%"));
    let mut s = sample();
    for e in &mut s.entries {
        e.state = StepState::Done;
    }
    v.unfinished = true;
    let complete = screen(&s, &mut v, 40, 5);
    assert!(complete.contains("미완료 항목이 없습니다"));
    assert!(!complete.contains("계획을 작성"));
}

#[test]
fn default_large_product_view_exposes_evidence_history_and_blocker_without_keys() {
    use agent_progress::{
        model::{Status, Verification},
        project::Project,
    };
    use std::fs;
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("ap.project.json"),serde_json::json!({"schema":1,"project_id":uuid::Uuid::new_v4(),"objective":"복귀 테스트","roadmap":"plan.md","required":{"AP-01":"automated"}}).to_string()).unwrap();
    fs::write(root.path().join("plan.md"), "- [ ] AP-01 한국어 작업 🚀\n").unwrap();
    let p = Project::discover(root.path()).unwrap().unwrap();
    p.sync().unwrap();
    p.evidence("AP-01", Verification::Automated, "runner", "result.json")
        .unwrap();
    p.status("AP-01", Status::Blocked, "codex", "사용자 결정 필요")
        .unwrap();
    p.note(
        "AP-01",
        "codex",
        "이전 세션의 결정 보존",
        Some("완료 조건 확인".into()),
    )
    .unwrap();
    let snapshot = p.snapshot().unwrap();
    let large = screen(&snapshot, &mut View::default(), 120, 30);
    for text in [
        "수용 조건",
        "검증",
        "근거",
        "result.json",
        "필요한 행동",
        "완료 조건 확인",
        "최근 이력",
        "사용자 결정 필요",
    ] {
        assert!(large.contains(text), "missing {text}: {large}");
    }
    let small = screen(&snapshot, &mut View::default(), 159, 5);
    assert!(small.contains("사용자 결정 필요"));
    assert!(small.contains("0/1"));
    assert_eq!(snapshot.activity, "활동 미관측");
}
