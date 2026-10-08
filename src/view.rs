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
    /// The travelling light: cool, so the fill never turns yellowish.
    glow: Color,
}

/// Signal preset in cool deep green: blue-green fill and a mint travelling light,
/// with the brighter secondary text the user tuned in 1.x.
const SIGNAL: Palette = Palette {
    track: Color::Rgb(0x0C, 0x13, 0x16),
    fill: Color::Rgb(0x1F, 0x48, 0x42),
    accent: Color::Rgb(0xC7, 0xF9, 0x6C),
    text: Color::Rgb(0xF2, 0xF5, 0xEE),
    muted: Color::Rgb(0xC4, 0xD0, 0xC6),
    metadata: Color::Rgb(0xB6, 0xC4, 0xBA),
    warning: Color::Rgb(0xF5, 0xC2, 0x6F),
    glow: Color::Rgb(0x5C, 0xD6, 0xB8),
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
        glow: pick("glow", SIGNAL.glow),
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
/// sliding) and, while recently recorded, where the travelling light is (0..1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub ratio: f64,
    pub sweep: Option<f64>,
}

impl Look {
    /// Static look for a plan: no motion.
    pub fn of(plan: Option<&Plan>) -> Self {
        let (done, total) = plan.map_or((0, 0), Plan::progress);
        Look {
            ratio: ratio(done, total),
            sweep: None,
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
const SWEEP_SECS: f64 = 2.8;

/// The completion slide plus the idle travelling light, as a pure function of time so
/// it can be tested without sleeping.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    ratio: Tween,
    /// Last time the light was allowed to flow; the sweep then running is finished.
    last_flow: Option<f64>,
}

impl Motion {
    /// No transition: first load, or the viewer switched to another plan.
    pub fn snap(plan: Option<&Plan>) -> Self {
        Motion {
            ratio: Tween::snap(Look::of(plan).ratio),
            last_flow: None,
        }
    }

    /// The same plan changed: slide the fill from where it is shown now.
    pub fn update(&mut self, t: f64, plan: Option<&Plan>) {
        self.ratio.retarget(t, Look::of(plan).ratio, SLIDE_SECS);
    }

    /// Look at time `t`. `flow` lets the light travel through the fill toward the edge;
    /// when it stops being allowed, the sweep already under way still runs to the edge
    /// and fades out instead of vanishing mid-way. No light while sliding, on an empty
    /// plan or a finished one.
    pub fn look(&mut self, t: f64, flow: bool) -> Look {
        let ratio = self.ratio.at(t);
        let settled = !self.ratio.moving(t);
        let able = ratio > 0.0 && ratio < 1.0 && settled;
        if flow && able {
            self.last_flow = Some(t);
        }
        let finishing = self.last_flow.is_some_and(|last| {
            let cycle_end = ((last / SWEEP_SECS).floor() + 1.0) * SWEEP_SECS;
            t < cycle_end
        });
        Look {
            ratio,
            sweep: (able && (flow || finishing)).then(|| (t / SWEEP_SECS).rem_euclid(1.0)),
        }
    }

    pub fn moving(&self, t: f64) -> bool {
        self.ratio.moving(t)
    }
}

fn ratio(done: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    }
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Colours at twice the vertical resolution (two pixel rows per terminal row): solid
/// fill fading widely into the track at the progress edge. A soft band of light travels
/// from the left toward the edge; past half way it bends into a `>` (middle rows lead,
/// two cells per row so it reads as ~45°) and fades out as it reaches the edge.
fn pixels(p: &Palette, look: &Look, width: usize, height: usize) -> Vec<Vec<Color>> {
    let rows = height * 2;
    if look.ratio <= 0.0 || width == 0 {
        return vec![vec![p.track; width]; rows];
    }
    if look.ratio >= 1.0 {
        return vec![vec![p.fill; width]; rows];
    }
    let edge = look.ratio * width as f64;
    let ramp = (width as f64 * 0.35).clamp(6.0, 40.0).min(edge.max(1.0));
    let band = (width as f64 * 0.12).clamp(4.0, 16.0);
    let half = height as f64 / 2.0;
    (0..rows)
        .map(|py| {
            // Distance of this pixel row from the vertical middle, 0 (middle) to 1 (edge).
            let y = (py as f64 + 0.5) / 2.0;
            let from_middle = ((y - half) / half).abs().min(1.0);
            (0..width)
                .map(|x| {
                    let center = x as f64 + 0.5;
                    if center > edge {
                        return p.track;
                    }
                    let t = smooth((edge - center) / ramp);
                    let base = mix(p.track, p.fill, t);
                    let Some(phase) = look.sweep else {
                        return base;
                    };
                    let bend = smooth((phase - 0.45) / 0.55);
                    let lead = bend * 2.0 * half * (1.0 - from_middle);
                    let at = phase * (edge + band) - band / 2.0 - 2.0 * half * bend + lead;
                    let width_now = band * (1.0 - 0.45 * bend);
                    let d = ((center - at) / (width_now / 2.0)).abs();
                    let light = if d < 1.0 {
                        0.5 + 0.5 * (std::f64::consts::PI * d).cos()
                    } else {
                        0.0
                    };
                    let fade = 1.0 - smooth((phase - 0.7) / 0.3);
                    mix(base, p.glow, 0.34 * light * fade * (0.3 + 0.7 * t))
                })
                .collect()
        })
        .collect()
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

/// Paint the bar under the text at twice the vertical resolution: a blank cell shows
/// its two pixels with `▀` (top as foreground, bottom as background); a cell holding
/// text gets their average so the text stays as is. Both halves of a wide (e.g. Korean)
/// glyph share one background so it stays legible.
fn paint(frame: &mut Frame, p: &Palette, look: &Look) {
    let area = frame.area();
    let px = pixels(p, look, usize::from(area.width), usize::from(area.height));
    let buf = frame.buffer_mut();
    for r in 0..usize::from(area.height) {
        let y = area.y + r as u16;
        let (top, bottom) = (&px[2 * r], &px[2 * r + 1]);
        let mut wide_prev: Option<Color> = None;
        for c in 0..usize::from(area.width) {
            let cell = &mut buf[(area.x + c as u16, y)];
            if let Some(bg) = wide_prev.take() {
                cell.bg = bg;
                continue;
            }
            let (t, b) = (top[c], bottom[c]);
            if cell.symbol() == " " && t != b {
                cell.set_symbol("▀");
                cell.fg = t;
                cell.bg = b;
            } else {
                cell.bg = mix(t, b, 0.5);
                if cell.symbol().width() == 2 {
                    wide_prev = Some(cell.bg);
                }
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
            let tick = if motion.moving(t()) || look.sweep.is_some() {
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

    fn green(c: Color) -> u8 {
        match c {
            Color::Rgb(_, g, _) => g,
            _ => 0,
        }
    }

    #[test]
    fn fill_fades_widely_into_the_track_and_extremes_are_solid() {
        let look = Look {
            ratio: 0.6,
            sweep: None,
        };
        let rows = pixels(&SIGNAL, &look, 100, 7);
        assert!(rows.iter().all(|r| r == &rows[0]));
        let row = &rows[0];
        assert_eq!(row[0], SIGNAL.fill);
        assert_eq!(row[60], SIGNAL.track);
        let fading = row[..60].iter().filter(|c| **c != SIGNAL.fill).count();
        assert!(fading >= 30, "a wide gradient: {fading}");
        let greens: Vec<u8> = row[..60].iter().map(|c| green(*c)).collect();
        assert!(greens.windows(2).all(|w| w[1] <= w[0]), "{greens:?}");
        let solid = |r: f64| {
            pixels(
                &SIGNAL,
                &Look {
                    ratio: r,
                    sweep: None,
                },
                30,
                3,
            )
        };
        assert!(solid(1.0).iter().flatten().all(|c| *c == SIGNAL.fill));
        assert!(solid(0.0).iter().flatten().all(|c| *c == SIGNAL.track));
    }

    #[test]
    fn light_travels_toward_the_edge_and_stays_inside_the_fill() {
        let brightest = |phase: f64| {
            let look = Look {
                ratio: 0.6,
                sweep: Some(phase),
            };
            let plain = pixels(
                &SIGNAL,
                &Look {
                    ratio: 0.6,
                    sweep: None,
                },
                100,
                7,
            );
            let lit = pixels(&SIGNAL, &look, 100, 7);
            assert!(
                lit.iter()
                    .all(|r| r[60..].iter().all(|c| *c == SIGNAL.track)),
                "never past the edge"
            );
            (0..100)
                .max_by_key(|&x| green(lit[7][x]).saturating_sub(green(plain[7][x])))
                .unwrap()
        };
        let early = brightest(0.2);
        let late = brightest(0.7);
        assert!(early < late, "light moves right: {early} -> {late}");
    }

    fn lift_by_row(phase: f64) -> Vec<(usize, u8)> {
        let plain = pixels(
            &SIGNAL,
            &Look {
                ratio: 0.6,
                sweep: None,
            },
            100,
            7,
        );
        let lit = pixels(
            &SIGNAL,
            &Look {
                ratio: 0.6,
                sweep: Some(phase),
            },
            100,
            7,
        );
        (0..14)
            .map(|y| {
                (0..100)
                    .map(|x| (x, green(lit[y][x]).saturating_sub(green(plain[y][x]))))
                    .max_by_key(|(_, l)| *l)
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn light_is_a_straight_band_first_then_bends_into_an_arrow() {
        let early = lift_by_row(0.3);
        assert!(
            early.iter().all(|(x, _)| x.abs_diff(early[7].0) <= 1),
            "straight: {early:?}"
        );
        let late = lift_by_row(0.85);
        assert!(late[7].0 > late[0].0 + 3, "middle leads: {late:?}");
        assert!(
            late[3].0 < late[7].0 && late[3].0 > late[0].0,
            "a > shape: {late:?}"
        );
        assert_eq!(late[0].0.abs_diff(late[13].0), 0, "symmetric");
    }

    #[test]
    fn light_fades_out_as_it_reaches_the_edge() {
        let strength = |phase: f64| lift_by_row(phase).iter().map(|(_, l)| *l).max().unwrap();
        assert!(strength(0.5) > 0);
        assert!(
            strength(0.99) < strength(0.5) / 4,
            "{} vs {}",
            strength(0.99),
            strength(0.5)
        );
    }

    #[test]
    fn a_stopped_flow_finishes_the_current_sweep() {
        let mut p = plan(2);
        p.set("1", State::Done, None, None).unwrap();
        let mut motion = Motion::snap(Some(&p));
        assert!(motion.look(1.0, true).sweep.is_some());
        let mid = motion.look(1.5, false);
        assert!(mid.sweep.is_some(), "the sweep under way keeps going");
        assert!(motion.look(SWEEP_SECS - 0.01, false).sweep.is_some());
        assert!(
            motion.look(SWEEP_SECS + 0.01, false).sweep.is_none(),
            "then stops"
        );
    }

    #[test]
    fn blank_cells_draw_two_pixels_and_text_cells_keep_their_text() {
        let mut plan = plan(2);
        plan.set("1", State::Done, None, None).unwrap();
        let look = Look {
            ratio: 0.5,
            sweep: Some(0.85),
        };
        let terminal = render(&plan, 70, 7, &look);
        let buffer = terminal.backend().buffer();
        let halves = (0..7)
            .flat_map(|y| (0..70).map(move |x| (x, y)))
            .filter(|&(x, y)| buffer[(x, y)].symbol() == "▀")
            .count();
        assert!(halves > 0, "the bent light uses half-cell pixels");
        assert!(text_of(&terminal).contains("지금›") || text_of(&terminal).contains("다음›항목2"));
    }

    #[test]
    fn glyph_halves_share_a_background_on_the_gradient() {
        let mut plan = Plan::new("t");
        plan.goal = Some("가나다라마바사아자차카타파하".into());
        plan.add("하나").unwrap();
        plan.add("둘").unwrap();
        plan.set("1", State::Done, None, None).unwrap();
        for step in 0..20 {
            let look = Look {
                ratio: 0.5,
                sweep: Some(f64::from(step) / 20.0),
            };
            let terminal = render(&plan, 70, 7, &look);
            assert!(text_of(&terminal).contains("가나다라마바사아자차카타파하"));
            let buffer = terminal.backend().buffer();
            for y in 0..7 {
                for x in 1..70 {
                    // The backend never receives a wide glyph's continuation cell (the
                    // terminal paints it with the glyph); check it when it is present.
                    if buffer[(x - 1, y)].symbol().width() == 2 && buffer[(x, y)].bg != Color::Reset
                    {
                        assert_eq!(buffer[(x, y)].bg, buffer[(x - 1, y)].bg);
                    }
                }
            }
        }
    }

    #[test]
    fn completion_slides_from_where_the_fill_is_shown() {
        let mut p = plan(4);
        p.set("1", State::Done, None, None).unwrap();
        let mut motion = Motion::snap(Some(&p));
        assert_eq!(motion.look(0.0, false).ratio, 0.25);
        p.set("2", State::Done, None, None).unwrap();
        motion.update(10.0, Some(&p));
        let mid = motion.look(10.1, false).ratio;
        assert!(mid > 0.25 && mid < 0.5, "{mid}");
        p.set("3", State::Done, None, None).unwrap();
        motion.update(10.2, Some(&p));
        assert!(
            motion.look(10.2, false).ratio < 0.5,
            "no jump on a quick second update"
        );
        assert_eq!(motion.look(10.2 + SLIDE_SECS, false).ratio, 0.75);
        assert!(!motion.moving(11.0));
    }

    #[test]
    fn light_only_when_settled_recent_and_unfinished() {
        let mut p = plan(2);
        p.set("1", State::Done, None, None).unwrap();
        let mut motion = Motion::snap(Some(&p));
        assert!(motion.look(1.0, true).sweep.is_some());
        assert!(
            motion.look(SWEEP_SECS + 0.5, false).sweep.is_none(),
            "stale: no light once the sweep under way has finished"
        );
        p.set("2", State::Done, None, None).unwrap();
        motion.update(5.0, Some(&p));
        assert!(
            motion.look(5.1, true).sweep.is_none(),
            "no light while sliding"
        );
        assert!(motion.look(9.0, true).sweep.is_none(), "no light at 100%");
        assert!(
            Motion::snap(Some(&plan(3))).look(1.0, true).sweep.is_none(),
            "none at 0%"
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
        let cases: Vec<(String, usize, Option<f64>)> = std::env::var("AP_PREVIEW_CASES")
            .unwrap_or_else(|_| "15,25,30,47,50".into())
            .split(',')
            .map(|d| {
                let done: usize = d.trim().parse().unwrap();
                (
                    format!("{done:03}"),
                    done,
                    (done < 50).then(|| {
                        std::env::var("AP_PREVIEW_SWEEP")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0.6)
                    }),
                )
            })
            .collect();
        for (name, done, glow) in cases {
            let mut plan = p.clone();
            for i in 1..=done {
                plan.set(&i.to_string(), State::Done, None, None).unwrap();
            }
            if done < 50 {
                plan.set(&(done + 1).to_string(), State::Doing, None, None)
                    .unwrap();
            }
            let mut look = Look::of(Some(&plan));
            look.sweep = glow;
            let mut theme = SIGNAL;
            if let Ok(spec) = std::env::var("AP_PREVIEW_THEME") {
                let c: Vec<Color> = spec
                    .split(',')
                    .map(|h| hex(Some(&serde_json::json!(h))).unwrap())
                    .collect();
                (theme.track, theme.fill, theme.glow) = (c[0], c[1], c[2]);
                if let Some(accent) = c.get(3) {
                    theme.accent = *accent;
                }
            }
            let mut terminal = Terminal::new(TestBackend::new(96, 7)).unwrap();
            terminal
                .draw(|f| draw(f, &theme, Some(&plan), None, "plans/x.json", &look))
                .unwrap();
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
