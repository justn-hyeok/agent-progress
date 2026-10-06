use crate::store::{Item, Plan, State, Store};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use std::time::{Duration, Instant, SystemTime};

const BEAT_EVERY: Duration = Duration::from_secs(2);
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn item_line(item: &crate::store::Item) -> String {
    let mut s = format!("{} {}. {}", item.state.mark(), item.id, item.title);
    if let Some(reason) = &item.reason {
        s.push_str(&format!(" — {reason}"));
    }
    if let Some(needs) = &item.needs {
        s.push_str(&format!(" (필요: {needs})"));
    }
    s
}

/// Plain text used by CLI output and `view --once`.
pub fn summary(plan: Option<&Plan>) -> String {
    let Some(plan) = plan else {
        return "계획 없음 · 진행률 미정".into();
    };
    let mut lines = vec![format!(
        "{} · {}",
        plan.goal.as_deref().unwrap_or("목표 미정"),
        plan.progress_label()
    )];
    lines.extend(plan.items.iter().map(item_line));
    lines.join("\n")
}

#[derive(Clone, Copy)]
struct Palette {
    track: Color,
    fill: Color,
    accent: Color,
    text: Color,
    muted: Color,
    metadata: Color,
    warning: Color,
}

/// Signal preset with the brighter fill and secondary text the user tuned in 1.x.
const SIGNAL: Palette = Palette {
    track: Color::Rgb(0x0F, 0x14, 0x18),
    fill: Color::Rgb(0x34, 0x54, 0x3B),
    accent: Color::Rgb(0xC7, 0xF9, 0x6C),
    text: Color::Rgb(0xF2, 0xF5, 0xEE),
    muted: Color::Rgb(0xC4, 0xD0, 0xC6),
    metadata: Color::Rgb(0xB6, 0xC4, 0xBA),
    warning: Color::Rgb(0xF5, 0xC2, 0x6F),
};

