use crate::{
    model::{Plan, Status},
    store,
};
use anyhow::{Result, ensure};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    io::{self, IsTerminal},
    path::Path,
    time::Duration,
};

pub fn summary(plan: &Plan) -> String {
    let mut lines = vec![
        format!("{} · {} · r{}", plan.project, plan.goal, plan.revision),
        plan.progress(),
        "활동: 알 수 없음 (관측 연동 없음) · 근거 종류는 기록자 선언".into(),
    ];
    for task in &plan.tasks {
        lines.push(format!(
            "[{}] {} {}",
            task.status.label(),
            task.id,
            task.title
        ));
        if let Some(blocker) = &task.blocker {
            lines.push(format!("  막힌 이유: {blocker}"));
        }
        if !task.next_action.is_empty() {
            lines.push(format!("  다음 행동: {}", task.next_action));
        }
    }
    if let Some(last) = plan.history.last() {
        lines.push(format!(
            "마지막 기록: {} · {} · {}",
            last.at, last.actor, last.reason
        ));
    }
    lines.join("\n")
}

#[derive(Default)]
pub struct View {
    pub selected: usize,
    pub query: String,
    pub searching: bool,
    pub unfinished_only: bool,
    pub detail: bool,
    pub scroll: u16,
    pub error: Option<String>,
    list_state: ListState,
}

