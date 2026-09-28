use crate::{
    herdr::{self, Binding},
    live::{self, Feed, Snapshot, StepState},
};
use anyhow::{Context, Result, ensure};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};
use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use uuid::Uuid;

#[derive(Default)]
pub struct View {
    pub palette: crate::settings::Palette,
    pub state_palettes:
        std::collections::BTreeMap<crate::settings::BackgroundState, crate::settings::Palette>,
    pub presentation_warning: bool,
    pub selected: usize,
    pub query: String,
    pub searching: bool,
    pub unfinished: bool,
    pub detail: bool,
    pub history: bool,
    pub help: bool,
    pub manual_selection: bool,
    pub scroll: u16,
    pub error: Option<String>,
    pub storage_error: Option<String>,
    pub connection: Option<String>,
    pub pane: Option<String>,
    pub project_choices: Vec<String>,
    list: ListState,
}

pub fn wrapped(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut row = String::new();
        let mut used = 0;
        for g in line.graphemes(true) {
            let n = g.width();
            if used + n > width && !row.is_empty() {
                rows.push(std::mem::take(&mut row));
                used = 0;
            }
            if n <= width {
                row.push_str(g);
                used += n;
            }
        }
        rows.push(row);
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

impl View {
    pub fn visible(&self, s: &Snapshot) -> Vec<usize> {
        let query = self.query.to_lowercase();
        s.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                (!self.unfinished || e.state != StepState::Done)
                    && e.title.to_lowercase().contains(&query)
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
                KeyCode::Enter | KeyCode::Esc => self.searching = false,
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
            KeyCode::Char('?') => {
                self.help = !self.help;
                self.detail = false;
                self.history = false;
                self.scroll = 0;
            }
            KeyCode::Char('/') => {
                self.searching = true;
                self.manual_selection = true;
            }
            KeyCode::Char('f') => {
                self.unfinished = !self.unfinished;
                self.selected = 0;
                self.manual_selection = true;
            }
            KeyCode::Char('h') => {
                self.history = !self.history;
                self.help = false;
                self.detail = false;
                self.scroll = 0;
            }
            KeyCode::Enter => {
                self.detail = !self.detail;
                self.help = false;
                self.history = false;
                self.scroll = 0;
            }
            KeyCode::Esc => {
                self.query.clear();
                self.detail = false;
                self.history = false;
                self.help = false;
                self.manual_selection = false;
                self.scroll = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.detail || self.history || self.help {
                    self.scroll = self.scroll.saturating_add(1);
                } else {
                    self.manual_selection = true;
                    self.selected = self.selected.saturating_add(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.detail || self.history || self.help {
                    self.scroll = self.scroll.saturating_sub(1);
                } else {
                    self.manual_selection = true;
                    self.selected = self.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
        false
    }
}

/// One line per item in the compact overview; full text remains in Enter detail.
pub fn ellipsize(text: &str, width: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut used = 0;
    for g in text.graphemes(true) {
        if used + g.width() > width - 1 {
            break;
        }
        result.push_str(g);
        used += g.width();
    }
    result.push('…');
    result
}

fn source_path_line(label: &str, path: &Path, width: usize) -> String {
    let full = format!("{label} · {}", path.display());
    if full.width() <= width {
        return full;
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    ellipsize(&format!("{label} · {name}"), width)
}

fn compact_task_title(title: &str) -> &str {
    if title.starts_with("[AP-") {
        title.split_once("] ").map_or(title, |(_, rest)| rest)
    } else {
        title
    }
}

fn render_progress_bar(frame: &mut Frame, s: &Snapshot, view: &mut View) {
    let area = frame.area();
    let (done, total) = s.counts();
    let percent = (done * 100).checked_div(total);
    let product = s.overall.as_ref();
    let goal = product
        .map(|p| p.plan.goal.as_str())
        .or_else(|| s.goal.as_ref().map(|g| g.objective.as_str()))
        .unwrap_or("목표 미정");
    let active = s
        .entries
        .iter()
        .position(|e| e.present && e.state == StepState::Active);
    let current_work = product.and_then(|p| p.current_work.as_ref()).or_else(|| {
        s.entries
            .iter()
            .find(|e| e.present && e.state == StepState::Active)
    });
    let blocked = product.and_then(|p| {
        p.plan
            .tasks
            .iter()
            .find(|t| t.status == crate::model::Status::Blocked)
    });
    if let Some(index) = active {
        view.selected = index;
    }
    let activity = view.connection.as_deref().unwrap_or(&s.activity);
    let source_goal = product
        .and_then(|p| p.session_goal.as_ref())
        .or(s.goal.as_ref());
    let (label, color) = if let Some(error) = &view.error {
        (format!("오래된 상태 · {error}"), Color::Yellow)
    } else if view.storage_error.is_some() {
        ("저장 실패 · 다시 시도 중".into(), Color::Yellow)
    } else if let Some(task) = blocked {
        (
            format!(
                "! {} · {}",
                task.title,
                task.blocker.as_deref().unwrap_or("막힌 이유 확인")
            ),
            Color::Yellow,
        )
    } else if activity.contains("입력 대기") || source_goal.is_some_and(|g| g.status == "blocked")
    {
        ("사용자 입력 대기".into(), Color::Yellow)
    } else if source_goal.is_some_and(|g| g.status == "paused") {
        (
            current_work
                .map(|work| format!("목표 일시 중지 · 계획상 진행 · {}", work.title))
                .unwrap_or_else(|| "일시 중지".into()),
            Color::Yellow,
        )
    } else if let Some(warning) = s.warning.as_ref().filter(|warning| {
        !(warning.starts_with("goal 저장소") && product.is_some() && !s.entries.is_empty())
    }) {
        (warning.clone(), Color::Yellow)
    } else if total == 0 {
        ("계획 대기".into(), Color::DarkGray)
    } else if done == total {
        (
            if area.height >= 3 {
                "✓ 완료".into()
            } else {
                format!("✓ {goal} · 완료")
            },
            Color::Green,
        )
    } else if let Some(work) = current_work {
        (format!("▶ {}", work.title), Color::Cyan)
    } else if s
        .entries
        .iter()
        .any(|e| !e.present && e.state != StepState::Done)
    {
        ("! 계획에서 빠진 항목 확인 필요".into(), Color::Yellow)
    } else {
        let next = product
            .and_then(|p| {
                p.session_plan
                    .iter()
                    .find(|e| e.state == StepState::Pending)
            })
            .or_else(|| s.entries.iter().find(|e| e.state == StepState::Pending))
            .map(|e| e.title.as_str())
            .unwrap_or("계획 갱신");
        (format!("다음 · {next}"), Color::Cyan)
    };
    let metric = percent
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| "체크율 미정".into());
    // The whole pane is the track. Text and percentage form one left-aligned group.
    use crate::settings::BackgroundState;
    let background_state = if view.error.is_some() || view.storage_error.is_some() {
        BackgroundState::Error
    } else if blocked.is_some() || source_goal.is_some_and(|g| g.status == "blocked") {
        BackgroundState::Blocked
    } else if source_goal.is_some_and(|g| g.status == "paused") {
        BackgroundState::Paused
    } else if color == Color::Green {
        BackgroundState::Done
    } else if total == 0 {
        BackgroundState::Empty
    } else if color == Color::Yellow {
        BackgroundState::Waiting
    } else if current_work.is_some() {
        BackgroundState::Working
    } else {
        BackgroundState::Waiting
    };
    let palette = view
        .state_palettes
        .get(&background_state)
        .copied()
        .unwrap_or(view.palette);
    let track = palette.track;
    let fill = palette.fill;
    let primary = palette.text;
    let secondary = palette.muted;
    let pad = if area.width >= 24 {
        2
    } else {
        u16::from(area.width > 4)
    };
    let width = area.width.saturating_sub(pad * 2).min(110);
    let product_counts = product.map(|p| {
        let done = p
            .plan
            .tasks
            .iter()
            .filter(|t| t.status == crate::model::Status::Done)
            .count();
        let total = p
            .plan
            .tasks
            .iter()
            .filter(|t| t.status != crate::model::Status::Cancelled)
            .count();
        (done, total)
    });
    let product_metric = product_counts
        .map(|(d, n)| {
            (d * 100)
                .checked_div(n)
                .map(|percent| format!("{percent}% · {d}/{n}"))
                .unwrap_or_else(|| "체크율 미정".into())
        })
        .unwrap_or_else(|| metric.clone());
    let mut session_source = product
        .map(|_| format!("세션 출처 · {}", s.source.trim_end_matches(" → 제품 계획")))
        .unwrap_or_else(|| format!("계획 출처 · {}", s.source));
    if s.warning
        .as_deref()
        .is_some_and(|w| w.starts_with("goal 저장소"))
    {
        session_source.push_str(" · goal 확인 불가");
    }
    let last_done = product
        .and_then(|p| {
            p.session_plan
                .iter()
                .rev()
                .find(|e| e.state == StepState::Done)
        })
        .or_else(|| s.entries.iter().rev().find(|e| e.state == StepState::Done));
    let context = if color == Color::Yellow || label.contains("완료") {
        last_done.map(|e| format!("마지막 완료 · {}", compact_task_title(&e.title)))
    } else {
        product
            .and_then(|p| {
                p.session_plan
                    .iter()
                    .find(|e| e.state == StepState::Pending)
            })
            .or_else(|| s.entries.iter().find(|e| e.state == StepState::Pending))
            .map(|e| format!("다음 · {}", e.title))
            .or_else(|| {
                last_done.map(|e| format!("마지막 완료 · {}", compact_task_title(&e.title)))
            })
    };
    let roadmap = product.map(|p| source_path_line("계획", &p.roadmap, width as usize));
    let manifest = product.map(|p| {
        let path = p
            .manifest
            .strip_prefix(Path::new(&s.project))
            .unwrap_or_else(|_| Path::new(p.manifest.file_name().unwrap_or_default()));
        source_path_line("목표", path, width as usize)
    });
    let status_line = if blocked.is_some()
        || view.error.is_some()
        || view.storage_error.is_some()
        || activity.contains("입력 대기")
        || source_goal.is_some_and(|g| g.status == "blocked" || g.status == "paused")
        || label.contains("완료")
    {
        label.clone()
    } else if let Some(work) = current_work {
        format!("계획상 진행 · {}", work.title)
    } else {
        label.clone()
    };
    let context = if context.as_deref() == Some(status_line.as_str()) {
        None
    } else {
        context
    };
    let session_plan = product
        .map(|p| p.session_plan.as_slice())
        .unwrap_or(&s.entries);
    let plan_kind =
        if !session_plan.is_empty() && session_plan.iter().all(|e| e.state == StepState::Done) {
            "최근 완료 계획"
        } else {
            "현재 계획"
        };
    let plan_line = format!(
        "{plan_kind}  ·  {}",
        s.plan_goal
            .as_deref()
            .unwrap_or_else(|| s.source.trim_end_matches(" → 제품 계획"))
    );
    let mut paths = Vec::new();
    if let Some(roadmap) = roadmap {
        paths.push(roadmap);
    }
    if let Some(manifest) = manifest {
        paths.push(manifest);
    }
    if paths.is_empty() {
        paths.push(session_source.clone());
    }
    if view.presentation_warning
        && let Some(path) = paths.last_mut()
    {
        path.push_str(" · 색 설정 확인 필요");
    }
    let side_by_side = paths.len() == 2
        && area.width >= 72
        && format!("{}   │   {}", paths[0], paths[1]).width() <= width as usize;
    let mut lines: Vec<(String, u8)> = Vec::new();
    if area.height == 1 {
        lines.push((
            format!("제품 {product_metric} · 현재 {status_line} · {goal}"),
            1,
        ));
    } else {
        lines.push((String::new(), 0)); // Goal and metric form one reading group.
        if area.height >= 4 {
            lines.push((plan_line, 5));
        }
        lines.push((status_line, 1));
        if area.height >= 6 || (area.height >= 5 && (side_by_side || paths.len() == 1)) {
            lines.push((context.unwrap_or_default(), 6));
        }
        if area.height >= 7 {
            lines.push((session_source, 2));
            if !view.project_choices.is_empty() {
                lines.push((
                    format!(
                        "다른 목표 · {} · Tab으로 전환",
                        view.project_choices.join(" · ")
                    ),
                    2,
                ));
            }
        }
        if area.height >= 6 && side_by_side {
            lines.push((String::new(), 4));
        }
        if side_by_side {
            lines.push((format!("{}   │   {}", paths[0], paths[1]), 3));
        } else {
            lines.extend(paths.into_iter().map(|path| (path, 3)));
        }
        if area.height >= 10 && !s.last_plan_at.is_empty() {
            lines.push((
                format!("최근 계획 변경 · {}", short_time(&s.last_plan_at)),
                2,
            ));
        }
        if area.height >= 18
            && let Some(product) = product
        {
            let task = blocked
                .or_else(|| {
                    product
                        .plan
                        .tasks
                        .iter()
                        .find(|t| t.status == crate::model::Status::Active)
                })
                .or_else(|| {
                    product
                        .plan
                        .tasks
                        .iter()
                        .find(|t| t.status == crate::model::Status::Review)
                })
                .or_else(|| {
                    product
                        .plan
                        .tasks
                        .iter()
                        .find(|t| t.status == crate::model::Status::Planned)
                });
            if let Some(task) = task {
                lines.push((format!("수용 조건 · {}", task.criteria.join(" · ")), 2));
                lines.push((
                    format!(
                        "검증 · {} · 요구 {:?} · 근거는 기록자 선언",
                        task.status.label(),
                        task.required
                    ),
                    2,
                ));
                if let Some(e) = task
                    .evidence
                    .iter()
                    .rev()
                    .find(|e| e.kind != crate::model::Verification::Reported)
                    .or_else(|| task.evidence.last())
                {
                    lines.push((
                        format!(
                            "근거 {:?} · {}{}",
                            e.kind,
                            e.reference,
                            if e.stale { " · 재검증 필요" } else { "" }
                        ),
                        2,
                    ));
                }
                if !task.next_action.is_empty() {
                    lines.push((format!("필요한 행동 · {}", task.next_action), 1));
                }
                if let Some(note) = task.notes.last() {
                    lines.push((format!("메모 · {note}"), 2));
                }
            }
            if product.session_count > 1 {
                lines.push((
                    format!(
                        "복귀 · {}개 세션의 ID·기록 유지 · {}",
                        product.session_count,
                        product.plan.progress()
                    ),
                    2,
                ));
            }
            for event in product.plan.history.iter().rev().take(2) {
                lines.push((
                    format!(
                        "최근 이력 r{} · {} · {}",
                        event.revision, event.actor, event.reason
                    ),
                    2,
                ));
            }
        }
        if area.height >= 8 {
            lines.push((
                if total == 0 {
                    "체크리스트 · 아직 계획이 없습니다".into()
                } else {
                    format!("체크리스트 · {done}/{total} 완료 · 에이전트 보고 기준")
                },
                2,
            ));
            let limit = area.height.saturating_sub(lines.len() as u16) as usize;
            for entry in s.entries.iter().take(limit) {
                let indicator = if !entry.present && entry.state != StepState::Done {
                    "!"
                } else {
                    mark(entry.state)
                };
                lines.push((format!("{indicator} {}", entry.title), 2));
            }
        }
    }
    let path_color = palette.metadata;
    let header_goal = if view.error.is_some() {
        format!("오래된 상태 · {goal}")
    } else if view.storage_error.is_some() {
        format!("저장 실패 · {goal}")
    } else if activity.contains("입력 대기") || source_goal.is_some_and(|g| g.status == "blocked")
    {
        format!("입력 대기 · {goal}")
    } else {
        goal.to_owned()
    };
    for (i, (line, kind)) in lines.iter().take(area.height as usize).enumerate() {
        if i == 0 && area.height > 1 {
            let metric_width = product_metric.width().min(width as usize) as u16;
            let left_width = width.saturating_sub(metric_width.saturating_add(2));
            let title = ellipsize(&header_goal, left_width as usize);
            let mut spans = vec![Span::styled(title, Style::new().fg(primary).bold())];
            if left_width > 0 {
                spans.push(Span::raw("  "));
            }
            if let Some((percent, counts)) = product_metric.split_once(" · ") {
                spans.push(Span::styled(
                    percent,
                    Style::new().fg(palette.accent).bold(),
                ));
                spans.push(Span::styled(
                    format!(" · {counts}"),
                    Style::new().fg(secondary),
                ));
            } else {
                spans.push(Span::styled(
                    product_metric.as_str(),
                    Style::new().fg(palette.accent).bold(),
                ));
            }
            frame.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect {
                    x: area.x + pad,
                    y: area.y,
                    width,
                    height: 1,
                },
            );
            continue;
        }
        let style = match kind {
            1 => Style::new()
                .fg(if color == Color::Yellow {
                    palette.warning
                } else {
                    palette.accent
                })
                .bold(),
            3 => Style::new().fg(path_color),
            _ => Style::new().fg(secondary),
        };
        let text_width = width as usize;
        let clipped = ellipsize(line, text_width);
        let paragraph = if matches!(kind, 5 | 6) {
            if let Some((caption, content)) = clipped.split_once('·') {
                let value_style = if *kind == 5 {
                    Style::new().fg(primary).bold()
                } else {
                    Style::new().fg(primary)
                };
                Paragraph::new(Line::from(vec![
                    Span::styled(
                        format!("{} ·", caption.trim_end()),
                        Style::new().fg(secondary),
                    ),
                    Span::styled(content.to_owned(), value_style),
                ]))
            } else {
                Paragraph::new(clipped).style(style)
            }
        } else {
            Paragraph::new(clipped).style(style)
        };
        frame.render_widget(
            paragraph,
            Rect {
                x: area.x + pad,
                y: area.y + i as u16,
                width,
                height: 1,
            },
        );
    }
    let filled = (done * area.width as usize).checked_div(total).unwrap_or(0) as u16;
    // Paint after text so wide-character continuation cells retain the same track.
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            frame.buffer_mut()[(x, y)].bg = if x - area.x < filled { fill } else { track };
        }
    }
}

fn render_compact(frame: &mut Frame, s: &Snapshot, view: &mut View) {
    let area = frame.area();
    let pad = u16::from(area.width >= 24);
    let inner = Rect {
        x: area.x + pad,
        width: area.width.saturating_sub(pad * 2),
        ..area
    };
    let (done, total) = s.counts();
    let percent = (done * 100).checked_div(total);
    let metric = percent
        .map(|p| format!("{done}/{total} · {p}%"))
        .unwrap_or_else(|| "미정".into());
    let goal = s
        .goal
        .as_ref()
        .map(|g| g.objective.as_str())
        .unwrap_or("아직 목표가 없습니다");
    let issue = view
        .error
        .as_ref()
        .or(view.storage_error.as_ref())
        .or(s.warning.as_ref());
    let title = if view.error.is_some() {
        format!("오래된 상태 · {}", view.error.as_deref().unwrap_or(""))
    } else if view.storage_error.is_some() {
        "저장 실패 · 다시 시도 중".into()
    } else if let Some(warning) = &s.warning {
        format!("! {warning}")
    } else if view.help {
        "도움말".into()
    } else if view.searching || !view.query.is_empty() {
        format!("검색: {}", view.query)
    } else if view.history {
        "변경 이력".into()
    } else if view.detail {
        "항목 상세 · Esc 돌아가기".into()
    } else if view.unfinished {
        format!("미완료 · {goal}")
    } else {
        goal.into()
    };
    let header = Rect { height: 1, ..inner };
    let columns = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(2),
        Constraint::Length(metric.width().min(inner.width as usize) as u16),
    ])
    .split(header);
    frame.render_widget(
        Paragraph::new(ellipsize(&title, columns[0].width as usize)).style(if issue.is_some() {
            Style::new().fg(Color::Yellow).bold()
        } else {
            Style::new().bold()
        }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(ellipsize(&metric, columns[2].width as usize))
            .style(Style::new().fg(Color::Cyan).bold()),
        columns[2],
    );
    if inner.height < 2 {
        return;
    }
    let body = Rect {
        y: inner.y + 1,
        height: inner.height - 1,
        ..inner
    };
    let visible = view.visible(s);
    if !view.manual_selection
        && !view.detail
        && !view.history
        && !view.help
        && let Some(active) = visible
            .iter()
            .position(|i| s.entries[*i].state == StepState::Active && s.entries[*i].present)
    {
        view.selected = active;
    }
    view.selected = view.selected.min(visible.len().saturating_sub(1));
    if view.help || view.history || (view.detail && !visible.is_empty()) {
        let mut text = if view.help {
            "↑↓ / j k  목록·항목 이동\nEnter     현재 항목 상세\n/         검색\nf         미완료만\nh         변경 이력\nEsc       현재 작업 막대로\nq / Ctrl-C 종료\n\n퍼센트는 전체 체크리스트의 에이전트 보고 기준입니다.".to_owned()
        } else if view.history {
            let mut lines = s
                .changes
                .iter()
                .rev()
                .map(|c| format!("{}  {}", short_time(&c.at), c.text))
                .collect::<Vec<_>>();
            for archive in s.archives.iter().rev() {
                lines.push(format!(
                    "이전 목표: {}",
                    archive
                        .goal
                        .as_ref()
                        .map(|g| g.objective.as_str())
                        .unwrap_or("미지정")
                ));
                lines.extend(
                    archive
                        .entries
                        .iter()
                        .map(|e| format!("{} {}", mark(e.state), e.title)),
                );
            }
            lines.join("\n")
        } else {
            let entry = &s.entries[visible[view.selected]];
            format!(
                "{} {}\n\n목표: {}\n근거: 에이전트 보고\n출처: {}\n활동: {}\n현재 계획: {}\n{}",
                mark(entry.state),
                entry.title,
                goal,
                s.source,
                view.connection.as_deref().unwrap_or(&s.activity),
                if entry.present {
                    "포함됨"
                } else {
                    "빠짐 · 기록 유지"
                },
                issue.map(String::as_str).unwrap_or("")
            )
        };
        if view.detail {
            text.push_str(&product_detail(s, visible.get(view.selected).copied()));
        }
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        view.scroll = view.scroll.min(
            paragraph
                .line_count(body.width)
                .saturating_sub(body.height as usize)
                .min(u16::MAX as usize) as u16,
        );
        frame.render_widget(paragraph.scroll((view.scroll, 0)), body);
        return;
    }
    if visible.is_empty() {
        let message = if s.entries.is_empty() {
            "에이전트가 계획을 작성하면 여기에 표시됩니다."
        } else if !view.query.is_empty() {
            "검색 결과가 없습니다. Esc로 돌아가기"
        } else {
            "미완료 항목이 없습니다. f로 전체 보기"
        };
        frame.render_widget(
            Paragraph::new(message)
                .wrap(Wrap { trim: false })
                .style(Style::new().fg(Color::DarkGray)),
            body,
        );
        return;
    }
    let items = visible
        .iter()
        .map(|i| {
            let entry = &s.entries[*i];
            let missing = !entry.present && entry.state != StepState::Done;
            let color = if missing {
                Color::Yellow
            } else {
                match entry.state {
                    StepState::Active => Color::Cyan,
                    StepState::Done => Color::Green,
                    StepState::Pending => Color::DarkGray,
                }
            };
            let label = ellipsize(&entry.title, body.width.saturating_sub(4) as usize);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", if missing { "!" } else { mark(entry.state) }),
                    Style::new().fg(color),
                ),
                Span::styled(
                    label,
                    if entry.state == StepState::Active {
                        Style::new().bold()
                    } else {
                        Style::new()
                    },
                ),
            ]))
        })
        .collect::<Vec<_>>();
    view.list.select(Some(view.selected));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol(if view.manual_selection { "› " } else { "  " })
            .highlight_style(Style::new().bold()),
        body,
        &mut view.list,
    );
    if body.height as usize > visible.len() && inner.height >= 6 {
        frame.render_widget(
            Paragraph::new("? 도움말")
                .style(Style::new().fg(Color::DarkGray))
                .right_aligned(),
            Rect {
                y: inner.y + inner.height - 1,
                height: 1,
                ..inner
            },
        );
    }
}

