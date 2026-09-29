use crate::live::{find_session, session_header};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{Read, Seek},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    #[serde(default = "codex_agent")]
    pub agent: String,
    pub pane: String,
    pub terminal: String,
    pub pid: u64,
    pub session: Uuid,
    pub rollout: PathBuf,
    pub cwd: PathBuf,
    #[serde(default)]
    pub client_owned: bool,
}

fn codex_agent() -> String {
    "codex".into()
}

// Redirect to private temp files so a full pipe cannot hang the timeout loop.
pub(crate) fn bounded(mut command: Command, cancel: Option<&AtomicBool>) -> Result<Vec<u8>> {
    bounded_output(&mut command, cancel, false)
}

// Some native CLIs write help to stderr. Use only for explicit --help probes,
// never for errors that can contain configuration or provider credentials.
pub(crate) fn bounded_help(mut command: Command) -> Result<Vec<u8>> {
    bounded_output(&mut command, None, true)
}

fn bounded_output(
    command: &mut Command,
    cancel: Option<&AtomicBool>,
    help: bool,
) -> Result<Vec<u8>> {
    ensure!(
        !cancel.is_some_and(|c| c.load(Ordering::Relaxed)),
        "cancelled"
    );
    let mut output = tempfile::tempfile()?;
    let errors = if help {
        output.try_clone()?
    } else {
        tempfile::tempfile()?
    };
    let mut child = command
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(errors)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline || cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("local command timed out");
        }
        thread::sleep(Duration::from_millis(20));
    };
    ensure!(
        status.success(),
        "local command failed ({status}); source may have disconnected"
    );
    ensure!(
        output.metadata()?.len() <= 4 * 1024 * 1024,
        "command output too large"
    );
    output.rewind()?;
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub fn call(args: &[&str]) -> Result<Value> {
    call_cancellable(args, None)
}

fn call_cancellable(args: &[&str], cancel: Option<&AtomicBool>) -> Result<Value> {
    ensure!(
        std::env::var("HERDR_ENV").as_deref() == Ok("1"),
        "Herdr-managed terminal required"
    );
    let mut command = Command::new("herdr");
    command.args(args);
    let bytes = bounded(command, cancel)?;
    // Herdr 0.9 returns successful pane run/input as empty stdout, not a JSON envelope.
    if bytes.is_empty()
        && matches!(
            args.get(..2),
            Some([
                "pane",
                "run" | "send-text" | "send-keys" | "report-agent-session"
            ])
        )
    {
        return Ok(serde_json::json!({"result":{}}));
    }
    let value: Value = serde_json::from_slice(&bytes)?;
    ensure!(value.get("error").is_none(), "Herdr request failed");
    Ok(value)
}

fn process_info(pane: &str, cancel: Option<&AtomicBool>) -> Result<Value> {
    ensure!(
        !pane.starts_with('-') && pane.contains(":p"),
        "explicit Herdr pane ID required"
    );
    Ok(call_cancellable(&["pane", "process-info", "--pane", pane], cancel)?["result"]["process_info"].clone())
}

