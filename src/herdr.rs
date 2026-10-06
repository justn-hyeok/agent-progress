use crate::store::Viewer;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{path::Path, process::Command};

pub fn inside() -> bool {
    std::env::var("HERDR_ENV").as_deref() == Ok("1")
}

fn call(args: &[&str]) -> Result<Value> {
    ensure!(inside(), "Herdr 안에서만 진행 창을 열 수 있습니다");
    let out = Command::new("herdr")
        .args(args)
        .output()
        .context("herdr 실행 실패")?;
    ensure!(
        out.status.success(),
        "herdr {} 실패: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    // `pane run` succeeds with empty stdout rather than a JSON envelope.
    if out.stdout.iter().all(u8::is_ascii_whitespace) {
        return Ok(serde_json::json!({"result": {}}));
    }
    let value: Value = serde_json::from_slice(&out.stdout)?;
    ensure!(
        value.get("error").is_none(),
        "herdr 요청 실패: {}",
        value["error"]
    );
    Ok(value)
}

/// Terminal currently behind a pane, or None when the pane is gone.
pub fn terminal_of(pane: &str) -> Option<String> {
    call(&["pane", "get", pane])
        .ok()?
        .pointer("/result/pane/terminal_id")?
        .as_str()
        .map(String::from)
}

fn ancestors() -> Vec<u32> {
    let mut pids = vec![std::process::id()];
    while pids.len() < 64 {
        let pid = pids[pids.len() - 1].to_string();
        let parent = Command::new("ps")
            .args(["-o", "ppid=", "-p", &pid])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u32>().ok());
        match parent {
            Some(ppid) if ppid > 1 && !pids.contains(&ppid) => pids.push(ppid),
            _ => break,
        }
    }
    pids
}

/// True when one of the pane's foreground processes is this process or its ancestor.
pub fn is_ancestor_pane(pane: &str) -> bool {
    let Ok(info) = call(&["pane", "process-info", "--pane", pane]) else {
        return false;
    };
    let Some(procs) = info
        .pointer("/result/process_info/foreground_processes")
        .and_then(Value::as_array)
    else {
        return false;
    };
    let ours = ancestors();
    procs
        .iter()
        .filter_map(|p| p.get("pid").and_then(Value::as_u64))
        .any(|pid| ours.iter().any(|&a| u64::from(a) == pid))
}

fn foreground(pane: &str) -> Vec<String> {
    call(&["pane", "process-info", "--pane", pane])
        .ok()
        .and_then(|v| {
            v.pointer("/result/process_info/foreground_processes")?
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p["name"].as_str().map(String::from))
                        .collect()
                })
        })
        .unwrap_or_default()
}

fn codex_thread_name(thread: &str) -> Option<String> {
    let home = std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".codex")))?;
    let index = std::fs::read_to_string(home.join("session_index.jsonl")).ok()?;
    // Later lines win: a renamed thread appends a new entry.
    index
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| v["id"] == thread)
        .find_map(|v| v["thread_name"].as_str().map(String::from))
        .filter(|n| !n.trim().is_empty())
}

/// The single Herdr pane running Codex whose title is this thread's name and whose
/// cwd is inside the project. None when absent or ambiguous; never a guess.
pub fn codex_pane_for_thread(thread: &str, root: &Path) -> Option<String> {
    let name = codex_thread_name(thread)?;
    let list = call(&["pane", "list"]).ok()?;
    let panes = list
        .pointer("/result/panes")
        .or_else(|| list.get("panes"))?
        .as_array()?;
    let mut found = panes.iter().filter(|p| {
        let title = p["terminal_title_stripped"]
            .as_str()
            .or_else(|| p["terminal_title"].as_str())
            .unwrap_or("");
        let cwd = p["foreground_cwd"]
            .as_str()
            .or_else(|| p["cwd"].as_str())
            .unwrap_or("");
        p["agent"] == "codex"
            && (title == name || title.starts_with(&format!("{name} | ")))
            && Path::new(cwd).starts_with(root)
    });
    let pane = found.next()?.get("pane_id")?.as_str()?.to_string();
    found.next().is_none().then_some(pane)
}