pub fn render(frame: &mut Frame, s: &Snapshot, view: &mut View) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    if !view.help
        && !view.detail
        && !view.history
        && !view.searching
        && view.query.is_empty()
        && !view.unfinished
        && !view.manual_selection
    {
        render_progress_bar(frame, s, view);
        return;
    }
    if area.height < 11 || area.width < 56 || view.help {
        render_compact(frame, s, view);
        return;
    }
    let (done, total) = s.counts();
    let goal = s
        .goal
        .as_ref()
        .map(|g| g.objective.as_str())
        .unwrap_or("목표 미지정 · 에이전트 계획을 기다립니다");
    let progress = if let Some(percent) = (done * 100).checked_div(total) {
        if area.width < 40 {
            format!("{done}/{total} 완료 {percent}%")
        } else {
            format!("{done}/{total} 완료 · {}% · 남음 {}", percent, total - done)
        }
    } else {
        "체크율 미정 · 아직 항목 없음".into()
    };
    let issue = view
        .error
        .as_ref()
        .or(view.storage_error.as_ref())
        .or(s.warning.as_ref());
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    let title = Path::new(&s.project)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let goal_lines = wrapped(goal, inner.width as usize);
    let goal_height = (goal_lines.len() as u16).clamp(1, if area.height >= 14 { 3 } else { 2 });
    let regions = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(goal_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let state = view.connection.as_deref().unwrap_or(&s.activity);
    let state = if view.error.is_some() {
        "오래된 상태 · 연결 확인"
    } else if view.storage_error.is_some() {
        "저장 실패 · 화면은 최신"
    } else if s.warning.is_some() {
        "목표 확인 필요 · 계획은 최신"
    } else {
        state
    };
    let state = if inner.width < 60 {
        state.split(" · ").next().unwrap_or(state)
    } else {
        state
    };
    let header_regions = Layout::horizontal([
        Constraint::Min(1),
        Constraint::Length(state.width().min(inner.width as usize) as u16),
    ])
    .split(regions[0]);
    let header = Line::from(vec![
        Span::styled(format!("{title}  "), Style::new().bold()),
        Span::styled(
            view.pane
                .as_ref()
                .map(|p| format!("{p} · "))
                .unwrap_or_default(),
            Style::new().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), header_regions[0]);
    frame.render_widget(
        Paragraph::new(state).style(Style::new().fg(if issue.is_some() {
            Color::Yellow
        } else {
            Color::Cyan
        })),
        header_regions[1],
    );
    let mut shown_goal: Vec<_> = goal_lines.into_iter().take(goal_height as usize).collect();
    if wrapped(goal, inner.width as usize).len() > goal_height as usize
        && let Some(last) = shown_goal.last_mut()
    {
        let mut glyphs = last.graphemes(true).collect::<Vec<_>>();
        glyphs.pop();
        *last = format!("{}…", glyphs.concat());
    }
    frame.render_widget(
        Paragraph::new(shown_goal.join("\n")).style(Style::new().bold()),
        regions[1],
    );
    let progress_regions = Layout::horizontal([
        Constraint::Length(if area.width < 40 {
            0
        } else {
            (inner.width / 5).clamp(6, 24)
        }),
        Constraint::Length(if area.width < 40 { 0 } else { 2 }),
        Constraint::Min(1),
    ])
    .split(regions[2]);
    let bar_len = progress_regions[0].width as usize;
    let filled = (done * bar_len).checked_div(total).unwrap_or(0);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("━".repeat(filled), Style::new().fg(Color::Cyan)),
            Span::styled(
                "┄".repeat(bar_len - filled),
                Style::new().fg(Color::DarkGray),
            ),
        ])),
        progress_regions[0],
    );
    frame.render_widget(
        Paragraph::new(progress).style(Style::new().bold()),
        progress_regions[2],
    );
    let absent = s
        .entries
        .iter()
        .filter(|e| !e.present && e.state != StepState::Done)
        .count();
    let heading = if view.history {
        "변경 이력 · 이전 목표".into()
    } else if view.detail {
        "항목 상세".into()
    } else if absent > 0 {
        format!("체크리스트 · 계획에서 빠진 미완료 {absent}개도 포함")
    } else {
        "체크리스트".into()
    };
    frame.render_widget(
        Paragraph::new(heading).style(Style::new().fg(Color::DarkGray)),
        regions[3],
    );
    let visible = view.visible(s);
    view.selected = view.selected.min(visible.len().saturating_sub(1));
    if view.history || (view.detail && !visible.is_empty()) {
        let mut text = if view.history {
            let mut lines = s
                .changes
                .iter()
                .rev()
                .map(|c| format!("{}  {}", short_time(&c.at), c.text))
                .collect::<Vec<_>>();
            if let Some(product) = &s.overall {
                lines.extend(
                    product.plan.history.iter().rev().map(|e| {
                        format!("r{} [{}] {} · {}", e.revision, e.actor, e.action, e.reason)
                    }),
                );
            }
            for archive in s.archives.iter().rev() {
                lines.push(format!(
                    "\n이전 목표: {}",
                    archive
                        .goal
                        .as_ref()
                        .map(|g| g.objective.as_str())
                        .unwrap_or("미지정")
                ));
                lines.extend(
                    archive
                        .entries
                        .iter()
                        .map(|e| format!("{} {}", mark(e.state), e.title)),
                );
            }
            lines.join("\n")
        } else {
            let entry = &s.entries[visible[view.selected]];
            format!(
                "{} {}\n\n완료 표시 기준: 원본 계획의 완료 상태\n근거 수준: 에이전트 보고 (자동 검사/사람 검증 아님)\n출처: {}\n현재 계획: {}\n이전 완료 기록: {}\n항목 ID: {}\n세션: {}\n\n목표: {}",
                mark(entry.state),
                entry.title,
                s.source,
                if entry.present {
                    "포함됨"
                } else {
                    "누락됨 · 기록 보존"
                },
                if entry.was_done { "있음" } else { "없음" },
                entry.id,
                s.session,
                goal
            )
        };
        if view.detail {
            text.push_str(&product_detail(s, visible.get(view.selected).copied()));
        }
        let widget = Paragraph::new(text).wrap(Wrap { trim: false });
        view.scroll = view.scroll.min(
            widget
                .line_count(regions[4].width)
                .saturating_sub(regions[4].height as usize)
                .min(u16::MAX as usize) as u16,
        );
        frame.render_widget(widget.scroll((view.scroll, 0)), regions[4]);
    } else if visible.is_empty() {
        frame.render_widget(Paragraph::new(if s.entries.is_empty() {
            "아직 계획이 없습니다.\n에이전트가 goal·plan을 작성하면 여기에 자동으로 표시됩니다.\n작업을 별도로 등록할 필요가 없습니다."
        } else { "검색·필터에 맞는 항목이 없습니다. Esc로 검색을 지울 수 있습니다." }).wrap(Wrap { trim: false }).style(Style::new().fg(Color::DarkGray)), regions[4]);
    } else {
        let items = visible
            .iter()
            .map(|i| {
                let entry = &s.entries[*i];
                let prefix = if !entry.present && entry.state != StepState::Done {
                    "!"
                } else {
                    mark(entry.state)
                };
                let color = if !entry.present && entry.state != StepState::Done {
                    Color::Yellow
                } else {
                    match entry.state {
                        StepState::Active => Color::Cyan,
                        StepState::Done => Color::Green,
                        StepState::Pending => Color::Reset,
                    }
                };
                let lines = wrapped(&entry.title, regions[4].width.saturating_sub(6) as usize)
                    .into_iter()
                    .enumerate()
                    .map(|(i, line)| {
                        Line::from(vec![
                            Span::styled(
                                if i == 0 {
                                    format!("{prefix} ")
                                } else {
                                    "  ".into()
                                },
                                Style::new().fg(color),
                            ),
                            Span::styled(
                                line,
                                if entry.state == StepState::Active {
                                    Style::new().bold()
                                } else {
                                    Style::new()
                                },
                            ),
                        ])
                    })
                    .collect::<Vec<_>>();
                ListItem::new(lines)
            })
            .collect::<Vec<_>>();
        view.list.select(Some(view.selected));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("› ")
                .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
            regions[4],
            &mut view.list,
        );
    }
    let note = if let Some(error) = issue {
        format!("! {error}")
    } else {
        format!(
            "{} · {} · 에이전트 보고 기준",
            s.source,
            if s.last_plan_at.is_empty() {
                "변경 대기".into()
            } else {
                short_time(&s.last_plan_at)
            }
        )
    };
    frame.render_widget(
        Paragraph::new(note).style(Style::new().fg(if issue.is_some() {
            Color::Yellow
        } else {
            Color::DarkGray
        })),
        regions[5],
    );
    let help = if view.searching || !view.query.is_empty() {
        format!("검색: {} ▏  Esc 해제", view.query)
    } else {
        "q 종료  ↵ 상세  ↑↓ 이동  / 검색  f 미완료  h 이력".into()
    };
    frame.render_widget(
        Paragraph::new(help)
            .block(Block::default().borders(Borders::NONE))
            .style(Style::new().fg(Color::DarkGray)),
        regions[6],
    );
}

