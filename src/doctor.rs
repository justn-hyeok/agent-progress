//! Read-only diagnostics. Never substitute a latest/focused project, pane or session.
use crate::{herdr, live, project::Project, recovery, terminal};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Default)]
pub struct Options {
    pub project: Option<PathBuf>,
    pub pane: Option<String>,
    pub rollout: Option<PathBuf>,
    pub agent: Option<String>,
    pub shell: Option<String>,
    pub rc: Option<PathBuf>,
    pub terminal_slot: Option<PathBuf>,
}

pub fn inspect(project: Option<PathBuf>, pane: Option<String>, rollout: Option<PathBuf>) -> Value {
    inspect_options(Options {
        project,
        pane,
        rollout,
        ..Default::default()
    })
}

fn version(program: &str, argument: &str) -> Option<String> {
    let mut command = Command::new(program);
    command.arg(argument);
    herdr::bounded(command, None).ok().map(|bytes| {
        live::clean(&String::from_utf8_lossy(&bytes))
            .trim()
            .chars()
            .take(100)
            .collect()
    })
}

fn selected_project(path: Option<&Path>) -> Result<(PathBuf, Option<Project>)> {
    if let Some(path) = path {
        let project = Project::open(path)?;
        let root = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()?;
        ensure!(project.root() == root, "declaration outside selected root");
        return Ok((root, Some(project)));
    }
    let cwd = std::env::current_dir()?.canonicalize()?;
    let project = Project::discover(&cwd)?;
    let root = project.as_ref().map(|p| p.root().to_owned()).unwrap_or(cwd);
    Ok((root, project))
}

// Missing metadata means startup may still be in progress; broken links never do.
fn optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(_) => recovery::read(path).map(Some),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn terminal_check(path: &Path) -> Result<(Value, bool)> {
    let request = terminal::validate_slot(path)?;
    ensure!(
        ["codex", "claude", "opencode"].contains(&request.agent.as_str())
            && request.cwd.is_absolute()
            && request.cwd.is_dir(),
        "invalid terminal request"
    );
    let owner = optional_file(&path.join("owner"))?
        .map(|bytes| -> Result<u32> {
            let pid: u32 = String::from_utf8(bytes)?.trim().parse()?;
            ensure!(pid > 0 && pid <= i32::MAX as u32, "invalid owner pid");
            Ok(pid)
        })
        .transpose()?;
    let selection = optional_file(&path.join("selection.json"))?
        .map(|bytes| serde_json::from_slice::<terminal::Selection>(&bytes))
        .transpose()?;
    if let Some(selection) = &selection {
        ensure!(owner == Some(selection.owner), "selection owner mismatch");
        let (session, cwd) = live::session_header(&selection.rollout)?;
        ensure!(session == selection.session, "selection session mismatch");
        ensure!(
            Path::new(&cwd).canonicalize()? == selection.cwd.canonicalize()?,
            "selection root mismatch"
        );
    }
    if let Some(owner) = owner {
        let mut command = Command::new("kill");
        command.args(["-0", &owner.to_string()]);
        if herdr::bounded(command, None).is_err() {
            return Ok((
                json!({"name":"terminal-slot","status":"stale","diagnostic":"source_not_running","next_action":"inspect","recovery":"Inspect the exact source launcher; no other slot or session is substituted"}),
                false,
            ));
        }
    }
    Ok((
        json!({"name":"terminal-slot","status":if selection.is_some() {"connected"} else {"waiting"},"diagnostic":if selection.is_some() {"exact_selection_verified"} else {"startup_waiting"},"next_action":"none"}),
        true,
    ))
}

