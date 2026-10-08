use crate::{
    pane::Panes,
    store::{Item, Plan, State, Store},
};
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

/// How the bar looks at one instant: the displayed ratio (may lag the plan while
/// sliding), the arrow scale (0 = flat edge, 1 = full arrow) and the edge glow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub ratio: f64,
    pub arrow: f64,
    pub glow: Option<f64>,
}

impl Look {
    /// Static look for a plan: no motion, arrow from 50% up to (not including) 100%.
    pub fn of(plan: Option<&Plan>) -> Self {
        let (done, total) = plan.map_or((0, 0), Plan::progress);
        Look {
            ratio: ratio(done, total),
            arrow: arrow_target(done, total),
            glow: None,
        }
    }
}

/// A value easing from `from` to `to` over `dur` seconds starting at `start`.
#[derive(Clone, Copy, Debug)]
struct Tween {
    from: f64,
    to: f64,
    start: f64,
    dur: f64,
}

impl Tween {
    fn snap(v: f64) -> Self {
        Tween {
            from: v,
            to: v,
            start: 0.0,
            dur: 0.0,
        }
    }

    fn at(&self, t: f64) -> f64 {
        if self.dur <= 0.0 || t >= self.start + self.dur {
            return self.to;
        }
        let x = ((t - self.start) / self.dur).clamp(0.0, 1.0);
        // Ease-out: quick start, gentle arrival.
        self.from + (self.to - self.from) * (1.0 - (1.0 - x).powi(3))
    }

    /// Head for `to` from wherever the value is now, so rapid updates never jump.
    fn retarget(&mut self, t: f64, to: f64, dur: f64) {
        if (to - self.to).abs() > 1e-9 {
            *self = Tween {
                from: self.at(t),
                to,
                start: t,
                dur,
            };
        }
    }

    fn moving(&self, t: f64) -> bool {
        self.dur > 0.0 && t < self.start + self.dur
    }
}

const SLIDE_SECS: f64 = 0.6;
const ARROW_SECS: f64 = 0.45;
const BREATH_SECS: f64 = 2.4;

/// One-shot transitions plus the idle "recent record" breathing, as a pure function of
/// time so it can be tested without sleeping.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    ratio: Tween,
    arrow: Tween,
    arrow_goal: f64,
}

impl Motion {
    /// No transition: first load, or the viewer switched to another plan.
    pub fn snap(plan: Option<&Plan>) -> Self {
        let look = Look::of(plan);
        Motion {
            ratio: Tween::snap(look.ratio),
            arrow: Tween::snap(look.arrow),
            arrow_goal: look.arrow,
        }
    }

    /// The same plan changed: slide the fill from where it is shown now.
    pub fn update(&mut self, t: f64, plan: Option<&Plan>) {
        let look = Look::of(plan);
        self.ratio.retarget(t, look.ratio, SLIDE_SECS);
        self.arrow_goal = look.arrow;
    }

    /// Look at time `t`. The arrow grows once the shown fill reaches half way, and
    /// shrinks as soon as it is no longer wanted. `breathe` lets the edge glow.
    pub fn look(&mut self, t: f64, breathe: bool) -> Look {
        let ratio = self.ratio.at(t);
        let want = if self.arrow_goal > 0.0 && ratio >= 0.5 {
            1.0
        } else {
            0.0
        };
        self.arrow.retarget(t, want, ARROW_SECS);
        let settled = (ratio - self.ratio.to).abs() < 1e-9;
        Look {
            ratio,
            arrow: self.arrow.at(t),
            glow: (breathe && ratio > 0.0 && ratio < 1.0 && settled)
                .then(|| 0.5 - 0.5 * (std::f64::consts::TAU * t / BREATH_SECS).cos()),
        }
    }

    pub fn moving(&self, t: f64) -> bool {
        self.ratio.moving(t) || self.arrow.moving(t)
    }
}

fn ratio(done: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    }
}

/// The arrow appears from half way (integer test, no float edge cases) until done.
fn arrow_target(done: usize, total: usize) -> f64 {
    if total > 0 && done * 2 >= total && done < total {
        1.0
    } else {
        0.0
    }
}