fn mark(state: StepState) -> &'static str {
    match state {
        StepState::Done => "✓",
        StepState::Active => "▶",
        StepState::Pending => "○",
    }
}

fn product_detail(s: &Snapshot, index: Option<usize>) -> String {
    let Some(p) = &s.overall else {
        return String::new();
    };
    let mut text = format!(
        "\n\n제품 전체: {}/{} 완료\n원본 계획: {}\n연결 세션: {}개\n세션 목표: {}\n현재 세부 작업: {}\n연결 항목: {}\n{}",
        s.counts().0,
        s.counts().1,
        p.roadmap.display(),
        p.session_count,
        p.session_goal
            .as_ref()
            .map(|g| g.objective.as_str())
            .unwrap_or("미지정"),
        p.current_work
            .as_ref()
            .map(|e| e.title.as_str())
            .unwrap_or("현재 진행 항목 없음"),
        p.parent_key.as_deref().unwrap_or("미연결"),
        p.notice.as_deref().unwrap_or("")
    );
    if let Some(task) = index.and_then(|i| p.plan.tasks.iter().find(|t| t.id == s.entries[i].id)) {
        text.push_str(&format!(
            "\n완료 조건: {}\n요구 검증: {:?}\n제품 상태: {}",
            task.criteria.join(" · "),
            task.required,
            task.status.label()
        ));
        for e in &task.evidence {
            text.push_str(&format!(
                "\n{:?}: {}{}",
                e.kind,
                e.reference,
                if e.stale { " (재검증 필요)" } else { "" }
            ));
        }
        if let Some(blocker) = &task.blocker {
            text.push_str(&format!("\n막힌 이유: {blocker}"));
        }
        if !task.next_action.is_empty() {
            text.push_str(&format!("\n필요한 행동: {}", task.next_action));
        }
        for note in &task.notes {
            text.push_str(&format!("\n메모: {note}"));
        }
        if !task.derived_from.is_empty() {
            text.push_str(&format!("\n분할/병합 원본: {:?}", task.derived_from));
        }
    }
    text
}
fn short_time(at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(at)
        .map(|t| {
            t.with_timezone(&chrono::Utc)
                .format("%H:%M:%S UTC")
                .to_string()
        })
        .unwrap_or_else(|_| at.into())
}

