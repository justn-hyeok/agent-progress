use crate::{herdr::shell_quote, store::Viewer};
use anyhow::{Context, Result, ensure};
use std::{path::Path, process::Command};

fn tmux(args: &[&str]) -> Result<String> {
    let out = Command::new("tmux")
        .args(args)
        .output()
        .context("tmux 실행 실패")?;
    ensure!(
        out.status.success(),
        "tmux {} 실패: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// tmux pane IDs (%N) are not reused within a server, so the ID alone identifies the viewer.
pub fn alive(viewer: &Viewer) -> bool {
    tmux(&["display-message", "-p", "-t", &viewer.pane, "#{pane_id}"])
        .is_ok_and(|id| id == viewer.pane)
}

pub fn open_below(source: &str, cwd: &Path, plan_file: &Path, size_percent: u8) -> Result<Viewer> {
    let exe = std::env::current_exe()?;
    let command = format!(
        "{} view --file {}",
        shell_quote(exe.to_str().context("binary path encoding")?),
        shell_quote(plan_file.to_str().context("plan path encoding")?)
    );
    let size = format!("{}%", size_percent.clamp(10, 50));
    let cwd = cwd.to_str().context("cwd encoding")?;
    let pane = tmux(&[
        "split-window",
        "-d",
        "-v",
        "-l",
        &size,
        "-t",
        source,
        "-c",
        cwd,
        "-P",
        "-F",
        "#{pane_id}",
        &command,
    ])?;
    Ok(Viewer {
        terminal_id: pane.clone(),
        pane,
        reused: false,
    })
}

pub fn close(viewer: &Viewer) -> Result<bool> {
    if !alive(viewer) {
        return Ok(false);
    }
    tmux(&["kill-pane", "-t", &viewer.pane])?;
    Ok(true)
}
