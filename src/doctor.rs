//! Read-only connection diagnostics. Reports evidence and recovery without selecting another session.
use crate::{herdr, live, project::Project};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

pub fn inspect(project: Option<PathBuf>, pane: Option<String>, rollout: Option<PathBuf>) -> Value {
    let mut checks = vec![
        json!({"name":"host","status":"observed","os":std::env::consts::OS,"architecture":std::env::consts::ARCH}),
    ];
    for agent in ["codex", "claude", "opencode"] {
        let mut command = Command::new(agent);
        command.arg("--version");
        match herdr::bounded(command,None) {
            Ok(bytes) => checks.push(json!({"name":agent,"status":"observed","version":live::clean(&String::from_utf8_lossy(&bytes)).chars().take(100).collect::<String>(),"support":"native producer must pass on this version; version detection alone is not compatibility proof"})),
            Err(_) => checks.push(json!({"name":agent,"status":"unavailable","recovery":"Install the selected CLI and make it available in PATH; core file mode remains available"})),
        }
    }
    match project.map(|p| Project::open(&p)).unwrap_or_else(|| std::env::current_dir().map_err(anyhow::Error::from).and_then(|cwd| Project::discover(&cwd)?.ok_or_else(|| anyhow::anyhow!("no ap.project.json")))) {
        Ok(p) => match p.plan() {
            Ok(plan) => checks.push(json!({"name":"product","status":"valid","id":plan.id,"revision":plan.revision,"counts":plan.counts(),"state":p.state_path()})),
            Err(_) => checks.push(json!({"name":"product","status":"unreadable","state":p.state_path(),"recovery":"Inspect a backup with product restore --backup PATH; initialize only by reading the exact source plan"})),
        },
        Err(_) => checks.push(json!({"name":"product","status":"unavailable","recovery":"Use an explicit --project ap.project.json; no latest-project fallback"})),
    }
    let source = if let Some(pane) = pane {
        match herdr::resolve(&pane) {
            Ok(binding) => {
                checks.push(json!({"name":"pane","status":"connected","pane":binding.pane,"session":binding.session,"pid":binding.pid,"rollout":binding.rollout}));
                Some(binding.rollout)
            }
            Err(error) => {
                checks.push(json!({"name":"pane","status":"disconnected","pane":pane,"reason":error.to_string(),"recovery":"The exact source pane/session must exist; reconnect explicitly. Focused/latest panes are never substituted"}));
                None
            }
        }
    } else {
        rollout
    };
    if let Some(path) = source {
        match live::Feed::open(path, None, None).and_then(|mut feed| { feed.refresh_all()?; Ok(feed.snapshot) }) {
            Ok(s) => checks.push(json!({"name":"source","status":"readable","session":s.session,"plan_seen":s.plan_seen,"counts":s.counts(),"activity":s.activity,"warning":s.warning})),
            Err(_) => checks.push(json!({"name":"source","status":"unreadable","recovery":"Check exact rollout path, permissions and supported event schema; preserve the source and last good checkpoint"})),
        }
    }
    json!({"schema":1,"checks":checks,"mutates":false})
}
