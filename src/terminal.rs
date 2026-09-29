//! Optional tmux display. Every launch owns a private slot; never pick a newest session.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub agent: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct Selection {
    pub session: uuid::Uuid,
    pub rollout: PathBuf,
    pub cwd: PathBuf,
    pub owner: u32,
}

pub fn interactive_args(agent: &str, args: &[String]) -> bool {
    if args.iter().any(|a| {
        matches!(
            a.as_str(),
            "--help" | "-h" | "--version" | "-V" | "--print" | "-p"
        )
    }) && agent != "codex"
    {
        return false;
    }
    let mut skip = false;
    for arg in args {
        if skip {
            skip = false;
            continue;
        }
        match arg.as_str() {
            "--help" | "-h" | "--version" | "-V" => return false,
            "--" => break,
            "-c"
            | "--config"
            | "-m"
            | "--model"
            | "-p"
            | "--profile"
            | "-C"
            | "--cd"
            | "-i"
            | "--image"
            | "-a"
            | "--ask-for-approval"
            | "-s"
            | "--sandbox"
            | "--enable"
            | "--disable"
            | "--remote"
            | "--remote-auth-token-env" => skip = true,
            value if value.starts_with('-') => {}
            value => {
                return !matches!(
                    value,
                    "exec"
                        | "e"
                        | "login"
                        | "logout"
                        | "mcp"
                        | "review"
                        | "app-server"
                        | "exec-server"
                        | "doctor"
                        | "update"
                        | "features"
                        | "completion"
                        | "sandbox"
                        | "queue"
                        | "archive"
                        | "unarchive"
                        | "delete"
                        | "migrate-rollouts"
                        | "cloud"
                        | "remote-control"
                        | "app"
                        | "run"
                        | "serve"
                        | "web"
                        | "auth"
                        | "models"
                        | "export"
                        | "import"
                        | "upgrade"
                );
            }
        }
    }
    true
}

pub fn slot() -> Option<PathBuf> {
    std::env::var_os("AP_TERMINAL_SLOT").map(PathBuf::from)
}

pub fn validate_slot(path: &Path) -> Result<Request> {
    ensure!(
        path.is_absolute()
            && !path.is_symlink()
            && path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("ap-terminal-")),
        "invalid terminal slot"
    );
    serde_json::from_slice(&crate::recovery::read(&path.join("request.json"))?).map_err(Into::into)
}

fn tmux(socket: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut command = Command::new("tmux");
    if let Some(socket) = socket {
        command.arg("-S").arg(socket).args(["-f", "/dev/null"]);
    }
    command.args(args);
    Ok(String::from_utf8(crate::herdr::bounded(command, None)?)?
        .trim()
        .to_owned())
}