/// Arrow protrusion per row: grows toward the middle by one cell per row, capped at 2
/// (`0·1·2·1·0` for five rows, two apex rows on even heights).
fn arrow_offsets(height: usize) -> Vec<usize> {
    (0..height)
        .map(|r| r.min(height.saturating_sub(1) - r).min(2))
        .collect()
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Per-cell backgrounds (row-major) and per-row optional half-cell tip (column, colour).
type Fill = (Vec<Vec<Color>>, Vec<Option<(usize, Color)>>);

/// Background per cell (row-major) and, per row, an optional half-cell tip `▌` with its
/// colour. The arrow tip sits exactly at the progress edge; outer rows are pulled back.
fn fill_cells(p: &Palette, look: &Look, width: usize, height: usize) -> Fill {
    if look.ratio <= 0.0 || width == 0 {
        return (vec![vec![p.track; width]; height], vec![None; height]);
    }
    if look.ratio >= 1.0 {
        return (vec![vec![p.fill; width]; height], vec![None; height]);
    }
    let edge = look.ratio * width as f64;
    let offsets = arrow_offsets(height);
    let peak = offsets.iter().copied().max().unwrap_or(0) as f64;
    let glow_color = mix(p.fill, p.accent, 0.45);
    let soft = mix(p.track, p.fill, 0.8);
    let mut rows = Vec::with_capacity(height);
    let mut tips = Vec::with_capacity(height);
    for off in offsets {
        let row_edge = (edge - (peak - off as f64) * look.arrow).max(0.0);
        let ramp = (width as f64 / 5.0).clamp(3.0, 14.0).min(row_edge.max(1.0));
        let colour_at = |center: f64| {
            let t = smooth((row_edge - center) / ramp);
            // Flat edge: fade into the track. Arrow: a subtle fade kept inside the fill.
            let flat = mix(p.track, p.fill, t);
            let inner = mix(soft, p.fill, t);
            mix(flat, inner, look.arrow)
        };
        let last = row_edge.floor() as usize;
        let mut row: Vec<Color> = (0..width)
            .map(|x| {
                if (x as f64) + 1.0 <= row_edge {
                    colour_at(x as f64 + 0.5)
                } else {
                    p.track
                }
            })
            .collect();
        let frac = row_edge - row_edge.floor();
        let mut tip = (frac >= 0.5 && last < width).then(|| (last, colour_at(row_edge - 0.25)));
        if let Some(g) = look.glow {
            let lift = |c: Color| mix(c, glow_color, 0.5 * g);
            if last > 0 && last <= width {
                row[last - 1] = lift(row[last - 1]);
            }
            tip = tip.map(|(x, c)| (x, lift(c)));
        }
        rows.push(row);
        tips.push(tip);
    }
    (rows, tips)
}

fn field(label: &str, value: String, value_style: Style, p: &Palette) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label} › "), Style::new().fg(p.muted)),
        Span::styled(value, value_style),
    ])
}