#[derive(Default)]
pub struct Options {
    pub pane: Option<String>,
    pub session: Option<Uuid>,
    pub rollout: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub once: bool,
    pub cache: Option<PathBuf>,
    pub project: Option<PathBuf>,
}

/// Explicit project selection is the escape hatch; ordinary follow stays passive.
pub fn watch_projects(paths: Vec<PathBuf>) -> Result<()> {
    ensure!(!paths.is_empty(), "at least one explicit project required");
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "projects requires a terminal; use product resume for JSON"
    );
    let projects = paths
        .iter()
        .map(|p| crate::project::Project::open(p))
        .collect::<Result<Vec<_>>>()?;
    let mut snapshots = projects
        .iter()
        .map(|p| p.snapshot())
        .collect::<Result<Vec<_>>>()?;
    let mut views: Vec<_> = (0..projects.len()).map(|_| View::default()).collect();
    for (project, view) in projects.iter().zip(&mut views) {
        refresh_palette(project.root(), view);
    }
    let mut selected = 0;
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ratatui::restore();
        }
    }
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    let mut refresh = Instant::now();
    loop {
        if refresh.elapsed() > Duration::from_millis(500) {
            for (i, p) in projects.iter().enumerate() {
                refresh_palette(p.root(), &mut views[i]);
                match p.snapshot() {
                    Ok(s) => {
                        snapshots[i] = s;
                        views[i].error = None;
                    }
                    Err(_) => views[i].error = Some("제품 읽기 실패 · 백업 복원/경로 확인".into()),
                }
            }
            refresh = Instant::now();
        }
        views[selected].project_choices = snapshots
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != selected)
            .filter_map(|(_, s)| s.goal.as_ref().map(|g| g.objective.clone()))
            .collect();
        terminal.draw(|f| render(f, &snapshots[selected], &mut views[selected]))?;
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Tab => selected = (selected + 1) % projects.len(),
                KeyCode::BackTab => selected = (selected + projects.len() - 1) % projects.len(),
                _ => {
                    if views[selected].key(key) {
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn follow(options: Options) -> Result<()> {
    let home = options
        .home
        .or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".codex")))
        .context("Codex home unavailable")?;
    let mut binding: Option<Binding> = None;
    let path = if let Some(path) = options.rollout {
        path
    } else if let Some(pane) = options.pane {
        let b = herdr::resolve(&pane)?;
        let path = b.rollout.clone();
        binding = Some(b);
        path
    } else if let Some(session) = options.session {
        live::find_session(&home, session)?
    } else if std::env::var("HERDR_ENV").as_deref() != Ok("1") {
        let session = std::env::var("CODEX_THREAD_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .context("use --session or --rollout outside Herdr")?;
        live::find_session(&home, session)?
    } else {
        // Use the captured pane explicitly: some Herdr versions resolve --current
        // to the focused UI pane when called from an ordinary shell subprocess.
        let caller = std::env::var("HERDR_PANE_ID").context("caller pane missing; use --pane")?;
        let mut candidates = Vec::new();
        for direction in ["up", "down"] {
            if let Ok(value) = herdr::call(&[
                "pane",
                "neighbor",
                "--direction",
                direction,
                "--pane",
                &caller,
            ]) && let Some(pane) = value["result"]["neighbor"]["neighbor_pane_id"].as_str()
                && let Ok(binding) = herdr::resolve(pane)
            {
                candidates.push(binding);
            }
        }
        ensure!(
            candidates.len() == 1,
            "need one exact agent neighbor above or below; use --pane for ambiguous layouts"
        );
        let b = candidates.pop().unwrap();
        let path = b.rollout.clone();
        binding = Some(b);
        path
    };
    let mut feed = Feed::open(path.canonicalize()?, options.session, Some(home))?;
    let project = if let Some(path) = options.project {
        Some(crate::project::Project::open(&path)?)
    } else {
        crate::project::Project::discover(Path::new(&feed.snapshot.project))?
    };
    if options.once {
        feed.refresh_all()?;
        let snapshot = if let Some(project) = project {
            project.project(&feed.snapshot)?
        } else {
            feed.snapshot.clone()
        };
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }
    ensure!(
        io::stdin().is_terminal() && io::stdout().is_terminal(),
        "follow requires a terminal; use --once for JSON"
    );
    let cache = options.cache.unwrap_or_else(|| {
        PathBuf::from(&feed.snapshot.project)
            .join(".agent-progress")
            .join(format!("{}.json", feed.snapshot.session))
    });
    feed.restore_checkpoint(&cache)?;
    feed.refresh_all()?;
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ratatui::restore();
        }
    }
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    let mut view = View {
        selected: feed
            .snapshot
            .entries
            .iter()
            .position(|e| e.state == StepState::Active)
            .unwrap_or(0),
        pane: binding.as_ref().map(|b| b.pane.clone()),
        ..View::default()
    };
    let mut snapshot = if let Some(project) = &project {
        project.project(&feed.snapshot)?
    } else {
        feed.snapshot.clone()
    };
    let settings_root = project
        .as_ref()
        .map(|p| p.root().to_owned())
        .unwrap_or_else(|| PathBuf::from(&snapshot.project));
    refresh_palette(&settings_root, &mut view);
    let mut runner = crate::runner::Runner::start(feed, cache, binding, project, snapshot.clone());
    let mut last_update = Instant::now();
    let mut settings_check = Instant::now();
    loop {
        if settings_check.elapsed() >= Duration::from_millis(500) {
            refresh_palette(&settings_root, &mut view);
            settings_check = Instant::now();
        }
        while let Ok(update) = runner.updates.try_recv() {
            let selected = view
                .visible(&snapshot)
                .get(view.selected)
                .map(|i| snapshot.entries[*i].id);
            snapshot = update.snapshot;
            view.error = update.error;
            view.storage_error = update.storage_error;
            view.connection = update.connection;
            last_update = Instant::now();
            if let Some(id) = selected
                && let Some(i) = view
                    .visible(&snapshot)
                    .iter()
                    .position(|i| snapshot.entries[*i].id == id)
            {
                view.selected = i;
            }
        }
        if last_update.elapsed() > Duration::from_secs(4) {
            view.error = Some("세션 확인 지연 · 마지막 기록 표시".into());
        }
        terminal.draw(|f| render(f, &snapshot, &mut view))?;
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && view.key(key)
        {
            break;
        }
    }
    runner.shutdown()?;
    Ok(())
}