fn hex(value: Option<&serde_json::Value>) -> Option<Color> {
    let v = value?.as_str()?.strip_prefix('#')?;
    (v.len() == 6).then_some(())?;
    let n = u32::from_str_radix(v, 16).ok()?;
    Some(Color::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// Project colors from `.agent-progress/ui.json` ("theme"), as saved by 1.x.
fn palette(store: &Store) -> Palette {
    let theme = store
        .path
        .ancestors()
        .nth(3)
        .and_then(|root| std::fs::read(root.join(".agent-progress/ui.json")).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("theme").cloned());
    let Some(t) = theme else { return SIGNAL };
    let pick = |k: &str, d: Color| hex(t.get(k)).unwrap_or(d);
    Palette {
        track: pick("track", SIGNAL.track),
        fill: pick("fill", SIGNAL.fill),
        accent: pick("accent", SIGNAL.accent),
        text: pick("text", SIGNAL.text),
        muted: pick("muted", SIGNAL.muted),
        metadata: pick("metadata", SIGNAL.metadata),
        warning: pick("warning", SIGNAL.warning),
    }
}

fn ellipsize(text: &str, width: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > width - 1 {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Status,
    Caption,
    CaptionBold,
    Path,
    Muted,
}

fn draw(frame: &mut Frame, p: &Palette, plan: Option<&Plan>, error: Option<&str>, source: &str) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    let empty = Plan::new("");
    let plan = plan.unwrap_or(&empty);
    let (done, total) = plan.progress();
    let goal = plan.goal.as_deref().unwrap_or("목표 미정");
    let doing = plan.items.iter().find(|i| i.state == State::Doing);
    let blocked = plan.items.iter().find(|i| i.state == State::Blocked);
    let next = plan.items.iter().find(|i| i.state == State::Todo);
    let last_done = plan.last_done();

    let (status, warn) = if let Some(error) = error {
        (format!("오래된 상태 · {error}"), true)
    } else if let Some(b) = blocked {
        let mut s = format!(
            "! {} · {}",
            b.title,
            b.reason.as_deref().unwrap_or("막힌 이유 확인")
        );
        if let Some(needs) = &b.needs {
            s.push_str(&format!(" · 필요: {needs}"));
        }
        (s, true)
    } else if total == 0 {
        ("계획 대기".into(), false)
    } else if done == total {
        ("✓ 완료".into(), false)
    } else if let Some(d) = doing {
        (format!("계획상 진행 · {}", d.title), false)
    } else {
        (
            format!("다음 · {}", next.map_or("계획 갱신", |n| n.title.as_str())),
            false,
        )
    };
    let metric = (done * 100)
        .checked_div(total)
        .map(|pct| format!("{pct}% · {done}/{total}"))
        .unwrap_or_else(|| "체크율 미정".into());
    let plan_line = match doing {
        Some(d) => format!("현재 계획  ·  {}", d.title),
        None => format!("현재 계획  ·  {goal}"),
    };
    let context = if warn || done == total {
        last_done.map(|i| format!("마지막 완료 · {}", i.title))
    } else {
        next.filter(|_| doing.is_some())
            .map(|i| format!("다음 · {}", i.title))
            .or_else(|| last_done.map(|i| format!("마지막 완료 · {}", i.title)))
    };

    let pad = if area.width >= 24 {
        2
    } else {
        u16::from(area.width > 4)
    };
    let width = area.width.saturating_sub(pad * 2).min(110);
    let mut lines: Vec<(String, Kind)> = Vec::new();
    if area.height == 1 {
        lines.push((format!("{metric} · {status} · {goal}"), Kind::Status));
    } else {
        lines.push((String::new(), Kind::Muted)); // header, drawn below
        if area.height >= 4 {
            lines.push((plan_line, Kind::CaptionBold));
        }
        lines.push((status.clone(), Kind::Status));
        if area.height >= 5
            && let Some(c) = context
        {
            lines.push((c, Kind::Caption));
        }
        lines.push((format!("계획 · {source}"), Kind::Path));
        if area.height >= 8 && total > 0 {
            lines.push((
                format!("체크리스트 · {done}/{total} 완료 · 에이전트 보고 기준"),
                Kind::Muted,
            ));
            for item in &plan.items {
                let mut s = format!("{} {}", item.state.mark(), item.title);
                if let Some(r) = &item.reason {
                    s.push_str(&format!(" — {r}"));
                }
                lines.push((s, Kind::Muted));
            }
        }
    }

    let header_goal = if error.is_some() {
        format!("오래된 상태 · {goal}")
    } else {
        goal.to_string()
    };
    for (i, (text, kind)) in lines.iter().take(area.height as usize).enumerate() {
        let rect = Rect {
            x: area.x + pad,
            y: area.y + i as u16,
            width,
            height: 1,
        };
        if i == 0 && area.height > 1 {
            let (pct, counts) = metric
                .split_once(" · ")
                .map_or((metric.as_str(), None), |(a, b)| (a, Some(b)));
            let metric_width = metric.width() as u16;
            let left = width.saturating_sub(metric_width + 2) as usize;
            let mut spans = vec![
                Span::styled(
                    ellipsize(&header_goal, left),
                    Style::new().fg(p.text).bold(),
                ),
                Span::raw("  "),
                Span::styled(pct.to_string(), Style::new().fg(p.accent).bold()),
            ];
            if let Some(counts) = counts {
                spans.push(Span::styled(
                    format!(" · {counts}"),
                    Style::new().fg(p.muted),
                ));
            }
            frame.render_widget(Paragraph::new(Line::from(spans)), rect);
            continue;
        }
        let clipped = ellipsize(text, width as usize);
        let line = match kind {
            Kind::Caption | Kind::CaptionBold => match clipped.split_once('·') {
                Some((caption, value)) => {
                    let value_style = if *kind == Kind::CaptionBold {
                        Style::new().fg(p.text).bold()
                    } else {
                        Style::new().fg(p.text)
                    };
                    Line::from(vec![
                        Span::styled(
                            format!("{} ·", caption.trim_end()),
                            Style::new().fg(p.muted),
                        ),
                        Span::styled(value.to_string(), value_style),
                    ])
                }
                None => Line::styled(clipped, Style::new().fg(p.text)),
            },
            Kind::Status => Line::styled(
                clipped,
                Style::new()
                    .fg(if warn { p.warning } else { p.accent })
                    .add_modifier(Modifier::BOLD),
            ),
            Kind::Path => Line::styled(clipped, Style::new().fg(p.metadata)),
            Kind::Muted => Line::styled(clipped, Style::new().fg(p.muted)),
        };
        frame.render_widget(Paragraph::new(line), rect);
    }
    // The whole pane is the track; paint after text so wide-character cells keep it.
    for (i, bg) in fill_row(p, done, total, area.width).into_iter().enumerate() {
        for y in area.y..area.bottom() {
            frame.buffer_mut()[(area.x + i as u16, y)].bg = bg;
        }
    }
}

fn mix(a: Color, b: Color, t: f64) -> Color {
    match (a, b) {
        (Color::Rgb(ar, ag, ab), Color::Rgb(br, bg, bb)) => {
            let m = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
            Color::Rgb(m(ar, br), m(ag, bg), m(ab, bb))
        }
        _ if t < 0.5 => a,
        _ => b,
    }
}

/// Background per column: solid fill, then a soft fade into the track that ends
/// exactly at the progress position. 0% and 100% stay solid.
fn fill_row(p: &Palette, done: usize, total: usize, width: u16) -> Vec<Color> {
    let width = usize::from(width);
    if total == 0 || done == 0 {
        return vec![p.track; width];
    }
    if done >= total {
        return vec![p.fill; width];
    }
    let edge = done as f64 / total as f64 * width as f64;
    let ramp = (width as f64 / 5.0).clamp(3.0, 14.0).min(edge);
    (0..width)
        .map(|x| {
            let center = x as f64 + 0.5;
            let t = ((edge - center) / ramp).clamp(0.0, 1.0);
            // Smoothstep keeps both ends of the fade gentle.
            mix(p.track, p.fill, t * t * (3.0 - 2.0 * t))
        })
        .collect()
}

fn mtime(store: &Store) -> Option<SystemTime> {
    std::fs::metadata(&store.path)
        .and_then(|m| m.modified())
        .ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Bar,
    List,
    Detail,
    History,
    Help,
}

/// Interactive state on top of the passive progress bar. Nothing here is required to
/// read the status; it only opens details on request.
#[derive(Default)]
struct Ui {
    mode: Mode,
    selected: usize,
    query: String,
    typing: bool,
    unfinished: bool,
    scroll: usize,
}

enum Action {
    None,
    Quit,
}

fn visible<'a>(plan: Option<&'a Plan>, ui: &Ui) -> Vec<&'a Item> {
    let query = ui.query.to_lowercase();
    plan.map(|p| {
        p.items
            .iter()
            .filter(|i| !ui.unfinished || !matches!(i.state, State::Done | State::Cancelled))
            .filter(|i| query.is_empty() || i.title.to_lowercase().contains(&query))
            .collect()
    })
    .unwrap_or_default()
}