fn draw(
    frame: &mut Frame,
    p: &Palette,
    plan: Option<&Plan>,
    error: Option<&str>,
    source: &str,
    look: &Look,
) {
    let area = frame.area();
    if area.is_empty() {
        return;
    }
    let empty = Plan::new("");
    let plan = plan.unwrap_or(&empty);
    let (done, total) = plan.progress();
    let goal = plan.goal.as_deref().unwrap_or("목표 미정");
    let doing: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.state == State::Doing)
        .collect();
    let blocked: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.state == State::Blocked)
        .collect();
    let next = plan.items.iter().find(|i| i.state == State::Todo);
    let last_done = plan.last_done();
    let pad = if area.width >= 24 {
        2
    } else {
        u16::from(area.width > 4)
    };
    let width = area.width.saturating_sub(pad * 2).min(110);
    let metric = (done * 100)
        .checked_div(total)
        .map(|pct| format!("{pct}% · {done}/{total}"))
        .unwrap_or_else(|| "체크율 미정".into());

    // Middle rows by priority: where we are (or what is stuck), then next, then last done.
    let warn = Style::new().fg(p.warning).add_modifier(Modifier::BOLD);
    let mut middle: Vec<Line> = Vec::new();
    if let Some(error) = error {
        middle.push(field("오래됨", error.to_string(), warn, p));
    }
    // Two items in progress at once means the plan and the work disagree: show each.
    let now_style = if doing.len() > 1 {
        warn
    } else {
        Style::new().fg(p.accent).add_modifier(Modifier::BOLD)
    };
    for item in &doing {
        middle.push(field("지금", item.title.clone(), now_style, p));
    }
    for item in &blocked {
        let mut text = format!(
            "{} — {}",
            item.title,
            item.reason.as_deref().unwrap_or("막힌 이유 확인")
        );
        if let Some(needs) = &item.needs {
            text.push_str(&format!(" (필요: {needs})"));
        }
        middle.push(field("막힘", text, warn, p));
    }
    if doing.is_empty() && blocked.is_empty() {
        if total == 0 {
            middle.push(Line::styled("계획 대기", Style::new().fg(p.muted)));
        } else if done == total {
            middle.push(Line::styled(
                "✓ 완료",
                Style::new().fg(p.accent).add_modifier(Modifier::BOLD),
            ));
        }
    }
    if let Some(item) = next {
        middle.push(field(
            "다음",
            item.title.clone(),
            Style::new().fg(p.text),
            p,
        ));
    }
    if let Some(item) = last_done {
        middle.push(field(
            "완료",
            item.title.clone(),
            Style::new().fg(p.muted),
            p,
        ));
    }

    let rect = |row: u16| Rect {
        x: area.x + pad,
        y: area.y + row,
        width,
        height: 1,
    };
    let clip = |line: Line<'static>| -> Line<'static> {
        // Shorten the value span so the label always stays visible.
        let label_width: usize = line.spans.iter().take(1).map(|s| s.content.width()).sum();
        let mut spans = line.spans;
        if spans.len() == 2 {
            let room = (width as usize).saturating_sub(label_width);
            let value = ellipsize(&spans[1].content, room);
            spans[1] = Span::styled(value, spans[1].style);
        } else if let Some(only) = spans.first_mut() {
            *only = Span::styled(ellipsize(&only.content, width as usize), only.style);
        }
        Line::from(spans)
    };
    if area.height == 1 {
        let now = doing
            .first()
            .map(|i| format!("지금 › {}", i.title))
            .unwrap_or_default();
        let line = Line::styled(format!("{metric}  {now}"), Style::new().fg(p.text));
        frame.render_widget(Paragraph::new(clip(line)), rect(0));
    } else {
        let header_goal = if error.is_some() {
            format!("오래된 상태 · {goal}")
        } else {
            goal.to_string()
        };
        let (pct, counts) = metric
            .split_once(" · ")
            .map_or((metric.as_str(), None), |(a, b)| (a, Some(b)));
        let left = width.saturating_sub(metric.width() as u16 + 2) as usize;
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
        frame.render_widget(Paragraph::new(Line::from(spans)), rect(0));
        // The file path stays dim on the bottom row; the middle fills what is left.
        let file_row = (area.height >= 3).then(|| area.height - 1);
        let middle_rows = file_row.unwrap_or(area.height) - 1;
        let mut rows: Vec<Line> = middle;
        if area.height >= 8 && total > 0 {
            rows.push(Line::styled(
                format!("체크리스트 › {done}/{total} 완료 · 에이전트 보고 기준"),
                Style::new().fg(p.muted),
            ));
            rows.extend(plan.items.iter().map(|item| {
                let mut s = format!("{} {}", item.state.mark(), item.title);
                if let Some(r) = &item.reason {
                    s.push_str(&format!(" — {r}"));
                }
                Line::styled(s, Style::new().fg(p.muted))
            }));
        }
        for (i, line) in rows.into_iter().take(middle_rows as usize).enumerate() {
            frame.render_widget(Paragraph::new(clip(line)), rect(1 + i as u16));
        }
        if let Some(row) = file_row {
            let line = field("파일", source.to_string(), Style::new().fg(p.metadata), p);
            frame.render_widget(Paragraph::new(clip(line)), rect(row));
        }
    }
    paint(frame, p, look);
}

