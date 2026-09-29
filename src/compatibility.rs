//! Local contracts and optional authenticated producer smoke; neither certifies all versions.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Seek},
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn version(agent: &str) -> Result<String> {
    let mut command = Command::new(agent);
    command.arg("--version");
    let bytes = crate::herdr::bounded(command, None)?;
    Ok(crate::live::clean(&String::from_utf8_lossy(&bytes))
        .trim()
        .chars()
        .take(100)
        .collect())
}

fn contracts(agent: &str) -> Result<Value> {
    let mut command = Command::new(agent);
    if agent == "opencode" {
        command.arg("run");
    }
    command.arg("--help");
    let help = String::from_utf8(crate::herdr::bounded_help(command)?)?;
    let required: &[&str] = match agent {
        "codex" => &["--remote", "resume", "app-server"],
        "claude" => &["--settings", "--output-format", "--resume"],
        "opencode" => &["--format", "--session", "--dir"],
        _ => anyhow::bail!("unsupported agent"),
    };
    for flag in required {
        ensure!(
            help.contains(flag),
            "required native CLI interface missing: {flag}"
        );
    }
    let mut checks = json!({"cli_interface":"passed"});
    if agent == "codex" {
        let directory = tempfile::tempdir()?;
        let mut command = Command::new(agent);
        command
            .args(["app-server", "generate-json-schema", "--out"])
            .arg(directory.path());
        crate::herdr::bounded(command, None)?;
        for method in [
            "ThreadStartResponse",
            "ThreadResumeResponse",
            "ThreadForkResponse",
        ] {
            let file = directory.path().join("v2").join(format!("{method}.json"));
            let schema: Value = serde_json::from_slice(
                &fs::read(file).context("native thread response schema missing")?,
            )?;
            let serialized = schema.to_string();
            for field in ["thread", "cwd", "id", "path"] {
                ensure!(
                    serialized.contains(&format!("\"{field}\"")),
                    "native thread response field missing: {field}"
                );
            }
        }
        checks["native_rpc_schema"] = json!("passed");
    }
    Ok(checks)
}

fn live_bytes(mut command: Command) -> Result<Vec<u8>> {
    let mut output = tempfile::tempfile()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(output.try_clone()?)
        .stderr(tempfile::tempfile()?)
        .process_group(0)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(90);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{}", child.id())])
                .status();
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "native producer timed out; authentication/provider or startup must be checked"
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    ensure!(
        status.success(),
        "native producer failed; check authentication/provider outside ap"
    );
    ensure!(
        output.metadata()?.len() <= 4 * 1024 * 1024,
        "native producer output too large"
    );
    output.rewind()?;
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn producer(agent: &str, model: Option<&str>) -> Result<Value> {
    let directory = tempfile::tempdir()?;
    let prompt = "Do not call any tools, read files, or change anything. Respond exactly with these three lines:\n### 진행 계획\n- [x] [AP-01] Compatibility observed\n- [ ] [AP-02] Next action";
    let mut command = Command::new(agent);
    command
        .current_dir(directory.path())
        .env("HERDR_ENV", "0")
        .env("AP_AUTO_OPEN", "0")
        .env_remove("AP_TERMINAL_SLOT");
    match agent {
        "codex" => {
            command.args([
                "exec",
                "--json",
                "--skip-git-repo-check",
                "--sandbox",
                "read-only",
                prompt,
            ]);
        }
        "claude" => {
            command.args([
                "--print",
                "--output-format",
                "json",
                "--tools",
                "",
                "--strict-mcp-config",
                "--setting-sources",
                "",
                "--max-turns",
                "1",
                prompt,
            ]);
        }
        "opencode" => {
            fs::write(
                directory.path().join("opencode.json"),
                json!({"permission":{"*":"deny"}}).to_string(),
            )?;
            command.args(["run", "--pure", "--format", "json", prompt]);
        }
        _ => anyhow::bail!("unsupported agent"),
    }
    if let Some(model) = model {
        command.args(["--model", model]);
    }
    let bytes = live_bytes(command)?;
    let mut text = String::new();
    for line in String::from_utf8(bytes)?.lines() {
        if let Ok(event) = serde_json::from_str::<Value>(line) {
            if agent == "codex"
                && event["type"] == "item.completed"
                && event["item"]["type"] == "agent_message"
            {
                text.push_str(event["item"]["text"].as_str().unwrap_or(""));
            } else if agent == "claude" && event["type"] == "result" && event["is_error"] != true {
                text.push_str(event["result"].as_str().unwrap_or(""));
            } else if agent == "opencode" && event["type"] == "text" {
                text.push_str(event["part"]["text"].as_str().unwrap_or(""));
            }
        }
    }
    let (_, plan) =
        crate::live::markdown_plan(&text).context("native producer plan format not recognized")?;
    ensure!(
        plan.len() == 2,
        "native producer did not preserve the expected plan"
    );
    let rollout = directory.path().join("producer.jsonl");
    let session = uuid::Uuid::new_v4();
    fs::write(
        &rollout,
        format!(
            "{}\n{}\n",
            json!({"type":"session_meta","payload":{"id":session,"cwd":directory.path()}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]}})
        ),
    )?;
    let mut feed = crate::live::Feed::open(rollout, Some(session), None)?;
    feed.refresh_all()?;
    ensure!(
        feed.snapshot.counts() == (1, 2),
        "native producer plan did not reach dashboard counts"
    );
    Ok(
        json!({"native_markdown_producer":"passed","native_task_tools":"not_checked","counts":[1,2]}),
    )
}