fn handle(ui: &mut Ui, code: KeyCode, ctrl: bool, plan: Option<&Plan>, limit: usize) -> Action {
    if ctrl && code == KeyCode::Char('c') {
        return Action::Quit;
    }
    if ui.typing {
        match code {
            KeyCode::Esc => {
                ui.typing = false;
                ui.query.clear();
            }
            KeyCode::Enter => ui.typing = false,
            KeyCode::Backspace => {
                ui.query.pop();
            }
            KeyCode::Char(c) => ui.query.push(c),
            _ => {}
        }
        ui.selected = 0;
        return Action::None;
    }
    let count = visible(plan, ui).len();
    let current = || {
        plan.and_then(|p| {
            let items = visible(Some(p), &Ui::default());
            items.iter().position(|i| i.state == State::Doing)
        })
        .unwrap_or(0)
    };
    match code {
        KeyCode::Char('q') => return Action::Quit,
        KeyCode::Esc => {
            if ui.mode == Mode::Detail {
                ui.mode = Mode::List;
            } else {
                *ui = Ui::default();
            }
        }
        KeyCode::Down | KeyCode::Char('j') => match ui.mode {
            Mode::Bar => {
                ui.mode = Mode::List;
                ui.selected = current();
            }
            Mode::List => ui.selected = (ui.selected + 1).min(count.saturating_sub(1)),
            _ => ui.scroll = (ui.scroll + 1).min(limit.saturating_sub(1)),
        },
        KeyCode::Up | KeyCode::Char('k') => match ui.mode {
            Mode::Bar => {
                ui.mode = Mode::List;
                ui.selected = current();
            }
            Mode::List => ui.selected = ui.selected.saturating_sub(1),
            _ => ui.scroll = ui.scroll.saturating_sub(1),
        },
        KeyCode::Enter => {
            if ui.mode == Mode::Bar {
                ui.selected = current();
            }
            if count > 0 {
                ui.mode = Mode::Detail;
                ui.scroll = 0;
            }
        }
        KeyCode::Char('/') => {
            ui.mode = Mode::List;
            ui.typing = true;
            ui.query.clear();
            ui.selected = 0;
        }
        KeyCode::Char('f') => {
            ui.mode = Mode::List;
            ui.unfinished = !ui.unfinished;
            ui.selected = 0;
        }
        KeyCode::Char('h') => {
            ui.mode = Mode::History;
            ui.scroll = 0;
        }
        KeyCode::Char('?') => ui.mode = Mode::Help,
        _ => {}
    }
    Action::None
}