pub fn resolve(pane: &str) -> Result<Binding> {
    let info = call(&["agent", "get", pane])?;
    let agent = &info["result"]["agent"];
    let kind = agent["agent"].as_str().context("pane agent missing")?;
    ensure!(
        matches!(kind, "codex" | "claude" | "opencode"),
        "target pane must contain a supported coding agent"
    );
    let process = process_info(pane, None)?;
    let processes = process["foreground_processes"]
        .as_array()
        .context("no foreground processes")?;
    let codex: Vec<_> = processes.iter().filter(|p| p["name"] == kind).collect();
    ensure!(
        codex.len() == 1,
        "cannot uniquely identify the native Codex process"
    );
    let pid = codex[0]["pid"].as_u64().context("Codex pid missing")?;
    if kind != "codex" {
        let cwd = agent["foreground_cwd"]
            .as_str()
            .context("agent project missing")?;
        let registered = crate::bridge::lookup(
            Path::new(cwd),
            pane,
            agent["terminal_id"].as_str().context("terminal missing")?,
            pid,
            kind,
        )?;
        return Ok(Binding {
            client_owned: false,
            agent: kind.into(),
            pane: pane.into(),
            terminal: registered.terminal,
            pid,
            session: registered.session,
            rollout: registered.rollout,
            cwd: registered.cwd,
        });
    }
    let reported_session = agent_session(agent);
    let rollout = match open_rollout(pid, None) {
        Ok(path) => path,
        Err(_) => {
            if let Some(cwd) = agent["foreground_cwd"].as_str()
                && let Some(terminal) = agent["terminal_id"].as_str()
                && let Ok(registered) =
                    crate::bridge::lookup(Path::new(cwd), pane, terminal, pid, "codex")
            {
                let (session, source_cwd) = session_header(&registered.rollout)?;
                ensure!(
                    registered.schema == 3 || reported_session.is_none_or(|id| id == session),
                    "Herdr/native hook identity mismatch"
                );
                ensure!(
                    Path::new(&source_cwd)
                        .canonicalize()?
                        .starts_with(&registered.cwd),
                    "native hook project mismatch"
                );
                return Ok(Binding {
                    client_owned: registered.schema == 3,
                    agent: kind.into(),
                    pane: pane.into(),
                    terminal: registered.terminal,
                    pid,
                    session,
                    rollout: registered.rollout,
                    cwd: source_cwd.into(),
                });
            }
            ensure!(
                agent["agent_session"]["source"] != "herdr:codex-client",
                "native client selection proof is no longer current"
            );
            let session = reported_session.context("Codex session identity unavailable")?;
            let home = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".codex")))
                .context("Codex home unavailable")?;
            find_session(&home, session)?.canonicalize()?
        }
    };
    let (session, cwd) = session_header(&rollout)?;
    if let Some(reported) = reported_session {
        ensure!(reported == session, "Herdr/Codex session identity mismatch");
    }
    if let Some(foreground) = agent["foreground_cwd"].as_str() {
        ensure!(
            Path::new(&cwd).canonicalize()? == Path::new(foreground).canonicalize()?,
            "source project changed; reconnect explicitly"
        );
    }
    Ok(Binding {
        client_owned: false,
        agent: kind.into(),
        pane: pane.into(),
        terminal: agent["terminal_id"]
            .as_str()
            .context("terminal identity missing")?
            .into(),
        pid,
        session,
        rollout,
        cwd: cwd.into(),
    })
}

fn agent_session(agent: &Value) -> Option<Uuid> {
    let identity = &agent["agent_session"];
    (identity["agent"] == "codex"
        && identity["kind"] == "id"
        && matches!(
            identity["source"].as_str(),
            Some("herdr:codex" | "herdr:codex-client")
        ))
    .then(|| identity["value"].as_str()?.parse().ok())
    .flatten()
}

fn open_rollout(pid: u64, cancel: Option<&AtomicBool>) -> Result<PathBuf> {
    let mut command = Command::new("lsof");
    command.args(["-a", "-p", &pid.to_string(), "-Fn"]);
    let bytes = bounded(command, cancel)?;
    let text = String::from_utf8(bytes)?;
    let mut candidates: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .filter(|name| {
            name.ends_with(".jsonl")
                && name
                    .rsplit('/')
                    .next()
                    .is_some_and(|n| n.starts_with("rollout-"))
        })
        .map(PathBuf::from)
        .collect();
    candidates.sort();
    candidates.dedup();
    ensure!(
        candidates.len() == 1,
        "Codex must have one open rollout; use an exact --session for offline reading"
    );
    Ok(candidates[0].canonicalize()?)
}

pub fn check(binding: &Binding) -> Result<String> {
    check_cancellable(binding, None)
}