pub fn inspect(agent: &str, live: bool) -> Value {
    inspect_model(agent, live, None)
}

fn inspect_model(agent: &str, live: bool, model: Option<&str>) -> Value {
    let result = (|| -> Result<Value> {
        let version = version(agent)?;
        let checks = contracts(agent)?;
        let evidence = if live {
            producer(agent, model)?
        } else {
            json!({"native_markdown_producer":"not_checked","native_task_tools":"not_checked"})
        };
        Ok(
            json!({"agent":agent,"version":version,"status":"passed","contracts":checks,"evidence":evidence,"scope":"native CLI/RPC interface; --live also checks authenticated markdown producer, not all task tools or pane lifecycle"}),
        )
    })();
    result
        .unwrap_or_else(|error| json!({"agent":agent,"status":"failed","reason":error.to_string()}))
}

pub fn report(agents: &[String], live: bool) -> Value {
    report_model(agents, live, None)
}

pub fn report_model(agents: &[String], live: bool, model: Option<&str>) -> Value {
    let checks: Vec<_> = agents
        .iter()
        .map(|agent| inspect_model(agent, live, model))
        .collect();
    json!({"schema":1,"passed":checks.iter().all(|c| c["status"]=="passed"),"live":live,"checks":checks})
}

/// Automatic launch check: version and contract drift, without paid model calls.
pub fn on_launch(agent: &str) {
    let result = (|| -> Result<()> {
        let cwd = std::env::current_dir()?;
        let root = crate::project::Project::discover(&cwd)?
            .map(|p| p.root().to_owned())
            .unwrap_or(cwd);
        let directory = root.join(".agent-progress/compatibility");
        ensure!(
            !root.join(".agent-progress").is_symlink() && !directory.is_symlink(),
            "refusing symlink compatibility cache"
        );
        let current = version(agent)?;
        let path = directory.join(format!("{agent}.json"));
        let old = crate::recovery::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        // A failed contract is retried next launch. Successful versions avoid repeated probes.
        if old
            .as_ref()
            .is_some_and(|v| v["version"] == current && v["status"] == "passed")
        {
            return Ok(());
        }
        let mut check = inspect(agent, false);
        check["version"] = json!(current);
        check["version_changed"] = json!(old.as_ref().is_some_and(|v| v["version"] != current));
        if check["status"] != "passed" {
            eprintln!(
                "ap: {agent} connection compatibility failed: {}",
                check["reason"].as_str().unwrap_or("unknown")
            );
        } else if check["version_changed"] == true {
            eprintln!(
                "ap: {agent} version changed; local interface passed. Native producer recheck: ap compatibility --agent {agent} --live"
            );
        }
        fs::create_dir_all(directory)?;
        crate::recovery::write(&path, &serde_json::to_vec(&check)?, true)?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("ap: compatibility check unavailable: {error}");
    }
}