fn label(state: State) -> &'static str {
    match state {
        State::Todo => "예정",
        State::Doing => "진행 중",
        State::Blocked => "막힘",
        State::Done => "완료",
        State::Cancelled => "취소",
    }
}

fn ago(at: u64) -> String {
    let secs = crate::store::now().saturating_sub(at);
    match secs {
        0..60 => "방금".into(),
        60..3600 => format!("{}분 전", secs / 60),
        3600..86400 => format!("{}시간 전", secs / 3600),
        _ => format!("{}일 전", secs / 86400),
    }
}

fn history_lines(store: &Store) -> Vec<String> {
    let text = std::fs::read_to_string(store.history_path()).unwrap_or_default();
    let mut lines: Vec<String> = text
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            format!(
                "{}  {}  · {}",
                ago(v["at"].as_u64().unwrap_or(0)),
                v["event"].as_str().unwrap_or(""),
                v["progress"].as_str().unwrap_or("")
            )
        })
        .collect();
    if lines.is_empty() {
        lines.push("아직 변경 이력이 없습니다".into());
    }
    lines
}

const HELP: &str = "↑↓ / j k   목록·항목 이동\nEnter      항목 상세\n/          검색 (Enter 확정, Esc 취소)\nf          미완료만 보기\nh          변경 이력\nEsc        진행 막대로 돌아가기\nq / Ctrl-C 종료\n\n퍼센트는 완료/(전체−취소)이며 에이전트 보고 기준입니다.";