fn refresh_palette(root: &Path, view: &mut View) {
    match crate::settings::load(root).and_then(|settings| {
        let palette = settings.palette(None)?;
        let states = crate::settings::BackgroundState::ALL
            .into_iter()
            .map(|state| Ok((state, settings.palette(Some(state))?)))
            .collect::<Result<_>>()?;
        Ok((palette, states))
    }) {
        Ok((palette, states)) => {
            view.palette = palette;
            view.state_palettes = states;
            view.presentation_warning = false;
        }
        Err(_) => view.presentation_warning = true,
    }
}

#[cfg(test)]
mod dashboard_tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn rendered(width: u16, height: u16, snapshot: &Snapshot) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| render(frame, snapshot, &mut View::default()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn passive_dashboard_fits_one_three_and_five_rows_without_source_fabrication() {
        let snapshot = Snapshot::empty(Uuid::nil(), "/tmp/project".into());
        let one = rendered(133, 1, &snapshot).join("");
        let one = one.split_whitespace().collect::<String>();
        assert!(one.contains("제품"));
        assert!(one.contains("미정"));
        assert!(one.contains("현재"));

        let three = rendered(80, 3, &snapshot)
            .join("\n")
            .split_whitespace()
            .collect::<String>();
        assert!(three.contains("목표미정") && three.contains("체크율미정"));
        assert!(three.contains("계획대기"));
        assert!(three.contains("계획출처·계획대기"));

        let five = rendered(133, 5, &snapshot)
            .join("\n")
            .split_whitespace()
            .collect::<String>();
        assert!(five.contains("계획출처·계획대기"));
        assert!(!five.contains("docs/current-plan.md"));
        assert!(!five.contains("제품계획·"));
    }
}