pub fn check_cancellable(binding: &Binding, cancel: Option<&AtomicBool>) -> Result<String> {
    let info = call_cancellable(&["agent", "get", &binding.pane], cancel)?;
    let agent = &info["result"]["agent"];
    ensure!(
        agent["agent"].as_str() == Some(&binding.agent)
            && agent["terminal_id"].as_str() == Some(&binding.terminal),
        "source terminal changed; reconnect explicitly"
    );
    let process = process_info(&binding.pane, cancel)?;
    ensure!(
        process["foreground_processes"]
            .as_array()
            .is_some_and(
                |ps| ps.iter().any(|p| p["pid"].as_u64() == Some(binding.pid)
                    && p["name"].as_str() == Some(&binding.agent))
            ),
        "source process exited or changed; reconnect explicitly"
    );
    if binding.agent != "codex" {
        let registered = crate::bridge::lookup(
            &binding.cwd,
            &binding.pane,
            &binding.terminal,
            binding.pid,
            &binding.agent,
        )?;
        ensure!(
            registered.session == binding.session && registered.rollout == binding.rollout,
            "source session changed; reconnect explicitly"
        );
    } else {
        match open_rollout(binding.pid, cancel) {
            Ok(path) => ensure!(
                path == binding.rollout,
                "source session changed; reconnect explicitly"
            ),
            Err(_) => {
                let verified_client = crate::bridge::lookup(
                    &binding.cwd,
                    &binding.pane,
                    &binding.terminal,
                    binding.pid,
                    "codex",
                )
                .is_ok_and(|r| {
                    r.schema == 3 && r.session == binding.session && r.rollout == binding.rollout
                });
                ensure!(
                    verified_client
                        || (!binding.client_owned
                            && agent_session(agent) == Some(binding.session)
                            && session_header(&binding.rollout)?.0 == binding.session),
                    "source session changed; reconnect explicitly"
                );
            }
        }
    }
    Ok(match agent["agent_status"].as_str() {
        Some("working") => "작업 중 · Herdr 관측",
        Some("blocked") => "입력 대기 · Herdr 관측",
        Some("idle" | "done") => "응답 대기 · Herdr 관측",
        _ => "활동 알 수 없음 · Herdr",
    }
    .into())
}

pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn open(pane: Option<String>) -> Result<()> {
    open_with(pane, false)
}

pub fn open_with(pane: Option<String>, reconnect: bool) -> Result<()> {
    open_internal(pane, reconnect, false, None)
}

pub fn open_position(
    pane: Option<String>,
    reconnect: bool,
    position: Option<crate::settings::Position>,
) -> Result<()> {
    open_internal(pane, reconnect, false, position)
}

pub fn open_quiet(pane: Option<String>, reconnect: bool) -> Result<()> {
    open_internal(pane, reconnect, true, None)
}