pub fn maybe_launch(agent: &str, args: &[String]) -> Result<bool> {
    if std::env::var("HERDR_ENV").as_deref() == Ok("1")
        || slot().is_some()
        || !interactive_args(agent, args)
        || (agent == "codex" && !crate::codex_client::can_observe(args))
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        return Ok(false);
    }
    let cwd = std::env::current_dir()?;
    let settings = crate::settings::load_for_cwd(&cwd).unwrap_or_else(|_| {
        eprintln!("ap: invalid presentation settings; using default display settings");
        Default::default()
    });
    if !settings.auto_open || std::env::var("AP_AUTO_OPEN").as_deref() == Ok("0") {
        return Ok(false);
    }
    if tmux(None, &["-V"]).is_err() {
        eprintln!(
            "ap: automatic progress outside Herdr needs tmux; continuing native agent (brew install tmux)"
        );
        return Ok(false);
    }
    let directory = tempfile::Builder::new()
        .prefix("ap-terminal-")
        .tempdir_in("/tmp")?;
    let path = directory.path();
    let request = Request {
        agent: agent.into(),
        args: args.into(),
        cwd: cwd.canonicalize()?,
    };
    crate::recovery::write(
        &path.join("request.json"),
        &serde_json::to_vec(&request)?,
        false,
    )?;
    let executable = crate::herdr::shell_quote(
        std::env::current_exe()?
            .to_str()
            .context("binary path encoding")?,
    );
    let quoted = crate::herdr::shell_quote(path.to_str().context("slot encoding")?);
    let source_command = format!("exec {executable} terminal-source --slot {quoted}");
    let observer_command = format!("exec {executable} terminal-follow --slot {quoted}");
    let existing = std::env::var_os("TMUX").is_some();
    let (columns, rows) = crossterm::terminal::size().unwrap_or((120, 40));
    let columns = columns.to_string();
    // tmux's private default status line consumes one terminal row after attach.
    let rows = rows.saturating_sub(1).max(4).to_string();
    let socket = (!existing).then(|| path.join("tmux.sock"));
    let source = if existing {
        // One new owned window avoids modifying another task's split geometry.
        tmux(
            None,
            &[
                "new-window",
                "-P",
                "-F",
                "#{pane_id}",
                "-n",
                "agent-progress",
                "-c",
                request.cwd.to_str().context("cwd encoding")?,
                &source_command,
            ],
        )?
    } else {
        tmux(
            socket.as_deref(),
            &[
                "new-session",
                "-d",
                "-P",
                "-F",
                "#{pane_id}",
                "-s",
                "ap",
                "-x",
                &columns,
                "-y",
                &rows,
                "-c",
                request.cwd.to_str().context("cwd encoding")?,
                &source_command,
            ],
        )?
    };
    let height: usize = tmux(
        socket.as_deref(),
        &["display-message", "-p", "-t", &source, "#{window_height}"],
    )?
    .parse()?;
    let progress_rows = (height / 3).clamp(1, 8).to_string();
    let mut split = vec![
        "split-window",
        "-d",
        "-v",
        "-l",
        &progress_rows,
        "-t",
        source.as_str(),
    ];
    let position = settings.position;
    if position == crate::settings::Position::Above {
        split.push("-b");
    }
    split.push(&observer_command);
    if let Err(error) = tmux(socket.as_deref(), &split) {
        // Close only this newly-created window, never an unrelated session/server.
        let _ = tmux(socket.as_deref(), &["kill-window", "-t", &source]);
        return Err(error.context("progress split failed"));
    }
    let _ = tmux(socket.as_deref(), &["select-pane", "-t", &source]);
    crate::recovery::write(&path.join("layout-ready"), b"ready", false)?;
    let kept = directory.keep();
    if existing {
        // The caller waits as it would for the native foreground command, while tmux
        // presents the new window. Do not return success before the native process exits.
        while !kept.join("exit-status").exists() {
            ensure!(
                tmux(
                    None,
                    &["display-message", "-p", "-t", &source, "#{pane_id}"]
                )
                .is_ok()
                    || kept.join("exit-status").exists(),
                "terminal source disappeared before recording its exit status"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        let code: i32 = fs::read_to_string(kept.join("exit-status"))?.parse()?;
        // Let the observer restore its terminal before removing its request metadata.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while kept.join("request.json").exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        cleanup(&kept, true);
        if code != 0 {
            std::process::exit(code);
        }
        return Ok(true);
    }
    if !existing {
        let status = Command::new("tmux")
            .arg("-S")
            .arg(kept.join("tmux.sock"))
            .args(["attach-session", "-t", "ap"])
            .status()?;
        ensure!(
            status.success(),
            "tmux attach failed; reconnect with tmux -S {} attach",
            kept.join("tmux.sock").display()
        );
        // Detach preserves the live session; finished sessions release only owned metadata.
        if tmux(Some(&kept.join("tmux.sock")), &["has-session", "-t", "ap"]).is_err() {
            let code = fs::read_to_string(kept.join("exit-status"))
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            cleanup(&kept, true);
            if code != 0 {
                std::process::exit(code);
            }
        }
    }
    Ok(true)
}

fn cleanup(path: &Path, socket: bool) {
    for name in ["request.json", "owner", "selection.json", "layout-ready"] {
        let _ = fs::remove_file(path.join(name));
    }
    if socket {
        let _ = fs::remove_file(path.join("tmux.sock"));
        let _ = fs::remove_file(path.join("exit-status"));
    }
    let _ = fs::remove_dir(path);
}

pub fn source(path: &Path) -> Result<()> {
    let request = validate_slot(path)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !path.join("layout-ready").exists() {
        ensure!(
            std::time::Instant::now() < deadline,
            "terminal layout did not become ready"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    crate::recovery::write(
        &path.join("owner"),
        std::process::id().to_string().as_bytes(),
        false,
    )?;
    // Wait like a normal shell wrapper, retaining the native exit status for the caller.
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("launch")
        .args(["--agent", &request.agent, "--"])
        .args(request.args)
        .current_dir(request.cwd)
        .env("AP_TERMINAL_SLOT", path)
        .env("AP_TERMINAL_PARENT", std::process::id().to_string())
        .env("HERDR_ENV", "0");
    let status = command.status()?;
    crate::recovery::write(
        &path.join("exit-status"),
        status.code().unwrap_or(130).to_string().as_bytes(),
        false,
    )?;
    Ok(())
}

pub fn claim() -> Result<()> {
    if let Some(slot) = slot()
        && let Ok(parent) = std::env::var("AP_TERMINAL_PARENT")
    {
        validate_slot(&slot)?;
        let parent: u32 = parent.parse()?;
        ensure!(
            fs::read_to_string(slot.join("owner"))? == parent.to_string()
                && crate::bridge::process_is_descendant(std::process::id().into(), parent.into())?,
            "terminal launcher ownership mismatch"
        );
        crate::recovery::write(
            &slot.join("owner"),
            std::process::id().to_string().as_bytes(),
            true,
        )?;
    }
    Ok(())
}

pub fn publish(path: &Path, selection: Selection) -> Result<()> {
    validate_slot(path)?;
    let owner: u32 = String::from_utf8(crate::recovery::read(&path.join("owner"))?)?.parse()?;
    ensure!(
        owner == selection.owner
            && crate::bridge::process_is_descendant(std::process::id().into(), owner.into())?,
        "terminal selection is not owned by this frontend"
    );
    let (session, _) = crate::live::session_header(&selection.rollout)?;
    ensure!(
        session == selection.session,
        "terminal transcript identity mismatch"
    );
    crate::recovery::write(
        &path.join("selection.json"),
        &serde_json::to_vec(&selection)?,
        true,
    )
}

pub fn follow(path: &Path, once: bool) -> Result<()> {
    validate_slot(path)?;
    if once {
        let selection: Selection =
            serde_json::from_slice(&crate::recovery::read(&path.join("selection.json"))?)?;
        return crate::dashboard::follow(crate::dashboard::Options {
            pane: None,
            session: Some(selection.session),
            rollout: Some(selection.rollout),
            home: None,
            once: true,
            cache: None,
            project: None,
        });
    }
    use crossterm::event::{self, Event};
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            ratatui::restore();
        }
    }
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    let mut snapshot = crate::live::Snapshot::empty(uuid::Uuid::nil(), "진행 연결 대기".into());
    let mut view = crate::dashboard::View::default();
    let mut feed: Option<crate::live::Feed> = None;
    let mut selected = None;
    loop {
        if let Ok(bytes) = crate::recovery::read(&path.join("selection.json")) {
            let result = (|| -> Result<()> {
                let selection: Selection = serde_json::from_slice(&bytes)?;
                if selected != Some(selection.session) {
                    feed = Some(crate::live::Feed::open(
                        selection.rollout,
                        Some(selection.session),
                        None,
                    )?);
                    selected = Some(selection.session);
                }
                let feed = feed.as_mut().context("feed missing")?;
                feed.refresh()?;
                snapshot = feed.snapshot.clone();
                if let Some(project) = crate::project::Project::discover(&selection.cwd)? {
                    snapshot = project.project(&snapshot)?;
                }
                crate::dashboard::refresh_palette(&selection.cwd, &mut view);
                view.error = None;
                Ok(())
            })();
            if result.is_err() {
                view.error = Some("세션 읽기 실패 · 마지막 정상 계획 보존".into());
            }
        }
        terminal.draw(|f| crate::dashboard::render(f, &snapshot, &mut view))?;
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && view.key(key)
        {
            break;
        }
        if let Ok(owner) = fs::read_to_string(path.join("owner")) {
            let mut command = Command::new("kill");
            command.args(["-0", owner.trim()]);
            if crate::herdr::bounded(command, None).is_err() {
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while !path.join("exit-status").exists() && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(20));
                }
                cleanup(path, false);
                break;
            }
        }
    }
    Ok(())
}