/// Paint the bar under the text. A half-cell tip goes only into a truly blank cell, and
/// both halves of a wide (e.g. Korean) glyph keep one background so it stays legible.
fn paint(frame: &mut Frame, p: &Palette, look: &Look) {
    let area = frame.area();
    let (rows, tips) = fill_cells(p, look, usize::from(area.width), usize::from(area.height));
    let buf = frame.buffer_mut();
    for (r, row) in rows.iter().enumerate() {
        let y = area.y + r as u16;
        let mut wide_prev = false;
        for (c, bg) in row.iter().enumerate() {
            let x = area.x + c as u16;
            let cell = &mut buf[(x, y)];
            if wide_prev {
                // Continuation half of the previous glyph: match its background.
                let prev_bg = row[c - 1];
                cell.bg = prev_bg;
                wide_prev = false;
                continue;
            }
            cell.bg = *bg;
            wide_prev = cell.symbol().width() == 2;
        }
        if let Some((c, colour)) = tips[r] {
            let x = area.x + c as u16;
            let blank = buf[(x, y)].symbol() == " ";
            let after_wide = c > 0 && buf[(x - 1, y)].symbol().width() == 2;
            if blank && !after_wide {
                let cell = &mut buf[(x, y)];
                cell.set_symbol("▌");
                cell.fg = colour;
                cell.bg = p.track;
            }
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

/// A manually started viewer (`ap view`): shows one plan file, changes nothing on exit.
pub fn watch(store: &Store, idle_secs: u64) -> Result<()> {
    run(Store::at(store.path.clone()), None, idle_secs)
}

/// An automatic viewer: follows its pane's state file, switching to whatever plan that
/// pane changed last, keeps a heartbeat, exits once the state stops naming `instance`,
/// and records the user's dismissal on q.
pub fn follow(panes: &Panes, instance: &str, idle_secs: u64) -> Result<()> {
    let Some(state) = panes.load() else {
        return Ok(());
    };
    run(Store::at(state.plan), Some((panes, instance)), idle_secs)
}

/// Default for how long after the last record the edge keeps breathing.
pub const IDLE_SECS: u64 = 300;

fn file_mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn source_label(store: &Store) -> String {
    store
        .path
        .components()
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<std::path::PathBuf>()
        .display()
        .to_string()
}

fn run(mut store: Store, follow: Option<(&Panes, &str)>, idle_secs: u64) -> Result<()> {
    let mut plan = store.load().ok().flatten();
    let clock = Instant::now();
    let t = || clock.elapsed().as_secs_f64();
    let mut motion = Motion::snap(plan.as_ref());
    let mut error: Option<String> = None;
    let mut seen = mtime(&store);
    let mut seen_pane = follow.and_then(|(panes, _)| file_mtime(&panes.path));
    let mut source = source_label(&store);
    let mut colors = palette(&store);
    let mut ui = Ui::default();
    let mut history: Vec<String> = Vec::new();
    let mut last_beat = Instant::now() - BEAT_EVERY;
    let mut terminal = ratatui::init();
    let result = (|| -> Result<bool> {
        loop {
            if let Some((panes, id)) = follow {
                if last_beat.elapsed() >= BEAT_EVERY {
                    panes.beat(id)?;
                    last_beat = Instant::now();
                }
                let now = file_mtime(&panes.path);
                if now != seen_pane {
                    seen_pane = now;
                    if let Some(state) = panes.load() {
                        // `ap close`, `ap open` or a newer viewer replaced us.
                        if state.viewer.as_ref().is_none_or(|v| v.instance != id) {
                            return Ok(false);
                        }
                        if state.plan != store.path {
                            store = Store::at(state.plan);
                            plan = store.load().ok().flatten();
                            motion = Motion::snap(plan.as_ref());
                            error = None;
                            seen = mtime(&store);
                            source = source_label(&store);
                            colors = palette(&store);
                            ui = Ui::default();
                        }
                    }
                }
            }
            // Reload before handling input so a burst of keys never delays updates.
            let current = mtime(&store);
            if current != seen {
                seen = current;
                match store.load() {
                    Ok(p) => {
                        plan = p;
                        error = None;
                        motion.update(t(), plan.as_ref());
                        if ui.mode == Mode::History {
                            history = history_lines(&store);
                        }
                    }
                    // Keep the last good plan visible and say it is stale.
                    Err(e) => error = Some(e.to_string()),
                }
            }
            // Breathing means "recorded recently", not proof the agent is running.
            let recent = plan
                .as_ref()
                .is_some_and(|p| crate::store::now().saturating_sub(p.updated_at) <= idle_secs);
            let look = motion.look(t(), recent && error.is_none());
            terminal.draw(|f| {
                if ui.mode == Mode::Bar && !ui.typing {
                    draw(f, &colors, plan.as_ref(), error.as_deref(), &source, &look)
                } else {
                    draw_panel(f, &colors, plan.as_ref(), &ui, &history)
                }
            })?;
            // ~8 fps only while something moves; otherwise a slow poll. ratatui redraws
            // only the cells that changed.
            let tick = if motion.moving(t()) || look.glow.is_some() {
                125
            } else {
                500
            };
            if event::poll(Duration::from_millis(tick))?
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
                    history = history_lines(&store);
                }
            }
        }
    })();
    ratatui::restore();
    if let Some((panes, id)) = follow {
        panes.clear_beat(id);
        if result? {
            // The user dismissed this pane's viewer: don't pop it back up.
            panes.update(|s| {
                if s.viewer.as_ref().is_some_and(|v| v.instance == id) {
                    s.viewer = None;
                    s.suppressed = true;
                }
            })?;
        }
        return Ok(());
    }
    result.map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn render(plan: &Plan, width: u16, height: u16, look: &Look) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| draw(f, &SIGNAL, Some(plan), None, "plans/x.json", look))
            .unwrap();
        terminal
    }

    fn text_of(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
            // Wide (Korean) cells are followed by a blank continuation cell.
            .replace(' ', "")
    }

    fn screen(plan: &Plan, width: u16, height: u16) -> String {
        text_of(&render(plan, width, height, &Look::of(Some(plan))))
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
    fn one_now_line_and_role_labels() {
        let mut plan = plan(4);
        plan.set("1", State::Done, None, None).unwrap();
        plan.set("2", State::Doing, None, None).unwrap();
        let text = screen(&plan, 80, 7);
        assert!(
            text.lines().next().unwrap().contains("목표25%·1/4"),
            "{text}"
        );
        assert_eq!(
            text.matches("항목2").count(),
            1,
            "current step shown once: {text}"
        );
        assert!(text.contains("지금›항목2"), "{text}");
        assert!(text.contains("다음›항목3"), "{text}");
        assert!(text.contains("완료›항목1"), "{text}");
        assert!(
            text.lines().last().unwrap().contains("파일›plans/x.json"),
            "{text}"
        );
        assert!(
            !text.contains("계획상진행") && !text.contains("현재계획"),
            "{text}"
        );
    }

    #[test]
    fn two_steps_in_progress_are_both_shown_as_a_warning() {
        let mut plan = plan(3);
        plan.set("1", State::Doing, None, None).unwrap();
        plan.set("2", State::Doing, None, None).unwrap();
        let terminal = render(&plan, 80, 7, &Look::of(Some(&plan)));
        let text = text_of(&terminal);
        assert!(
            text.contains("지금›항목1") && text.contains("지금›항목2"),
            "{text}"
        );
        let buffer = terminal.backend().buffer();
        let row = (0..7)
            .find(|&y| {
                (0..80)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .replace(' ', "")
                    .contains("항목1")
            })
            .unwrap();
        let value_cell = (0..80)
            .find(|&x| buffer[(x, row)].symbol() == "항")
            .unwrap();
        assert_eq!(buffer[(value_cell, row)].fg, SIGNAL.warning);
    }

    #[test]
    fn titles_with_middle_dots_keep_their_label() {
        let mut plan = Plan::new("t");
        plan.add("저장소·고정 번호·이력").unwrap();
        plan.set("1", State::Doing, None, None).unwrap();
        assert!(screen(&plan, 80, 5).contains("지금›저장소·고정번호·이력"));
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
        assert!(text.contains("막힘›항목3—키필요(필요:user)"), "{text}");
    }

    #[test]
    fn empty_plan_is_undetermined() {
        let text = screen(&Plan::new("t"), 60, 4);
        assert!(text.contains("체크율미정"), "{text}");
        assert!(text.contains("계획대기"), "{text}");
    }

    #[test]
    fn tall_pane_lists_checklist_above_the_file_row() {
        let mut plan = plan(3);
        plan.set("1", State::Done, None, None).unwrap();
        let text = screen(&plan, 80, 12);
        assert!(text.contains("체크리스트›1/3완료"), "{text}");
        assert!(text.contains("[x]항목1"), "{text}");
        assert!(text.lines().last().unwrap().contains("파일›"), "{text}");
    }

    #[test]
    fn arrow_shape_by_height() {
        assert_eq!(arrow_offsets(5), vec![0, 1, 2, 1, 0]);
        assert_eq!(arrow_offsets(6), vec![0, 1, 2, 2, 1, 0]);
        assert_eq!(arrow_offsets(7), vec![0, 1, 2, 2, 2, 1, 0]);
    }

    #[test]
    fn arrow_only_from_half_until_done() {
        let at = |done, total| {
            let mut p = plan(total);
            for i in 1..=done {
                p.set(&i.to_string(), State::Done, None, None).unwrap();
            }
            Look::of(Some(&p))
        };
        assert_eq!(at(1, 2).arrow, 1.0);
        assert_eq!(at(49, 100).arrow, 0.0);
        assert_eq!(at(3, 3).arrow, 0.0);
        assert_eq!(at(0, 3).arrow, 0.0);
    }

    fn filled(row: &[Color]) -> usize {
        row.iter().filter(|c| **c != SIGNAL.track).count()
    }

    #[test]
    fn arrow_tip_sits_at_the_edge_and_outer_rows_pull_back() {
        let look = Look {
            ratio: 0.5,
            arrow: 1.0,
            glow: None,
        };
        let (rows, _) = fill_cells(&SIGNAL, &look, 40, 5);
        assert_eq!(filled(&rows[2]), 20, "apex reaches the progress edge");
        assert_eq!(filled(&rows[1]), 19);
        assert_eq!(filled(&rows[0]), 18);
        assert_eq!(rows[0], rows[4]);
    }

    #[test]
    fn below_half_is_a_flat_fade_and_extremes_are_solid() {
        let look = Look {
            ratio: 0.3,
            arrow: 0.0,
            glow: None,
        };
        let (rows, _) = fill_cells(&SIGNAL, &look, 40, 5);
        assert!(rows.iter().all(|r| r == &rows[0]));
        assert_eq!(rows[0][0], SIGNAL.fill);
        assert_eq!(rows[0][12], SIGNAL.track);
        let solid = |r: f64| {
            fill_cells(
                &SIGNAL,
                &Look {
                    ratio: r,
                    arrow: 0.0,
                    glow: None,
                },
                30,
                3,
            )
            .0
        };
        assert!(solid(1.0).iter().flatten().all(|c| *c == SIGNAL.fill));
        assert!(solid(0.0).iter().flatten().all(|c| *c == SIGNAL.track));
    }

    #[test]
    fn arrow_fade_stays_inside_the_fill() {
        let look = Look {
            ratio: 0.6,
            arrow: 1.0,
            glow: None,
        };
        let (rows, _) = fill_cells(&SIGNAL, &look, 50, 5);
        let soft = mix(SIGNAL.track, SIGNAL.fill, 0.8);
        // Last filled cell is the softest but never the bare track colour.
        let last = rows[2][filled(&rows[2]) - 1];
        assert_ne!(last, SIGNAL.track);
        assert_ne!(last, SIGNAL.fill);
        let Color::Rgb(_, g_last, _) = last else {
            panic!()
        };
        let Color::Rgb(_, g_soft, _) = soft else {
            panic!()
        };
        assert!(g_last >= g_soft);
    }

    #[test]
    fn half_cell_tip_never_lands_on_a_wide_glyph() {
        let mut plan = Plan::new("t");
        plan.goal = Some("가나다라마바사아자차카타파하".into());
        plan.add("하나").unwrap();
        plan.add("둘").unwrap();
        plan.set("1", State::Done, None, None).unwrap();
        for tenths in 0..40 {
            let look = Look {
                ratio: 0.5 + f64::from(tenths) / 400.0,
                arrow: 1.0,
                glow: Some(1.0),
            };
            let terminal = render(&plan, 70, 7, &look);
            let text = text_of(&terminal);
            assert!(text.contains("가나다라마바사아자차카타파하"), "{text}");
            let buffer = terminal.backend().buffer();
            for y in 0..7 {
                for x in 1..70 {
                    if buffer[(x, y)].symbol() == "▌" {
                        assert_ne!(buffer[(x - 1, y)].symbol().width(), 2);
                    }
                    // The backend never receives a wide glyph's continuation cell (the
                    // terminal paints it with the glyph); check it when it is present.
                    if buffer[(x - 1, y)].symbol().width() == 2 && buffer[(x, y)].bg != Color::Reset
                    {
                        assert_eq!(
                            buffer[(x, y)].bg,
                            buffer[(x - 1, y)].bg,
                            "glyph halves share a background"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn completion_slides_and_the_arrow_grows_after_half() {
        let mut p = plan(4);
        p.set("1", State::Done, None, None).unwrap();
        let mut motion = Motion::snap(Some(&p));
        assert_eq!(motion.look(0.0, false).ratio, 0.25);
        p.set("2", State::Done, None, None).unwrap();
        p.set("3", State::Done, None, None).unwrap();
        motion.update(10.0, Some(&p));
        let mid = motion.look(10.1, false);
        assert!(mid.ratio > 0.25 && mid.ratio < 0.75, "{mid:?}");
        assert!(motion.moving(10.1));
        let settled = motion.look(10.6, false);
        assert_eq!(settled.ratio, 0.75);
        // The arrow starts growing once the shown fill passed half way.
        let grown = motion.look(10.6 + ARROW_SECS, false);
        assert_eq!(grown.arrow, 1.0);
        assert!(!motion.moving(11.2));
    }

    #[test]
    fn breathing_only_when_settled_recent_and_unfinished() {
        let mut p = plan(2);
        p.set("1", State::Done, None, None).unwrap();
        let mut motion = Motion::snap(Some(&p));
        assert!(motion.look(1.0, true).glow.is_some());
        assert!(
            motion.look(1.0, false).glow.is_none(),
            "stale: no breathing"
        );
        p.set("2", State::Done, None, None).unwrap();
        motion.update(5.0, Some(&p));
        assert!(
            motion.look(5.1, true).glow.is_none(),
            "no breathing while sliding"
        );
        assert!(
            motion.look(9.0, true).glow.is_none(),
            "no breathing at 100%"
        );
    }

    #[test]
    fn switching_plans_snaps_without_sliding() {
        let a = plan(4);
        let mut b = plan(2);
        b.set("1", State::Done, None, None).unwrap();
        let _ = Motion::snap(Some(&a));
        let mut motion = Motion::snap(Some(&b));
        assert_eq!(motion.look(0.0, false).ratio, 0.5);
        assert!(!motion.moving(0.0));
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

    /// `AP_PREVIEW_DIR=… cargo test dump_previews -- --ignored` writes cell dumps that
    /// scripts/render_preview.py turns into PNGs for a visual check.
    #[test]
    #[ignore]
    fn dump_previews() {
        let dir = std::path::PathBuf::from(std::env::var("AP_PREVIEW_DIR").unwrap());
        let mut p = Plan::new("t");
        p.goal = Some("진행 창 UI 개선".into());
        for i in 1..=50 {
            p.add(&format!("단계 {i} · 세부 작업")).unwrap();
        }
        for (name, done, glow) in [
            ("030", 15, None),
            ("050", 25, Some(0.0)),
            ("060", 30, Some(1.0)),
            ("094", 47, Some(0.5)),
            ("100", 50, None),
        ] {
            let mut plan = p.clone();
            for i in 1..=done {
                plan.set(&i.to_string(), State::Done, None, None).unwrap();
            }
            if done < 50 {
                plan.set(&(done + 1).to_string(), State::Doing, None, None)
                    .unwrap();
            }
            let mut look = Look::of(Some(&plan));
            look.glow = glow;
            let terminal = render(&plan, 96, 7, &look);
            let buffer = terminal.backend().buffer();
            let rgb = |c: Color| match c {
                Color::Rgb(r, g, b) => vec![r, g, b],
                _ => vec![],
            };
            let cells: Vec<Vec<serde_json::Value>> = (0..7)
                .map(|y| {
                    (0..96)
                        .map(|x| {
                            let c = &buffer[(x, y)];
                            serde_json::json!({"s": c.symbol(), "fg": rgb(c.fg), "bg": rgb(c.bg),
                                "b": c.modifier.contains(Modifier::BOLD)})
                        })
                        .collect()
                })
                .collect();
            std::fs::write(
                dir.join(format!("{name}.json")),
                serde_json::to_vec(&cells).unwrap(),
            )
            .unwrap();
        }
    }
}
