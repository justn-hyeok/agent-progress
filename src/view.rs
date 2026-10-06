use crate::store::{Plan, State, Store};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::time::{Duration, SystemTime};
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

pub fn watch(store: &Store) -> Result<()> {
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
    let mut terminal = ratatui::init();
    let result = (|| -> Result<bool> {
        loop {
            terminal.draw(|f| draw(f, &colors, plan.as_ref(), error.as_deref(), &source))?;
            if event::poll(Duration::from_millis(500))? {
                if let Event::Key(key) = event::read()? {
                    let ctrl_c = key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if key.kind == KeyEventKind::Press && (key.code == KeyCode::Char('q') || ctrl_c)
                    {
                        return Ok(true);
                    }
                }
                continue;
            }
            let current = mtime(store);
            if current != seen {
                seen = current;
                match store.load() {
                    Ok(p) => {
                        plan = p;
                        error = None;
                    }
                    // Keep the last good plan visible and say it is stale.
                    Err(e) => error = Some(e.to_string()),
                }
            }
        }
    })();
    ratatui::restore();
    if result? {
        // The user dismissed the viewer: don't pop it back up on the next update.
        if let Some(p) = &plan {
            store.update(&p.key, "", |p| {
                p.view_suppressed = true;
                p.viewer = None;
                Ok(())
            })?;
        }
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
}