impl View {
    pub fn visible(&self, plan: &Plan) -> Vec<usize> {
        let query = self.query.to_lowercase();
        plan.tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                (!self.unfinished_only || !matches!(t.status, Status::Done | Status::Cancelled))
                    && (t.title.to_lowercase().contains(&query)
                        || t.id.to_string().contains(&query))
            })
            .map(|(i, _)| i)
            .collect()
    }
    pub fn key(&mut self, key: event::KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return true;
        }
        if self.searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    self.query.pop();
                }
                KeyCode::Char(c) if !c.is_control() => self.query.push(c),
                _ => {}
            }
            self.selected = 0;
            self.scroll = 0;
            return false;
        }
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Esc => {
                self.query.clear();
                self.detail = false;
                self.scroll = 0;
            }
            KeyCode::Char('f') => {
                self.unfinished_only = !self.unfinished_only;
                self.selected = 0;
            }
            KeyCode::Enter => {
                self.detail = !self.detail;
                self.scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail {
                    self.scroll = self.scroll.saturating_add(1);
                } else {
                    self.selected = self.selected.saturating_add(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail {
                    self.scroll = self.scroll.saturating_sub(1);
                } else {
                    self.selected = self.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
        false
    }
}

pub fn render(frame: &mut Frame, plan: &Plan, view: &mut View) {
    let area = frame.area();
    let (done, total) = plan.counts();
    let count = if total == 0 {
        "체크율 미정".to_owned()
    } else {
        format!("체크율 {done}/{total} · 남음 {}", total - done)
    };
    if area.height < 8 || area.width < 24 {
        let warning = view
            .error
            .as_ref()
            .map(|_| "오래된 상태 · 재읽기 실패\n")
            .unwrap_or("");
        frame.render_widget(
            Paragraph::new(format!(
                "{warning}{}\n{count} · 활동 미관측\n{}\nq 종료",
                plan.goal,
                plan.tasks
                    .iter()
                    .find(|t| matches!(t.status, Status::Active | Status::Blocked))
                    .map(|t| format!("[{}] {}", t.status.label(), t.title))
                    .unwrap_or_else(|| "현재 진행 작업 없음".into())
            )),
            area,
        );
        return;
    }
    let sections = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);
    let health = view
        .error
        .as_ref()
        .map(|e| format!("오래된 상태 · 재읽기 실패: {e}"))
        .unwrap_or_else(|| {
            format!(
                "활동: 알 수 없음 · 기록: {}",
                plan.history.last().map(|e| e.at).unwrap_or(0)
            )
        });
    frame.render_widget(
        Paragraph::new(format!(
            "{health}\n{count} · 제품 완성도 아님\n{}",
            plan.goal
        ))
        .block(Block::bordered().title(format!("{} · r{}", plan.project, plan.revision))),
        sections[0],
    );
    let visible = view.visible(plan);
    view.selected = view.selected.min(visible.len().saturating_sub(1));
    if view.detail && !visible.is_empty() {
        let t = &plan.tasks[visible[view.selected]];
        let evidence = t
            .evidence
            .iter()
            .map(|e| {
                format!(
                    "{:?} [{}] {} · {}",
                    e.kind,
                    if e.stale {
                        "재검증 필요"
                    } else {
                        "기록됨"
                    },
                    e.actor,
                    e.reference
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let history = plan
            .history
            .iter()
            .filter(|e| e.task == Some(t.id))
            .map(|e| format!("r{} {}: {}", e.revision, e.action, e.reason))
            .collect::<Vec<_>>()
            .join("\n");
        let text = format!(
            "{}\n{} [{}]\n완료 조건:\n{}\n요구 근거: {:?} (기록자 선언)\n근거:\n{}\n연결: {}\n활동: 알 수 없음\n막힌 이유: {}\n다음 행동: {}\n이력:\n{}",
            t.id,
            t.title,
            t.status.label(),
            t.criteria.join("\n"),
            t.required,
            evidence,
            t.session
                .as_ref()
                .map(|s| format!("{} / {}", s.agent, s.id))
                .unwrap_or_else(|| "미연결".into()),
            t.blocker.as_deref().unwrap_or("없음"),
            t.next_action,
            history
        );
        let mut widget = Paragraph::new(text)
            .block(Block::bordered().title("작업 상세"))
            .wrap(Wrap { trim: false });
        let max_scroll = widget
            .line_count(sections[1].width)
            .saturating_sub(sections[1].height as usize) as u16;
        view.scroll = view.scroll.min(max_scroll);
        widget = widget.scroll((view.scroll, 0));
        frame.render_widget(widget, sections[1]);
    } else {
        let items: Vec<_> = visible
            .iter()
            .map(|i| {
                let t = &plan.tasks[*i];
                let mut line = format!("[{}] {}", t.status.label(), t.title);
                if let Some(blocker) = &t.blocker {
                    line.push_str(&format!("\n  막힘: {blocker}"));
                }
                if !t.next_action.is_empty() {
                    line.push_str(&format!("\n  다음: {}", t.next_action));
                }
                ListItem::new(line)
            })
            .collect();
        view.list_state
            .select((!visible.is_empty()).then_some(view.selected));
        frame.render_stateful_widget(
            List::new(items)
                .block(Block::bordered().title(if visible.is_empty() {
                    "표시할 작업 없음"
                } else {
                    "작업"
                }))
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
                .highlight_symbol("› "),
            sections[1],
            &mut view.list_state,
        );
    }
    frame.render_widget(
        Paragraph::new(format!(
            "q 종료 · ↵ 상세 · / 검색 · f 필터 · ↑↓ 이동\n검색{}: {}",
            if view.searching { " 입력 중" } else { "" },
            view.query
        )),
        sections[2],
    );
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

pub fn watch(path: &Path) -> Result<()> {
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "watch requires a terminal; use show for piped output"
    );
    let mut plan = store::read(path)?;
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    let mut view = View::default();
    loop {
        match store::read(path) {
            Ok(next) => {
                if next.id != plan.id || next.project != plan.project {
                    view.error = Some(
                        "다른 계획으로 파일이 교체됨; 종료 후 명시적으로 다시 연결하세요".into(),
                    );
                } else {
                    plan = next;
                    view.error = None;
                }
            }
            Err(e) => view.error = Some(format!("{e:#}")),
        }
        terminal.draw(|f| render(f, &plan, &mut view))?;
        if event::poll(Duration::from_millis(300))?
            && let Event::Key(key) = event::read()?
            && view.key(key)
        {
            break;
        }
    }
    Ok(())
}