fn draw_panel(frame: &mut Frame, p: &Palette, plan: Option<&Plan>, ui: &Ui, history: &[String]) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    let pad = if area.width >= 24 {
        2
    } else {
        u16::from(area.width > 4)
    };
    let width = area.width.saturating_sub(pad * 2);
    let (done, total) = plan.map_or((0, 0), Plan::progress);
    let metric = (done * 100)
        .checked_div(total)
        .map(|pct| format!("{pct}% · {done}/{total}"))
        .unwrap_or_else(|| "체크율 미정".into());
    let title = match ui.mode {
        _ if ui.typing => format!("검색: {}▏", ui.query),
        Mode::List if !ui.query.is_empty() => format!("검색: {} · Esc 돌아가기", ui.query),
        Mode::List if ui.unfinished => "미완료 · f 전체 보기".into(),
        Mode::List => "목록 · Enter 상세 · ? 도움말".into(),
        Mode::Detail => "항목 상세 · Esc 목록".into(),
        Mode::History => "변경 이력 · Esc 돌아가기".into(),
        Mode::Help => "도움말 · Esc 돌아가기".into(),
        Mode::Bar => String::new(),
    };
    let left = width.saturating_sub(metric.width() as u16 + 2) as usize;
    let header = Line::from(vec![
        Span::styled(ellipsize(&title, left), Style::new().fg(p.text).bold()),
        Span::raw("  "),
        Span::styled(metric, Style::new().fg(p.accent).bold()),
    ]);
    let rect = |row: u16| Rect {
        x: area.x + pad,
        y: area.y + row,
        width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(header), rect(0));
    let rows = usize::from(area.height.saturating_sub(1));
    let body: Vec<Line> = match ui.mode {
        Mode::List | Mode::Bar => {
            let items = visible(plan, ui);
            if items.is_empty() {
                vec![Line::styled(
                    if plan.is_none_or(|p| p.items.is_empty()) {
                        "에이전트가 계획을 기록하면 여기에 표시됩니다"
                    } else {
                        "해당하는 항목이 없습니다 · Esc 돌아가기"
                    },
                    Style::new().fg(p.muted),
                )]
            } else {
                let start = ui.selected.saturating_sub(rows.saturating_sub(1));
                items
                    .iter()
                    .enumerate()
                    .skip(start)
                    .map(|(n, i)| {
                        let chosen = n == ui.selected;
                        let color = match i.state {
                            State::Blocked => p.warning,
                            State::Doing => p.accent,
                            State::Done | State::Cancelled => p.muted,
                            State::Todo => p.text,
                        };
                        let text = format!(
                            "{}{} {}. {}",
                            if chosen { "› " } else { "  " },
                            i.state.mark(),
                            i.id,
                            i.title
                        );
                        let style = Style::new().fg(color);
                        Line::styled(
                            ellipsize(&text, width as usize),
                            if chosen {
                                style.add_modifier(Modifier::BOLD)
                            } else {
                                style
                            },
                        )
                    })
                    .collect()
            }
        }
        Mode::Detail => {
            let items = visible(plan, ui);
            match items.get(ui.selected) {
                None => vec![],
                Some(i) => {
                    let mut out = vec![
                        Line::styled(
                            format!("{} {}. {}", i.state.mark(), i.id, i.title),
                            Style::new().fg(p.text).bold(),
                        ),
                        Line::from(vec![
                            Span::styled("상태 · ", Style::new().fg(p.muted)),
                            Span::styled(label(i.state), Style::new().fg(p.text)),
                        ]),
                    ];
                    if let Some(r) = &i.reason {
                        out.push(Line::from(vec![
                            Span::styled("이유 · ", Style::new().fg(p.muted)),
                            Span::styled(r.clone(), Style::new().fg(p.warning)),
                        ]));
                    }
                    if let Some(n) = &i.needs {
                        out.push(Line::from(vec![
                            Span::styled("필요 · ", Style::new().fg(p.muted)),
                            Span::styled(n.clone(), Style::new().fg(p.warning)),
                        ]));
                    }
                    if let Some(at) = i.done_at {
                        out.push(Line::from(vec![
                            Span::styled("완료 · ", Style::new().fg(p.muted)),
                            Span::styled(ago(at), Style::new().fg(p.text)),
                        ]));
                    }
                    if let Some(goal) = plan.and_then(|p| p.goal.as_deref()) {
                        out.push(Line::from(vec![
                            Span::styled("목표 · ", Style::new().fg(p.muted)),
                            Span::styled(goal.to_string(), Style::new().fg(p.text)),
                        ]));
                    }
                    out.push(Line::styled(
                        "근거 · 에이전트 보고 (검증 아님)",
                        Style::new().fg(p.metadata),
                    ));
                    out
                }
            }
        }
        Mode::History => history
            .iter()
            .map(|h| Line::styled(h.clone(), Style::new().fg(p.muted)))
            .collect(),
        Mode::Help => HELP
            .lines()
            .map(|h| Line::styled(h.to_string(), Style::new().fg(p.muted)))
            .collect(),
    };
    let scroll = if matches!(ui.mode, Mode::Detail | Mode::History | Mode::Help) {
        ui.scroll.min(body.len().saturating_sub(rows))
    } else {
        0
    };
    let body_rect = Rect {
        x: area.x + pad,
        y: area.y + 1,
        width,
        height: area.height.saturating_sub(1),
    };
    frame.render_widget(
        Paragraph::new(body.into_iter().skip(scroll).collect::<Vec<_>>())
            .wrap(Wrap { trim: false }),
        body_rect,
    );
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            frame.buffer_mut()[(x, y)].bg = p.track;
        }
    }
}