pub fn inspect_options(options: Options) -> Value {
    let mut healthy = true;
    let mut checks = vec![
        json!({"name":"host","status":"observed","os":std::env::consts::OS,"architecture":std::env::consts::ARCH}),
    ];
    let supported = ["codex", "claude", "opencode"];
    if options
        .agent
        .as_ref()
        .is_some_and(|a| !supported.contains(&a.as_str()))
    {
        healthy = false;
        checks.push(json!({"name":"agent","status":"unsupported","next_action":"inspect"}));
    }
    for agent in supported {
        let required = options.agent.as_deref() == Some(agent);
        match version(agent, "--version") {
            Some(version) => checks.push(json!({"name":agent,"status":"observed","version":version,"required":required,"support":"Native producer must pass on this version; version detection alone is not compatibility proof"})),
            None => {
                healthy &= !required;
                checks.push(json!({"name":agent,"status":"unavailable","required":required,"recovery":"Install the selected CLI and make it available in PATH; core file mode remains available"}));
            }
        }
    }
    match selected_project(options.project.as_deref()) {
        Ok((root, project)) => {
            match project {
                None => checks.push(json!({"name":"product","status":"optional","diagnostic":"no_declaration","next_action":"none"})),
                Some(p) => {
                    let result = p.validate_declaration().and_then(|()| {
                        match fs::symlink_metadata(p.state_path()) {
                            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
                            Err(e) => Err(e.into()),
                        Ok(_) => p.diagnostic_plan().map(Some),
                        }
                    });
                    match result {
                        Ok(Some(plan)) => checks.push(json!({"name":"product","status":"valid","id":plan.id,"revision":plan.revision,"counts":plan.counts(),"state":p.state_path()})),
                        Ok(None) => checks.push(json!({"name":"product","status":"not_observed","diagnostic":"declared_without_cached_state","next_action":"none"})),
                        Err(_) => {
                            healthy = false;
                            checks.push(json!({"name":"product","status":"unreadable","next_action":"inspect","recovery":"Inspect the existing declaration, roadmap and cached state; preserve backups and do not create a replacement goal"}));
                        }
                    }
                }
            }
            for agent in supported.into_iter().filter(|a| {
                options
                    .agent
                    .as_deref()
                    .is_none_or(|selected| selected == *a)
            }) {
                match crate::connection::manage_agent(&root, "preview", false, agent) {
                    Ok(preview) => {
                        healthy &= preview["connection_status"] != "conflict";
                        checks.push(json!({"name":format!("connection:{agent}"),"status":preview["connection_status"],"diagnostic":preview["diagnostic"],"next_action":preview["next_action"]}));
                    }
                    Err(_) => {
                        healthy = false;
                        checks.push(json!({"name":format!("connection:{agent}"),"status":"conflict","diagnostic":"connection_preview_unavailable","next_action":"inspect"}));
                    }
                }
            }
        }
        Err(_) => {
            healthy = false;
            checks.push(json!({"name":"product","status":"unreadable","diagnostic":"invalid_or_missing_selected_declaration","next_action":"inspect","recovery":"Check the exact project declaration; no different project or working directory is substituted"}));
        }
    }

    let explicit_shell = options.shell.is_some() || options.rc.is_some();
    let shell = options
        .shell
        .or_else(|| options.rc.as_ref().map(|_| "zsh".to_owned()))
        .or_else(|| {
            std::env::var_os("SHELL").and_then(|s| {
                Path::new(&s)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(str::to_owned)
            })
        });
    match shell.filter(|s| ["zsh", "bash", "fish"].contains(&s.as_str())) {
        Some(shell) => match crate::shell::inspect_selected(&shell, options.rc.as_deref()) {
            Ok(report) => {
                let status = report["status"].as_str().unwrap_or("conflict");
                healthy &= if explicit_shell {
                    status == "current"
                } else {
                    !matches!(status, "outdated" | "conflict")
                };
                // Whitelist summary fields: never forward rc bytes or managed snippets.
                checks.push(json!({"name":"shell","shell":shell,"status":status,"explicit":explicit_shell,"upgrade_required":report["upgrade_required"],"next_action":report["next_action"]}));
            }
            Err(_) => {
                healthy = false;
                checks.push(json!({"name":"shell","shell":shell,"status":"conflict","next_action":"inspect","diagnostic":"shell_configuration_unreadable"}));
            }
        },
        None => {
            healthy &= !explicit_shell;
            checks.push(json!({"name":"shell","status":if explicit_shell {"conflict"} else {"skipped"},"diagnostic":"supported_shell_not_selected","next_action":if explicit_shell {"inspect"} else {"none"}}));
        }
    }
    checks.push(json!({"name":"terminal","status":"observed","backend":if std::env::var("HERDR_ENV").as_deref() == Ok("1") {"herdr"} else if std::env::var_os("TMUX").is_some() {"tmux"} else {"standalone"}}));
    match version("tmux", "-V") {
        Some(version) => checks.push(json!({"name":"tmux","status":"observed","version":version,"required":false})),
        None => checks.push(json!({"name":"tmux","status":"unavailable","required":false,"recovery":"Optional tmux display is unavailable; use explicit file/rollout observation. No terminal backend is substituted automatically"})),
    }
    if let Some(slot) = options.terminal_slot.or_else(terminal::slot) {
        match terminal_check(&slot) {
            Ok((check, valid)) => {
                healthy &= valid;
                checks.push(check);
            }
            Err(_) => {
                healthy = false;
                checks.push(json!({"name":"terminal-slot","status":"conflict","diagnostic":"invalid_slot_or_selection","next_action":"inspect","recovery":"Inspect the exact slot metadata and source identity; preserve evidence and never select a newest session"}));
            }
        }
    }

    let source = if let Some(pane) = options.pane {
        match herdr::resolve(&pane) {
            Ok(binding) => {
                checks.push(json!({"name":"pane","status":"connected","pane":binding.pane,"session":binding.session,"pid":binding.pid,"rollout":binding.rollout}));
                Some(binding.rollout)
            }
            Err(_) => {
                healthy = false;
                checks.push(json!({"name":"pane","status":"disconnected","pane":pane,"recovery":"The exact source pane/session must exist; reconnect explicitly. Focused/latest panes are never substituted"}));
                None
            }
        }
    } else {
        options.rollout
    };
    if let Some(path) = source {
        match live::Feed::open(path, None, None).and_then(|mut feed| { feed.refresh_all()?; Ok(feed.snapshot) }) {
            Ok(s) => checks.push(json!({"name":"source","status":"readable","session":s.session,"plan_seen":s.plan_seen,"counts":s.counts(),"activity":s.activity,"warning":s.warning})),
            Err(_) => {
                healthy = false;
                checks.push(json!({"name":"source","status":"unreadable","recovery":"Check exact rollout path, permissions and supported event schema; preserve the source and last good checkpoint"}));
            }
        }
    }
    json!({"schema":1,"checks":checks,"mutates":false,"healthy":healthy})
}