fn open_internal(
    pane: Option<String>,
    reconnect: bool,
    quiet: bool,
    position: Option<crate::settings::Position>,
) -> Result<()> {
    let pane = pane
        .or_else(|| std::env::var("HERDR_PANE_ID").ok())
        .context("source pane required")?;
    let binding = resolve(&pane)?;
    let position = position.unwrap_or(crate::settings::load_for_cwd(&binding.cwd)?.position);
    let source_info = call(&["pane", "get", &pane])?;
    let workspace = source_info["result"]["pane"]["workspace_id"]
        .as_str()
        .context("workspace missing")?;
    let tab = source_info["result"]["pane"]["tab_id"]
        .as_str()
        .context("tab missing")?;
    let tabs = call(&["tab", "list", "--workspace", workspace])?;
    let target = tabs["result"]["tabs"]
        .as_array()
        .context("unknown Herdr tab response; refusing open")?
        .iter()
        .find(|t| t["tab_id"] == tab)
        .context("source tab disappeared")?;
    ensure!(
        target["label"].as_str().context("tab label missing")? != "lobby",
        "protected lobby: use an existing separate terminal"
    );
    // Serialize per source terminal, including an explicit transition to a new session.
    use fs2::FileExt;
    let directory = binding.cwd.join(".agent-progress");
    std::fs::create_dir_all(&directory)?;
    ensure!(
        directory
            .canonicalize()?
            .starts_with(binding.cwd.canonicalize()?),
        "window registry escapes project"
    );
    let receipt = directory.join(format!("window-{}.json", binding.session));
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(format!(
            "window-source-{}.ap-lock",
            crate::recovery::hash(binding.terminal.as_bytes())
        )))?;
    lock.try_lock_exclusive()
        .context("another window open is in progress")?;
    if reconnect && !receipt.exists() {
        let mut candidates = Vec::new();
        for entry in std::fs::read_dir(&directory)? {
            let path = entry?.path();
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !name.starts_with("window-") || !name.ends_with(".json") {
                continue;
            }
            let Ok(bytes) = crate::recovery::read(&path) else {
                continue;
            };
            let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if record["source_pane"] != pane || record["source_terminal"] != binding.terminal {
                continue;
            }
            let Some(child) = record["pane"].as_str() else {
                continue;
            };
            let Ok(live) = call(&["pane", "get", child]) else {
                continue;
            };
            if live["result"]["pane"]["terminal_id"] == record["terminal_id"] {
                candidates.push((path, record));
            }
        }
        // A migrated receipt points to its predecessor. Keep the unique lineage leaf,
        // not an arbitrary newest file, when multiple receipts describe the same terminal.
        let predecessors = candidates
            .iter()
            .filter_map(|(_, r)| r["previous_receipt"].as_str().map(PathBuf::from))
            .collect::<Vec<_>>();
        candidates.retain(|(path, _)| !predecessors.contains(path));
        ensure!(
            candidates.len() <= 1,
            "multiple owned progress windows; inspect before reconnecting"
        );
        if let Some((prior_path, mut record)) = candidates.pop() {
            let child = record["pane"]
                .as_str()
                .context("prior window pane missing")?
                .to_owned();
            let prior_session = record["session"]
                .as_str()
                .context("prior session missing")?;
            let processes = process_info(&child, None)?;
            let foreground = processes["foreground_processes"]
                .as_array()
                .context("window processes missing")?;
            if let Some(process) = foreground.iter().find(|p| p["name"] == "ap") {
                let pid = process["pid"].as_u64().context("window PID missing")?;
                let mut ps = Command::new("ps");
                ps.args(["-p", &pid.to_string(), "-o", "args="]);
                let args = String::from_utf8(bounded(ps, None)?)?;
                ensure!(
                    args.contains(&format!("--session {prior_session}"))
                        && args.contains(&format!("--pane {pane}")),
                    "prior window serves another source; retained"
                );
                call(&["pane", "send-keys", &child, "q"])?;
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    let info = process_info(&child, None)?;
                    if info["foreground_processes"].as_array().is_some_and(|ps| {
                        ps.iter().all(|p| {
                            matches!(p["name"].as_str(), Some("zsh" | "bash" | "sh" | "fish"))
                        })
                    }) {
                        break;
                    }
                    ensure!(
                        Instant::now() < deadline,
                        "prior progress window did not exit; retained"
                    );
                    thread::sleep(Duration::from_millis(30));
                }
            } else {
                ensure!(
                    foreground.iter().all(|p| matches!(
                        p["name"].as_str(),
                        Some("zsh" | "bash" | "sh" | "fish")
                    )),
                    "prior window is busy; retained"
                );
            }
            check(&binding)?;
            record["session"] = serde_json::json!(binding.session);
            record["previous_receipt"] = serde_json::json!(prior_path);
            crate::recovery::write(&receipt, &serde_json::to_vec_pretty(&record)?, false)?;
            // Prior receipt is immutable history. The following ordinary shell-reuse path relaunches the same pane.
        }
    }
    if reconnect && receipt.exists() {
        let record: Value = serde_json::from_slice(&crate::recovery::read(&receipt)?)?;
        ensure!(
            record["source_pane"] == pane && record["source_terminal"] == binding.terminal,
            "window receipt source changed"
        );
        let inventory = call(&["pane", "list"])?;
        let panes = inventory["result"]["panes"]
            .as_array()
            .context("cannot verify stale pane; retained")?;
        if !panes
            .iter()
            .any(|p| p["terminal_id"] == record["terminal_id"])
        {
            // Exact owned receipt only. Existing/moved terminals are always retained.
            std::fs::remove_file(&receipt)?;
            std::fs::File::open(&directory)?.sync_all()?;
        }
    }
    if receipt.exists() {
        let mut record: Value = serde_json::from_slice(&crate::recovery::read(&receipt)?)?;
        ensure!(
            record["source_pane"] == pane && record["session"] == binding.session.to_string(),
            "window receipt identity mismatch"
        );
        let child_id = record["pane"]
            .as_str()
            .context("window receipt pane missing")?
            .to_owned();
        let child = child_id.as_str();
        // A disconnected registry is diagnosed instead of guessing another pane or splitting.
        let live=call(&["pane","get",child]).context("recorded progress pane unavailable; use doctor then open --reconnect to verify and recover a closed pane")?;
        ensure!(
            live["result"]["pane"]["terminal_id"] == record["terminal_id"],
            "recorded progress terminal changed"
        );
        let processes = process_info(child, None)?;
        for process in processes["foreground_processes"]
            .as_array()
            .context("window processes unavailable")?
        {
            if process["name"] == "ap" {
                let pid = process["pid"].as_u64().context("window pid missing")?;
                let mut ps = Command::new("ps");
                ps.args(["-p", &pid.to_string(), "-o", "args="]);
                let args = String::from_utf8(bounded(ps, None)?)?;
                ensure!(
                    args.contains(&format!("--session {}", binding.session))
                        && args.contains(&format!("--pane {pane}")),
                    "progress pane now serves another source"
                );
                let recorded: crate::settings::Position = serde_json::from_value(
                    record
                        .get("position")
                        .cloned()
                        .unwrap_or(serde_json::json!("below")),
                )?;
                if recorded != position {
                    reposition(&pane, child, position)?;
                    record["position"] = serde_json::json!(position);
                    crate::recovery::write(&receipt, &serde_json::to_vec_pretty(&record)?, true)?;
                }
                if !quiet {
                    println!("기존 진행 상황 창: {child} ← {pane} · {}", binding.session);
                }
                return Ok(());
            }
        }
        // An owned, available shell can be reused after the ap process exited.
        ensure!(
            processes["foreground_processes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| matches!(p["name"].as_str(), Some("zsh" | "bash" | "sh" | "fish"))),
            "recorded pane is busy"
        );
        let recorded: crate::settings::Position = serde_json::from_value(
            record
                .get("position")
                .cloned()
                .unwrap_or(serde_json::json!("below")),
        )?;
        if recorded != position {
            reposition(&pane, child, position)?;
            record["position"] = serde_json::json!(position);
            crate::recovery::write(&receipt, &serde_json::to_vec_pretty(&record)?, true)?;
        }
        let command = format!(
            "{} follow --pane {} --session {}",
            shell_quote(
                std::env::current_exe()?
                    .canonicalize()?
                    .to_str()
                    .context("binary path encoding")?
            ),
            shell_quote(&pane),
            binding.session
        );
        call(&["pane", "run", child, &command])?;
        if !quiet {
            println!(
                "진행 상황 창 재연결: {child} ← {pane} · {}",
                binding.session
            );
        }
        return Ok(());
    }
    let executable = std::env::current_exe()?.canonicalize()?;
    let info = call(&["pane", "get", &pane])?;
    let workspace = info["result"]["pane"]["workspace_id"]
        .as_str()
        .context("workspace missing")?;
    let tab = info["result"]["pane"]["tab_id"]
        .as_str()
        .context("tab missing")?;
    let tabs = call(&["tab", "list", "--workspace", workspace])?;
    // Never mutate protected home layout or guess where a missing pane lives.
    let list = tabs["result"]["tabs"]
        .as_array()
        .context("unknown Herdr tab response; refusing split")?;
    let target = list
        .iter()
        .find(|t| t["tab_id"].as_str() == Some(tab))
        .context("source tab disappeared; refusing split")?;
    ensure!(
        target["label"].as_str().context("tab label missing")? != "lobby",
        "protected lobby: run follow in an existing separate terminal"
    );
    check(&binding)?;
    let result = call(&[
        "pane",
        "split",
        "--pane",
        &pane,
        "--direction",
        "down",
        "--ratio",
        "0.70",
        "--cwd",
        binding.cwd.to_str().context("cwd encoding")?,
        "--no-focus",
    ])?;
    let child = result["result"]["pane"]["pane_id"]
        .as_str()
        .context("new pane missing")?;
    let info = call(&["pane", "get", child])?;
    let terminal = info["result"]["pane"]["terminal_id"]
        .as_str()
        .context("child terminal identity missing")?;
    // Herdr 0.9 only exposes down/right splits. Build the owned lower sibling,
    // then exchange it with the source while restoring the original sizes.
    crate::recovery::write(
        &receipt,
        &serde_json::to_vec(
            &serde_json::json!({"source_pane":pane,"source_terminal":binding.terminal,"session":binding.session,"pane":child,"terminal_id":terminal,"position":"below"}),
        )?,
        false,
    )?;
    if position == crate::settings::Position::Above {
        reposition(&pane, child, position)?;
        let mut record: Value = serde_json::from_slice(&crate::recovery::read(&receipt)?)?;
        record["position"] = serde_json::json!(position);
        crate::recovery::write(&receipt, &serde_json::to_vec_pretty(&record)?, true)?;
    }
    let command = format!(
        "{} follow --pane {} --session {}",
        shell_quote(executable.to_str().context("binary path encoding")?),
        shell_quote(&pane),
        binding.session
    );
    if let Err(error) = call(&["pane", "run", child, &command]) {
        anyhow::bail!("created pane {child}, launch failed: {error}; pane retained");
    }
    if !quiet {
        println!(
            "진행 상황 창: {child} ← {} · {}",
            binding.pane, binding.session
        );
    }
    Ok(())
}