/// Run the viewer. `instance` is set for a viewer that `ap` opened automatically: it
/// keeps a heartbeat, exits once the plan stops naming it, and its q dismisses
/// automatic reopening. A manually started viewer (`ap view`) changes nothing on exit.
pub fn watch(store: &Store, instance: Option<&str>) -> Result<()> {
    let mut plan = store.load().ok().flatten();
    let mut error: Option<String> = None;
    let mut seen = mtime(store);
    let source = store
        .path
        .components()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string();
    let colors = palette(store);
    let mut ui = Ui::default();
    let mut history: Vec<String> = Vec::new();
    let mut last_beat = Instant::now() - BEAT_EVERY;
    let mut terminal = ratatui::init();
    let result = (|| -> Result<bool> {
        loop {
            if let Some(id) = instance
                && last_beat.elapsed() >= BEAT_EVERY
            {
                store.beat(id)?;
                last_beat = Instant::now();
            }
            // Reload before handling input so a burst of keys never delays updates.
            let current = mtime(store);
            if current != seen {
                seen = current;
                match store.load() {
                    Ok(p) => {
                        plan = p;
                        error = None;
                        if ui.mode == Mode::History {
                            history = history_lines(store);
                        }
                    }
                    // Keep the last good plan visible and say it is stale.
                    Err(e) => error = Some(e.to_string()),
                }
                // `ap close`, `ap open` or a newer viewer replaced us.
                if let (Some(id), Some(p)) = (instance, &plan)
                    && p.viewer.as_ref().is_none_or(|v| v.instance != id)
                {
                    return Ok(false);
                }
            }
            terminal.draw(|f| {
                if ui.mode == Mode::Bar && !ui.typing {
                    draw(f, &colors, plan.as_ref(), error.as_deref(), &source)
                } else {
                    draw_panel(f, &colors, plan.as_ref(), &ui, &history)
                }
            })?;
            if event::poll(Duration::from_millis(500))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                let limit = match ui.mode {
                    Mode::History => history.len(),
                    Mode::Help => HELP.lines().count(),
                    _ => 16,
                };
                if let Action::Quit = handle(&mut ui, key.code, ctrl, plan.as_ref(), limit) {
                    return Ok(true);
                }
                if ui.mode == Mode::History {
                    history = history_lines(store);
                }
            }
        }
    })();
    ratatui::restore();
    if let Some(id) = instance {
        store.clear_beat(id);
    }
    if result? && let (Some(id), Some(p)) = (instance, &plan) {
        // The user dismissed this automatic viewer: don't pop it back up.
        store.update(&p.key, "", |p| {
            if p.viewer.as_ref().is_some_and(|v| v.instance == id) {
                p.viewer = None;
                p.view_suppressed = true;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn screen(plan: &Plan, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| draw(f, &SIGNAL, Some(plan), None, "plans/x.json"))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
            // Wide (Korean) cells are followed by a blank continuation cell.
            .replace(' ', "")
    }

    fn plan(n: u32) -> Plan {
        let mut plan = Plan::new("t");
        plan.goal = Some("목표".into());
        for i in 1..=n {
            plan.add(&format!("항목{i}")).unwrap();
        }
        plan
    }

    #[test]
    fn header_groups_goal_percent_and_counts() {
        let mut plan = plan(4);
        plan.set("1", State::Done, None, None).unwrap();
        plan.set("2", State::Doing, None, None).unwrap();
        let text = screen(&plan, 80, 7);
        assert!(
            text.lines().next().unwrap().contains("목표25%·1/4"),
            "{text}"
        );
        assert!(text.contains("현재계획·항목2"), "{text}");
        assert!(text.contains("계획상진행·항목2"), "{text}");
    }

    #[test]
    fn blocked_status_shows_reason_and_who_acts() {
        let mut plan = plan(3);
        plan.set(
            "3",
            State::Blocked,
            Some("키필요".into()),
            Some("user".into()),
        )
        .unwrap();
        let text = screen(&plan, 80, 5);
        assert!(text.contains("!항목3·키필요·필요:user"), "{text}");
    }

    #[test]
    fn empty_plan_is_undetermined() {
        let text = screen(&Plan::new("t"), 60, 4);
        assert!(text.contains("체크율미정"), "{text}");
        assert!(text.contains("계획대기"), "{text}");
    }

    #[test]
    fn tall_pane_lists_checklist() {
        let mut plan = plan(3);
        plan.set("1", State::Done, None, None).unwrap();
        let text = screen(&plan, 80, 12);
        assert!(text.contains("체크리스트·1/3완료"), "{text}");
        assert!(text.contains("[x]항목1"), "{text}");
    }

    #[test]
    fn background_fades_into_track_at_progress_edge() {
        let row = fill_row(&SIGNAL, 1, 2, 40);
        assert_eq!(row[0], SIGNAL.fill);
        assert_eq!(row[20], SIGNAL.track);
        assert_eq!(row[39], SIGNAL.track);
        let fading: Vec<_> = row[..20].iter().filter(|c| **c != SIGNAL.fill).collect();
        assert!(fading.len() >= 3, "{row:?}");
        assert!(fading.iter().all(|c| **c != SIGNAL.track));
    }

    #[test]
    fn empty_and_complete_are_solid() {
        assert!(
            fill_row(&SIGNAL, 0, 3, 30)
                .iter()
                .all(|c| *c == SIGNAL.track)
        );
        assert!(
            fill_row(&SIGNAL, 3, 3, 30)
                .iter()
                .all(|c| *c == SIGNAL.fill)
        );
        assert!(
            fill_row(&SIGNAL, 0, 0, 30)
                .iter()
                .all(|c| *c == SIGNAL.track)
        );
    }

    fn press(ui: &mut Ui, plan: &Plan, keys: &[KeyCode]) {
        for k in keys {
            handle(ui, *k, false, Some(plan), 16);
        }
    }

    fn panel(plan: &Plan, ui: &Ui, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(70, height)).unwrap();
        terminal
            .draw(|f| {
                draw_panel(
                    f,
                    &SIGNAL,
                    Some(plan),
                    ui,
                    &["방금 done 1 · 1/3 (33%)".into()],
                )
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..70).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
            .replace(' ', "")
    }

    #[test]
    fn arrow_opens_list_at_current_item_and_enter_shows_detail() {
        let mut plan = plan(3);
        plan.set(
            "2",
            State::Blocked,
            Some("키필요".into()),
            Some("user".into()),
        )
        .unwrap();
        plan.set("3", State::Doing, None, None).unwrap();
        let mut ui = Ui::default();
        press(&mut ui, &plan, &[KeyCode::Down]);
        assert_eq!((ui.mode, ui.selected), (Mode::List, 2));
        press(&mut ui, &plan, &[KeyCode::Up, KeyCode::Enter]);
        assert_eq!(ui.mode, Mode::Detail);
        let text = panel(&plan, &ui, 10);
        assert!(text.contains("[!]2.항목2"), "{text}");
        assert!(text.contains("이유·키필요"), "{text}");
        assert!(text.contains("필요·user"), "{text}");
        press(&mut ui, &plan, &[KeyCode::Esc]);
        assert_eq!(ui.mode, Mode::List);
        press(&mut ui, &plan, &[KeyCode::Esc]);
        assert_eq!(ui.mode, Mode::Bar);
    }

    #[test]
    fn search_filters_and_q_is_typed_while_searching() {
        let mut plan = plan(3);
        plan.add("quick fix").unwrap();
        let mut ui = Ui::default();
        press(
            &mut ui,
            &plan,
            &[KeyCode::Char('/'), KeyCode::Char('q'), KeyCode::Char('u')],
        );
        assert!(matches!(
            handle(&mut ui, KeyCode::Enter, false, Some(&plan), 16),
            Action::None
        ));
        assert_eq!(ui.query, "qu");
        let text = panel(&plan, &ui, 8);
        assert!(text.contains("4.quickfix"), "{text}");
        assert!(!text.contains("항목1"), "{text}");
        assert!(matches!(
            handle(&mut ui, KeyCode::Char('q'), false, Some(&plan), 16),
            Action::Quit
        ));
    }

    #[test]
    fn unfinished_filter_and_history_and_help() {
        let mut plan = plan(3);
        plan.set("1", State::Done, None, None).unwrap();
        let mut ui = Ui::default();
        press(&mut ui, &plan, &[KeyCode::Char('f')]);
        let text = panel(&plan, &ui, 8);
        assert!(!text.contains("항목1") && text.contains("항목2"), "{text}");
        press(&mut ui, &plan, &[KeyCode::Char('h')]);
        assert!(panel(&plan, &ui, 8).contains("done1"));
        press(&mut ui, &plan, &[KeyCode::Char('?')]);
        assert!(panel(&plan, &ui, 12).contains("미완료만보기"));
        assert!(matches!(
            handle(&mut ui, KeyCode::Char('c'), true, Some(&plan), 16),
            Action::Quit
        ));
    }
}
