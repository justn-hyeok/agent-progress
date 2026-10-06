use crate::store::Viewer;
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

/// Split below `source` running the viewer; the pane closes when the viewer exits.
pub fn open_below(
    source: &str,
    cwd: &Path,
    command: &str,
    instance: &str,
    size_percent: u8,
) -> Result<Viewer> {
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
        command,
    ])?;
    Ok(Viewer {
        instance: instance.into(),
        pane,
        reused: false,
    })
}