fn reposition(source: &str, child: &str, position: crate::settings::Position) -> Result<()> {
    let value = call(&["pane", "layout", "--pane", source])?;
    let layout = &value["result"]["layout"];
    let panes = layout["panes"]
        .as_array()
        .context("pane geometry unavailable")?;
    let rect = |id: &str| {
        panes
            .iter()
            .find(|p| p["pane_id"] == id)
            .map(|p| &p["rect"])
            .context("pane disappeared")
    };
    let a = rect(source)?;
    let b = rect(child)?;
    let number = |rect: &Value, key: &str| rect[key].as_u64().context("invalid pane geometry");
    let ax = number(a, "x")?;
    let ay = number(a, "y")?;
    let aw = number(a, "width")?;
    let ah = number(a, "height")?;
    let bx = number(b, "x")?;
    let by = number(b, "y")?;
    let bw = number(b, "width")?;
    let bh = number(b, "height")?;
    ensure!(
        ax == bx && aw == bw && (ay + ah == by || by + bh == ay),
        "owned observer is not a vertical sibling; preserve layout and open an explicit target"
    );
    let already = match position {
        crate::settings::Position::Above => by < ay,
        crate::settings::Position::Below => by > ay,
    };
    if already {
        return Ok(());
    }
    let split = layout["splits"]
        .as_array()
        .context("split geometry missing")?
        .iter()
        .find(|s| {
            s["direction"] == "down"
                && s["rect"]["x"] == ax
                && s["rect"]["y"] == ay.min(by)
                && s["rect"]["width"] == aw
                && s["rect"]["height"] == ah + bh
        })
        .context("observer/source are not direct split siblings; layout preserved")?;
    let ratio = split["ratio"].as_f64().context("split ratio missing")?;
    let focused = layout["focused_pane_id"].clone();
    call(&[
        "pane",
        "swap",
        "--source-pane",
        source,
        "--target-pane",
        child,
    ])?;
    let amount = match position {
        crate::settings::Position::Above => 2.0 * ratio - 1.0,
        crate::settings::Position::Below => 1.0 - 2.0 * ratio,
    };
    let mut remaining = amount;
    while remaining.abs() > 0.0001 {
        // Herdr clamps each resize to 0.5. A 90/10 split needs 0.8 after
        // exchange, so one request leaves the observer at the wrong size.
        let step = remaining.clamp(-0.5, 0.5);
        let direction = match position {
            crate::settings::Position::Above => "up",
            crate::settings::Position::Below => "down",
        };
        call(&[
            "pane",
            "resize",
            "--pane",
            source,
            "--direction",
            direction,
            "--amount",
            &step.to_string(),
        ])?;
        remaining -= step;
    }
    let after = call(&["pane", "layout", "--pane", source])?;
    ensure!(
        after["result"]["layout"]["focused_pane_id"] == focused,
        "focus changed during reposition; inspect exact layout"
    );
    let final_panes = after["result"]["layout"]["panes"]
        .as_array()
        .context("final pane geometry missing")?;
    let final_height = |id: &str| {
        final_panes
            .iter()
            .find(|p| p["pane_id"] == id)
            .and_then(|p| p["rect"]["height"].as_u64())
            .context("final pane missing")
    };
    ensure!(
        final_height(source)?.abs_diff(ah) <= 1 && final_height(child)?.abs_diff(bh) <= 1,
        "pane sizes were not restored after exchange"
    );
    Ok(())
}