/// The viewer recorded in the plan is the same terminal and still running ap.
pub fn alive(viewer: &Viewer) -> bool {
    terminal_of(&viewer.pane).as_deref() == Some(viewer.terminal_id.as_str())
        && foreground(&viewer.pane).iter().any(|n| n == "ap")
}

/// An idle shell pane directly below `source` (same column, touching edge).
pub fn idle_pane_below(source: &str) -> Option<String> {
    let layout = call(&["pane", "layout", "--pane", source]).ok()?;
    let panes = layout.pointer("/result/layout/panes")?.as_array()?;
    let rect = |p: &Value| -> Option<(u64, u64, u64, u64)> {
        let r = p.get("rect")?;
        Some((
            r["x"].as_u64()?,
            r["y"].as_u64()?,
            r["width"].as_u64()?,
            r["height"].as_u64()?,
        ))
    };
    let (sx, sy, sw, sh) = rect(panes.iter().find(|p| p["pane_id"] == source)?)?;
    let below = panes
        .iter()
        .find(|p| rect(p).is_some_and(|(x, y, w, _)| x == sx && w == sw && y == sy + sh))?;
    let pane = below["pane_id"].as_str()?.to_string();
    let procs = foreground(&pane);
    let idle = !procs.is_empty()
        && procs
            .iter()
            .all(|n| matches!(n.trim_start_matches('-'), "zsh" | "bash" | "fish" | "sh"));
    idle.then_some(pane)
}

/// Run the viewer in the user's existing idle shell below; the pane stays theirs.
pub fn run_in(pane: &str, plan_file: &Path) -> Result<Viewer> {
    let terminal_id = terminal_of(pane).context("pane의 terminal을 확인할 수 없습니다")?;
    let exe = std::env::current_exe()?;
    let command = format!(
        "{} view --file {}",
        shell_quote(exe.to_str().context("binary path encoding")?),
        shell_quote(plan_file.to_str().context("plan path encoding")?)
    );
    call(&["pane", "run", pane, &command])?;
    Ok(Viewer {
        pane: pane.into(),
        terminal_id,
        reused: true,
    })
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Split a pane below `source` without taking focus and run the viewer in it.
pub fn open_below(source: &str, cwd: &Path, plan_file: &Path, size_percent: u8) -> Result<Viewer> {
    let ratio = format!("{:.2}", 1.0 - f64::from(size_percent.clamp(10, 50)) / 100.0);
    let cwd = cwd.to_str().context("cwd encoding")?;
    let split = call(&[
        "pane",
        "split",
        "--pane",
        source,
        "--direction",
        "down",
        "--ratio",
        &ratio,
        "--cwd",
        cwd,
        "--no-focus",
    ])?;
    let pane = split
        .pointer("/result/pane/pane_id")
        .and_then(Value::as_str)
        .context("herdr가 새 pane ID를 반환하지 않았습니다")?
        .to_string();
    let terminal_id = terminal_of(&pane).context("새 pane의 terminal을 확인할 수 없습니다")?;
    // Run this exact binary; a bare `ap` could resolve to another installed version.
    let exe = std::env::current_exe()?;
    let command = format!(
        "exec {} view --file {}",
        shell_quote(exe.to_str().context("binary path encoding")?),
        shell_quote(plan_file.to_str().context("plan path encoding")?)
    );
    call(&["pane", "run", &pane, &command])
        .with_context(|| format!("{pane} 생성 후 viewer 실행 실패; pane은 남아 있습니다"))?;
    Ok(Viewer {
        pane,
        terminal_id,
        reused: false,
    })
}

/// Close only the exact viewer terminal we created.
pub fn close(viewer: &Viewer) -> Result<bool> {
    if !alive(viewer) {
        return Ok(false);
    }
    if viewer.reused {
        // Quit the viewer and hand the shell back; the pane belongs to the user.
        call(&["pane", "send-keys", &viewer.pane, "q"])?;
    } else {
        call(&["pane", "close", &viewer.pane])?;
    }
    Ok(true)
}
