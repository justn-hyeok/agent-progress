//! Bounded native lifecycle adapters. Store only identity and explicit plans, never raw conversations.
use crate::{herdr, live, project::Project, recovery};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    #[serde(default)]
    pub schema: u32,
    pub agent: String,
    pub native_session: String,
    pub session: Uuid,
    pub rollout: PathBuf,
    pub cwd: PathBuf,
    pub pane: String,
    pub terminal: String,
    pub pid: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_owner: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_marker: Option<PathBuf>,
}

pub fn read_input(mut input: impl Read) -> Result<Value> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "hook input exceeds 1 MiB");
    Ok(serde_json::from_slice(&bytes)?)
}

fn directory(root: &Path) -> Result<PathBuf> {
    let storage = root.join(".agent-progress");
    ensure!(!storage.is_symlink(), "refusing symlink bridge storage");
    fs::create_dir_all(&storage)?;
    let dir = storage.join("bridges");
    ensure!(!dir.is_symlink(), "refusing symlink bridge directory");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn safe_text(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(4096)
        .collect()
}

fn markdown_plan(message: &str) -> String {
    // Do not copy surrounding assistant prose, tool output, or credentials into a checkpoint.
    let Some((_, steps)) = live::markdown_plan(message) else {
        return String::new();
    };
    let rows = steps
        .into_iter()
        .map(|(title, status)| {
            let mark = match status {
                live::StepState::Pending => " ",
                live::StepState::Active => ">",
                live::StepState::Done => "x",
            };
            format!("- [{mark}] {}", safe_text(&title))
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("### 진행 계획\n{rows}")
}

pub fn ingest(agent: &str, event: &Value, selected_root: Option<&Path>) -> Result<Value> {
    ensure!(
        matches!(agent, "codex" | "claude" | "opencode"),
        "unsupported agent"
    );
    let cwd = PathBuf::from(event["cwd"].as_str().context("hook cwd required")?).canonicalize()?;
    let project = if let Some(root) = selected_root {
        Project::open(&root.join("ap.project.json"))?
    } else {
        Project::discover(&cwd)?.context("no project declaration for hook")?
    };
    let root = project.root().canonicalize()?;
    ensure!(
        cwd.starts_with(&root),
        "hook belongs to a different project"
    );
    let native = event["session_id"]
        .as_str()
        .context("native session ID required")?;
    ensure!(
        !native.is_empty() && native.len() <= 200 && !native.chars().any(char::is_control),
        "invalid native session ID"
    );
    let dir = directory(&root)?;
    let session = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("agent-progress:{agent}:{native}").as_bytes(),
    );
    let lock_path = dir.join(format!("{agent}-{session}.lock"));
    ensure!(!lock_path.is_symlink(), "refusing symlink hook lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        if lock.try_lock_exclusive().is_ok() {
            break;
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "hook session busy; retry event"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if agent == "codex" {
        let status_path = dir.join(format!("codex-{session}.status.json"));
        let Some(transcript) = event["transcript_path"].as_str() else {
            let pending = json!({"agent":agent,"event":event["hook_event_name"],"registered_pane":false,"pending":"native transcript not created yet; next event retries"});
            recovery::write(&status_path, &serde_json::to_vec(&pending)?, true)?;
            return Ok(pending);
        };
        if !Path::new(transcript).exists() || fs::metadata(transcript)?.len() == 0 {
            let pending = json!({"agent":agent,"event":event["hook_event_name"],"registered_pane":false,"pending":"native transcript creation pending; next event retries"});
            recovery::write(&status_path, &serde_json::to_vec(&pending)?, true)?;
            return Ok(pending);
        }
        let rollout = PathBuf::from(transcript).canonicalize()?;
        // Hook session_id is not necessarily the thread ID. Read the exact supplied transcript header.
        let (thread, source_cwd) = live::session_header(&rollout)?;
        ensure!(
            Path::new(&source_cwd).canonicalize()?.starts_with(&root),
            "Codex transcript project mismatch"
        );
        let result = register(agent, native, thread, &rollout, &root, &dir);
        let status = match &result {
            Ok(v) => json!({"event":event["hook_event_name"],"result":v}),
            Err(error) => json!({"event":event["hook_event_name"],"error":error.to_string()}),
        };
        recovery::write(&status_path, &serde_json::to_vec(&status)?, true)?;
        return result;
    }
    let rollout = dir.join(format!("{agent}-{session}.jsonl"));
    ensure!(!rollout.is_symlink(), "refusing symlink hook stream");
    if !rollout.exists() {
        recovery::write(&rollout, format!("{}\n", json!({"type":"session_meta","payload":{"id":session,"cwd":root,"agent":agent,"native_session":native}})).as_bytes(), false)?;
    }
    ensure!(
        live::session_header(&rollout)?.0 == session,
        "hook stream identity mismatch"
    );
    let timestamp = chrono::DateTime::from_timestamp(crate::model::now() as i64, 0)
        .context("invalid clock")?
        .to_rfc3339();
    let mut payload = None;
    if let Some(todos) = event
        .get("todos")
        .and_then(Value::as_array)
        .or_else(|| event["tool_input"]["todos"].as_array())
    {
        ensure!(todos.len() <= 2000, "too many native tasks");
        let steps = todos
            .iter()
            .map(|todo| {
                let content = todo["content"]
                    .as_str()
                    .context("native todo content missing")?;
                let status = match todo["status"].as_str().unwrap_or("pending") {
                    "completed" => "completed",
                    "in_progress" => "in_progress",
                    "pending" => "pending",
                    other => anyhow::bail!("unsupported native todo status: {other}"),
                };
                Ok(json!({"step":safe_text(content),"status":status}))
            })
            .collect::<Result<Vec<_>>>()?;
        payload = Some(
            json!({"type":"plan_update","plan":steps,"adapter_source":format!("{agent} native todos")}),
        );
    } else if agent == "claude"
        && matches!(
            event["tool_name"].as_str(),
            Some("TaskCreate" | "TaskUpdate")
        )
    {
        let task_path = dir.join(format!("claude-{session}.tasks.json"));
        let mut tasks: BTreeMap<String, Value> = if task_path.exists() {
            serde_json::from_slice(&recovery::read(&task_path)?)?
        } else {
            BTreeMap::new()
        };
        let input = &event["tool_input"];
        let response = &event["tool_response"];
        let id = input["taskId"]
            .as_str()
            .or_else(|| response["task"]["id"].as_str())
            .or_else(|| response["id"].as_str())
            .context("native task ID missing")?;
        ensure!(
            id.len() <= 100 && id.chars().all(|c| c.is_alphanumeric() || c == '-'),
            "invalid native task ID"
        );
        let old = tasks.get(id);
        let subject = input["subject"]
            .as_str()
            .or_else(|| response["task"]["subject"].as_str())
            .or_else(|| old.and_then(|t| t["subject"].as_str()))
            .context("native task subject missing")?;
        let title = if subject.trim_start().starts_with('[') {
            safe_text(subject)
        } else {
            format!("[CC-{id}] {}", safe_text(subject))
        };
        let status = input["status"]
            .as_str()
            .or_else(|| old.and_then(|t| t["status"].as_str()))
            .unwrap_or("pending");
        ensure!(
            matches!(status, "pending" | "in_progress" | "completed"),
            "unsupported native task status"
        );
        tasks.insert(
            id.into(),
            json!({"subject":subject,"title":title,"status":status}),
        );
        let plan = tasks
            .values()
            .map(|t| json!({"step":t["title"],"status":t["status"]}))
            .collect::<Vec<_>>();
        payload =
            Some(json!({"type":"plan_update","plan":plan,"adapter_source":"claude native tasks"}));
        recovery::write(&task_path, &serde_json::to_vec(&tasks)?, true)?;
    } else if let Some(message) = event["last_assistant_message"]
        .as_str()
        .or_else(|| event["text"].as_str())
    {
        let text = markdown_plan(message);
        if !text.is_empty() {
            payload = Some(
                json!({"type":"item_completed","item":{"type":"AgentMessage","content":[{"type":"text","text":text}]},"adapter_source":format!("{agent} explicit progress report")}),
            );
        }
    }
    if let Some(payload) = payload {
        let record = json!({"type":"event_msg","timestamp":timestamp,"payload":payload});
        // Same normalized plan twice does not grow the stream or fabricate extra history.
        let last_path = dir.join(format!("{agent}-{session}.last.json"));
        let bytes = serde_json::to_vec(&record["payload"])?;
        if !last_path.exists() || recovery::read(&last_path)? != bytes {
            // A killed previous writer may have left only an uncommitted final line.
            // Preserve every complete event; remove only the partial tail before appending.
            use std::io::{Seek, SeekFrom};
            let mut file = OpenOptions::new().read(true).append(true).open(&rollout)?;
            let length = file.metadata()?.len();
            let start = length.saturating_sub(1024 * 1024 + 1);
            file.seek(SeekFrom::Start(start))?;
            let mut tail = Vec::new();
            file.read_to_end(&mut tail)?;
            if !tail.ends_with(b"\n") {
                let complete = tail
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .context("hook stream has an oversized partial event")?;
                file.set_len(start + complete as u64 + 1)?;
            }
            writeln!(file, "{record}")?;
            file.sync_all()?;
            recovery::write(&last_path, &bytes, true)?;
        }
    }
    let mut feed = live::Feed::open(rollout.clone(), Some(session), None)?;
    feed.refresh_all()?;
    project.project(&feed.snapshot)?;
    register(agent, native, session, &rollout, &root, &dir)
}

fn register(
    agent: &str,
    native: &str,
    session: Uuid,
    rollout: &Path,
    root: &Path,
    dir: &Path,
) -> Result<Value> {
    let mut result = json!({"agent":agent,"session":session,"native_session":native,"rollout":rollout,"registered_pane":false});
    if let Some(slot) = crate::terminal::slot() {
        let owner: u32 = String::from_utf8(recovery::read(&slot.join("owner"))?)?.parse()?;
        crate::terminal::publish(
            &slot,
            crate::terminal::Selection {
                session,
                rollout: rollout.into(),
                cwd: root.into(),
                owner,
            },
        )?;
        return Ok(result);
    }
    if std::env::var("HERDR_ENV").as_deref() != Ok("1") {
        return Ok(result);
    }
    let Ok(pane) = std::env::var("HERDR_PANE_ID") else {
        return Ok(result);
    };
    let info =
        herdr::call(&["agent", "get", &pane]).or_else(|_| herdr::call(&["pane", "get", &pane]))?;
    let actual = if info["result"]["agent"].is_object() {
        &info["result"]["agent"]
    } else {
        &info["result"]["pane"]
    };
    ensure!(
        actual["agent"].as_str().is_none_or(|kind| kind == agent),
        "hook pane agent mismatch; no registration written"
    );
    let terminal = actual["terminal_id"]
        .as_str()
        .context("hook terminal missing")?;
    ensure!(
        Path::new(
            actual["foreground_cwd"]
                .as_str()
                .context("hook pane cwd missing")?
        )
        .canonicalize()?
        .starts_with(root),
        "hook pane project mismatch"
    );
    let process = herdr::call(&["pane", "process-info", "--pane", &pane])?;
    let candidates = process["result"]["process_info"]["foreground_processes"]
        .as_array()
        .context("hook source process missing")?;
    let matching = candidates
        .iter()
        .filter(|p| p["name"] == agent)
        .collect::<Vec<_>>();
    ensure!(
        matching.len() == 1,
        "hook cannot identify exact agent process"
    );
    let pid = matching[0]["pid"].as_u64().context("source PID missing")?;
    // A shared daemon inherits its first client's pane environment. Environment alone
    // must never associate another thread with that pane. Prove the native process
    // is an ancestor of this hook before reporting its session identity.
    ensure!(
        sender_is_descendant(pid)?,
        "hook is not owned by this pane process; use Codex embedded mode (--no-daemon) for native pane hooks"
    );
    let registration = Registration {
        schema: 2,
        agent: agent.into(),
        native_session: native.into(),
        session,
        rollout: rollout.into(),
        cwd: root.into(),
        pane: pane.clone(),
        terminal: terminal.into(),
        pid,
        client_owner: None,
        client_marker: None,
    };
    recovery::write(
        &dir.join(format!("pane-{}.json", recovery::hash(pane.as_bytes()))),
        &serde_json::to_vec(&registration)?,
        true,
    )?;
    if agent == "codex" {
        herdr::call(&[
            "pane",
            "report-agent-session",
            &pane,
            "--source",
            "herdr:codex",
            "--agent",
            "codex",
            "--agent-session-id",
            &session.to_string(),
        ])?;
    }
    result["registered_pane"] = json!(true);
    Ok(result)
}

fn sender_is_descendant(expected: u64) -> Result<bool> {
    process_is_descendant(std::process::id().into(), expected)
}

pub(crate) fn process_is_descendant(mut current: u64, expected: u64) -> Result<bool> {
    for _ in 0..12 {
        if current == expected {
            return Ok(true);
        }
        if current <= 1 {
            return Ok(false);
        }
        let mut command = std::process::Command::new("ps");
        command.args(["-p", &current.to_string(), "-o", "ppid="]);
        let bytes = herdr::bounded(command, None)?;
        current = std::str::from_utf8(&bytes)?
            .trim()
            .parse()
            .context("hook parent PID unavailable")?;
    }
    Ok(false)
}

/// Called only after a reply on the launching frontend's private RPC connection.
pub fn register_client(
    pane: &str,
    owner: u32,
    session: Uuid,
    rollout: &Path,
    cwd: &Path,
    launch_root: &Path,
    marker: &Path,
) -> Result<()> {
    let rollout = rollout.canonicalize()?;
    let (actual_session, actual_cwd) = live::session_header(&rollout)?;
    ensure!(
        actual_session == session,
        "native client response/transcript identity mismatch"
    );
    // A resumed thread may override its execution cwd; the legacy header records
    // its creation project. Keep its existing product declaration and history.
    let original_cwd = Path::new(&actual_cwd).canonicalize()?;
    let info = herdr::call(&["agent", "get", pane])?;
    let agent = &info["result"]["agent"];
    ensure!(agent["agent"] == "codex", "source no longer contains Codex");
    let terminal = agent["terminal_id"]
        .as_str()
        .context("source terminal missing")?;
    let process = herdr::call(&["pane", "process-info", "--pane", pane])?;
    let candidates = process["result"]["process_info"]["foreground_processes"]
        .as_array()
        .context("source processes missing")?;
    let native = candidates
        .iter()
        .filter(|p| p["name"] == "codex")
        .collect::<Vec<_>>();
    ensure!(
        native.len() == 1,
        "source must have one native Codex frontend"
    );
    let pid = native[0]["pid"].as_u64().context("source PID missing")?;
    ensure!(
        process_is_descendant(pid, owner.into())?,
        "native frontend no longer belongs to its launcher"
    );
    let root = Project::discover(&original_cwd)?
        .map(|p| p.root().to_owned())
        .unwrap_or(original_cwd);
    let dir = directory(&root)?;
    let binding = Registration {
        schema: 3,
        agent: "codex".into(),
        native_session: session.to_string(),
        session,
        rollout,
        cwd: root,
        pane: pane.into(),
        terminal: terminal.into(),
        pid,
        client_owner: Some(owner),
        client_marker: Some(marker.into()),
    };
    recovery::write(
        &dir.join(format!("pane-{}.json", recovery::hash(pane.as_bytes()))),
        &serde_json::to_vec(&binding)?,
        true,
    )?;
    let launching_root = Project::discover(launch_root)?
        .map(|p| p.root().to_owned())
        .unwrap_or(launch_root.canonicalize()?);
    if launching_root != binding.cwd {
        let launching_dir = directory(&launching_root)?;
        recovery::write(
            &launching_dir.join(format!("pane-{}.json", recovery::hash(pane.as_bytes()))),
            &serde_json::to_vec(&binding)?,
            true,
        )?;
    }
    let current_root = Project::discover(cwd)?
        .map(|p| p.root().to_owned())
        .unwrap_or(cwd.canonicalize()?);
    if current_root != binding.cwd && current_root != launching_root {
        recovery::write(
            &directory(&current_root)?
                .join(format!("pane-{}.json", recovery::hash(pane.as_bytes()))),
            &serde_json::to_vec(&binding)?,
            true,
        )?;
    }
    if let Err(error) = herdr::call(&[
        "pane",
        "report-agent-session",
        pane,
        "--source",
        "herdr:codex-client",
        "--agent",
        "codex",
        "--agent-session-id",
        &session.to_string(),
    ]) {
        eprintln!("native metadata publication unavailable: {error:#}");
    }
    Ok(())
}

pub fn lookup(
    cwd: &Path,
    pane: &str,
    terminal: &str,
    pid: u64,
    agent: &str,
) -> Result<Registration> {
    let project = Project::discover(cwd)?;
    let standalone_root = cwd.canonicalize()?;
    let root = project
        .as_ref()
        .map(|p| p.root())
        .unwrap_or(&standalone_root);
    let path = root
        .join(".agent-progress/bridges")
        .join(format!("pane-{}.json", recovery::hash(pane.as_bytes())));
    let binding: Registration = serde_json::from_slice(&recovery::read(&path)?)?;
    ensure!(
        binding.schema == 2 || binding.schema == 3,
        "native registration predates process ownership proof; wait for a verified hook"
    );
    if binding.schema == 3 {
        let owner = binding
            .client_owner
            .context("client ownership proof missing")?;
        let marker = binding
            .client_marker
            .as_ref()
            .context("client selection proof missing")?;
        let selected: Value = serde_json::from_slice(&recovery::read(marker)?)?;
        ensure!(
            selected["session"] == binding.session.to_string() && selected["owner"] == owner,
            "native client selected another session; reconnect explicitly"
        );
        ensure!(
            agent == "codex" && process_is_descendant(pid, owner.into())?,
            "native client ownership changed"
        );
    }
    ensure!(
        binding.pane == pane
            && binding.terminal == terminal
            && binding.pid == pid
            && binding.agent == agent,
        "native hook registration is stale; reconnect after session start"
    );
    ensure!(
        live::session_header(&binding.rollout)?.0 == binding.session,
        "native hook stream identity changed"
    );
    ensure!(
        binding.schema == 3 || binding.cwd.canonicalize()? == root.canonicalize()?,
        "native registration project changed"
    );
    if agent != "codex" {
        ensure!(
            binding
                .rollout
                .canonicalize()?
                .starts_with(root.join(".agent-progress/bridges").canonicalize()?),
            "native stream escaped project storage"
        );
    }
    Ok(binding)
}
