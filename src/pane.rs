//! One progress viewer per source pane. The pane's state file names the plan to show
//! (the most recently changed one from that pane), the running viewer instance and
//! whether the user dismissed it. Plans themselves carry no viewer state, so working
//! on several projects from one pane switches the same viewer instead of stacking new ones.

use crate::store::{Viewer, now, sanitize};
use anyhow::{Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

/// A running viewer refreshes its heartbeat every few seconds; older than this means gone.
const HEARTBEAT_STALE_SECS: u64 = 6;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneState {
    /// Plan file the viewer should show.
    pub plan: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<Viewer>,
    /// Set by q or `ap close`; stops automatic opening until `ap open`.
    #[serde(default)]
    pub suppressed: bool,
    /// Terminal behind the source pane; a reused pane ID starts with fresh state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    #[serde(default)]
    pub updated_at: u64,
}

pub struct Panes {
    pub path: PathBuf,
    lock: PathBuf,
    heartbeat: PathBuf,
}

fn state_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/state")))
        .map(|d| d.join("agent-progress/panes"))
}

impl Panes {
    pub fn for_source(host: &str, pane: &str) -> Option<Self> {
        Some(Self::at(state_dir()?.join(format!(
            "{}.json",
            sanitize(&format!("{host}-{pane}"))
        ))))
    }

    pub fn at(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("pane.json");
        let stem = name.strip_suffix(".json").unwrap_or(name).to_string();
        Panes {
            lock: path.with_file_name(format!("{stem}.lock")),
            heartbeat: path.with_file_name(format!("{stem}.viewer.json")),
            path,
        }
    }

    pub fn load(&self) -> Option<PaneState> {
        fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    /// Read-modify-write under an exclusive lock; replaced atomically.
    pub fn update(&self, f: impl FnOnce(&mut PaneState)) -> Result<PaneState> {
        let dir = self.path.parent().context("pane state path")?;
        fs::create_dir_all(dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock)?;
        lock.lock_exclusive()?;
        let mut state = self.load().unwrap_or_default();
        f(&mut state);
        state.updated_at = now();
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        tmp.write_all(&serde_json::to_vec_pretty(&state)?)?;
        tmp.persist(&self.path)?;
        drop(lock);
        Ok(state)
    }

    pub fn beat(&self, instance: &str) -> Result<()> {
        let dir = self.heartbeat.parent().context("pane state path")?;
        fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        tmp.write_all(&serde_json::to_vec(
            &serde_json::json!({"instance": instance, "at": now()}),
        )?)?;
        tmp.persist(&self.heartbeat)?;
        Ok(())
    }

    pub fn clear_beat(&self, instance: &str) {
        if self.beating(instance) {
            let _ = fs::remove_file(&self.heartbeat);
        }
    }

    /// The viewer with this instance refreshed its heartbeat recently.
    pub fn beating(&self, instance: &str) -> bool {
        !instance.is_empty()
            && fs::read(&self.heartbeat)
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .is_some_and(|v| {
                    v["instance"] == instance
                        && now().saturating_sub(v["at"].as_u64().unwrap_or(0))
                            <= HEARTBEAT_STALE_SECS
                })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Start a viewer for this pane.
    Launch,
    /// Keep the current state: a live viewer already follows the pane's plan, or the user
    /// dismissed it, or automatic opening is off.
    Keep,
}

/// What to do after a change from this pane. `alive` is whether the recorded viewer is
/// still beating. A recorded viewer that stopped beating was closed from outside (e.g.
/// Herdr's own close), which counts as a dismissal unless the user asked to open.
pub fn decide(state: &PaneState, alive: bool, force: bool, auto_open: bool) -> Decision {
    if force {
        return if alive {
            Decision::Keep
        } else {
            Decision::Launch
        };
    }
    if state.suppressed || !auto_open || state.viewer.is_some() {
        return Decision::Keep;
    }
    Decision::Launch
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer() -> Option<Viewer> {
        Some(Viewer {
            instance: "v1".into(),
            ..Viewer::default()
        })
    }

    #[test]
    fn first_change_launches_and_live_viewer_is_reused() {
        let mut state = PaneState::default();
        assert_eq!(decide(&state, false, false, true), Decision::Launch);
        state.viewer = viewer();
        assert_eq!(decide(&state, true, false, true), Decision::Keep);
    }

    #[test]
    fn dismissal_and_external_close_are_respected_until_open() {
        let mut state = PaneState {
            suppressed: true,
            ..PaneState::default()
        };
        assert_eq!(decide(&state, false, false, true), Decision::Keep);
        assert_eq!(decide(&state, false, true, true), Decision::Launch);
        state.suppressed = false;
        state.viewer = viewer();
        assert_eq!(decide(&state, false, false, true), Decision::Keep);
        assert_eq!(decide(&state, false, true, true), Decision::Launch);
        assert_eq!(decide(&state, true, true, true), Decision::Keep);
    }

    #[test]
    fn auto_open_off_never_launches_on_its_own() {
        assert_eq!(
            decide(&PaneState::default(), false, false, false),
            Decision::Keep
        );
    }

    #[test]
    fn state_round_trips_and_heartbeat_expires_by_instance() {
        let dir = tempfile::tempdir().unwrap();
        let panes = Panes::at(dir.path().join("herdr-w1_p1.json"));
        let state = panes
            .update(|s| {
                s.plan = "/p/a.json".into();
                s.viewer = viewer();
            })
            .unwrap();
        assert_eq!(panes.load().unwrap(), state);
        assert!(!panes.beating("v1"));
        panes.beat("v1").unwrap();
        assert!(panes.beating("v1") && !panes.beating("v2"));
        panes.clear_beat("v2");
        assert!(panes.beating("v1"));
        panes.clear_beat("v1");
        assert!(!panes.beating("v1"));
    }
}
