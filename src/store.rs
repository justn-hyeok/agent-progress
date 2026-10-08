use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Todo,
    Doing,
    Blocked,
    Done,
    Cancelled,
}

impl State {
    pub fn mark(self) -> &'static str {
        match self {
            State::Todo => "[ ]",
            State::Doing => "[>]",
            State::Blocked => "[!]",
            State::Done => "[x]",
            State::Cancelled => "[-]",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Assigned once and never renumbered, so `ap done 2` stays correct after edits.
    pub id: u32,
    pub title: String,
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub needs: Option<String>,
    /// When the item last became done; shown as "마지막 완료".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub done_at: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewer {
    /// Unique per launched viewer. The viewer exits when the plan stops naming it,
    /// so closing never depends on pane IDs or key presses.
    #[serde(default)]
    pub instance: String,
    #[serde(default)]
    pub pane: String,
    /// The user's existing shell pane: the viewer exits back to their shell.
    #[serde(default)]
    pub reused: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Plan {
    pub schema: u32,
    pub key: String,
    /// Herdr terminal behind the key pane; a reused pane ID with another terminal starts fresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    pub next_id: u32,
    pub items: Vec<Item>,
    pub updated_at: u64,
}

impl Plan {
    pub fn new(key: &str) -> Self {
        Plan {
            schema: 1,
            key: key.into(),
            next_id: 1,
            ..Default::default()
        }
    }

    /// (done, counted). Cancelled items are excluded from both.
    pub fn progress(&self) -> (usize, usize) {
        let counted = self
            .items
            .iter()
            .filter(|i| i.state != State::Cancelled)
            .count();
        let done = self.items.iter().filter(|i| i.state == State::Done).count();
        (done, counted)
    }

    /// Whole-number percent: rounded, but 100 only when everything is done and at
    /// least 1 once anything is. None when there is nothing to count.
    pub fn percent(&self) -> Option<String> {
        let (done, total) = self.progress();
        percent(done, total)
    }

    pub fn progress_label(&self) -> String {
        match (self.progress(), self.percent()) {
            ((done, total), Some(pct)) => format!("{done}/{total} ({pct}%)"),
            _ => "미정".into(),
        }
    }

    pub fn find(&self, selector: &str) -> Result<usize> {
        let selector = selector.trim();
        if let Ok(id) = selector.parse::<u32>() {
            return self
                .items
                .iter()
                .position(|i| i.id == id)
                .with_context(|| format!("{id}번 항목이 없습니다"));
        }
        let matches: Vec<usize> = (0..self.items.len())
            .filter(|&n| self.items[n].title == selector)
            .collect();
        match matches.as_slice() {
            [n] => Ok(*n),
            [] => bail!("'{selector}' 항목이 없습니다"),
            _ => bail!("'{selector}'와 같은 항목이 여러 개입니다; 번호를 쓰세요"),
        }
    }

    pub fn add(&mut self, title: &str) -> Result<u32> {
        let title = title.trim();
        ensure!(!title.is_empty(), "빈 항목은 추가할 수 없습니다");
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Item {
            id,
            title: title.into(),
            state: State::Todo,
            reason: None,
            needs: None,
            done_at: None,
        });
        Ok(id)
    }

    pub fn set(
        &mut self,
        selector: &str,
        state: State,
        reason: Option<String>,
        needs: Option<String>,
    ) -> Result<u32> {
        let n = self.find(selector)?;
        if state == State::Blocked {
            ensure!(
                reason.as_deref().is_some_and(|r| !r.trim().is_empty()),
                "막힘에는 이유가 필요합니다"
            );
        }
        let item = &mut self.items[n];
        if state != State::Done {
            item.done_at = None;
        } else if item.state != State::Done {
            item.done_at = Some(now());
        }
        item.state = state;
        item.reason = reason;
        item.needs = needs;
        Ok(item.id)
    }

    pub fn last_done(&self) -> Option<&Item> {
        self.items
            .iter()
            .filter(|i| i.state == State::Done)
            .max_by_key(|i| (i.done_at, i.id))
    }

    pub fn remove(&mut self, selector: &str) -> Result<Item> {
        let n = self.find(selector)?;
        Ok(self.items.remove(n))
    }
}

/// Percent with up to two decimals: "50", "66.67". Rounded, but "100" only when
/// everything is done and at least "0.01" once anything is. None when nothing counts.
pub fn percent(done: usize, total: usize) -> Option<String> {
    if total == 0 {
        return None;
    }
    let hundredths = match done {
        0 => 0,
        d if d >= total => 10_000,
        d => ((d * 20_000 + total) / (2 * total)).clamp(1, 9_999),
    };
    Some(if hundredths % 100 == 0 {
        format!("{}", hundredths / 100)
    } else {
        format!("{}.{:02}", hundredths / 100, hundredths % 100)
    })
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Project root: the Git toplevel when available so `cd sub && ap done 1` finds the same plan.
pub fn project_root(cwd: &Path) -> PathBuf {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| cwd.to_path_buf())
}

pub fn sanitize(key: &str) -> String {
    let s: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() || s.starts_with('.') {
        format!("_{s}")
    } else {
        s
    }
}

/// Plans are local working state: keep the whole `.agent-progress` folder out of Git
/// in any project, without touching the project's own .gitignore.
fn ignore_in_git(plans_dir: &Path) {
    if let Some(base) = plans_dir.parent()
        && base.file_name().is_some_and(|n| n == ".agent-progress")
    {
        let file = base.join(".gitignore");
        if !file.exists() {
            let _ = fs::write(file, "*\n");
        }
    }
}

pub struct Store {
    pub path: PathBuf,
    lock: PathBuf,
    history: PathBuf,
}

impl Store {
    pub fn new(root: &Path, key: &str) -> Self {
        let dir = root.join(".agent-progress").join("plans");
        let name = sanitize(key);
        Store {
            path: dir.join(format!("{name}.json")),
            lock: dir.join(format!("{name}.lock")),
            history: dir.join(format!("{name}.history.jsonl")),
        }
    }

    pub fn at(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("plan.json");
        let stem = name.strip_suffix(".json").unwrap_or(name).to_string();
        Store {
            lock: path.with_file_name(format!("{stem}.lock")),
            history: path.with_file_name(format!("{stem}.history.jsonl")),
            path,
        }
    }

    pub fn load(&self) -> Result<Option<Plan>> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
                    format!("계획 파일 손상: {}", self.path.display())
                })?))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Read-modify-write under an exclusive lock; the file is replaced atomically.
    pub fn update<T>(
        &self,
        key: &str,
        event: &str,
        f: impl FnOnce(&mut Plan) -> Result<T>,
    ) -> Result<(Plan, T)> {
        let dir = self.path.parent().context("plan path")?;
        fs::create_dir_all(dir)?;
        ignore_in_git(dir);
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock)?;
        lock.lock_exclusive()?;
        let mut plan = self.load()?.unwrap_or_else(|| Plan::new(key));
        let out = f(&mut plan)?;
        plan.updated_at = now();
        self.write(&plan)?;
        if !event.is_empty() {
            let line = serde_json::json!({"at": plan.updated_at, "event": event, "progress": plan.progress_label()});
            let mut h = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.history)?;
            writeln!(h, "{line}")?;
        }
        drop(lock);
        Ok((plan, out))
    }

    fn write(&self, plan: &Plan) -> Result<()> {
        let dir = self.path.parent().context("plan path")?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        tmp.write_all(&serde_json::to_vec_pretty(plan)?)?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.path)?;
        Ok(())
    }

    /// Move the current plan and its history aside (kept, never overwritten) so the
    /// key starts empty. Caller holds no lock; this takes it.
    pub fn archive(&self) -> Result<Option<PathBuf>> {
        let dir = self.path.parent().context("plan path")?;
        if !self.path.exists() {
            return Ok(None);
        }
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock)?;
        lock.lock_exclusive()?;
        let stem = self
            .path
            .file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_suffix(".json"))
            .unwrap_or("plan")
            .to_string();
        let stamp = now();
        for n in 0.. {
            let target = dir.join(format!("{stem}.{stamp}-{n}.archived.json"));
            // hard_link fails instead of replacing an existing archive.
            match fs::hard_link(&self.path, &target) {
                Ok(()) => {
                    fs::remove_file(&self.path)?;
                    if self.history.exists() {
                        let history =
                            dir.join(format!("{stem}.{stamp}-{n}.archived.history.jsonl"));
                        fs::rename(&self.history, history)?;
                    }
                    return Ok(Some(target));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        unreachable!()
    }

    pub fn history_path(&self) -> &Path {
        &self.history
    }
}

#[cfg(test)]
mod tests {
    use super::percent;

    #[test]
    fn percent_has_two_decimals_but_never_overstates_the_ends() {
        let p = |d, t| percent(d, t);
        assert_eq!(p(0, 0), None);
        assert_eq!(p(1, 2).as_deref(), Some("50"));
        assert_eq!(p(2, 3).as_deref(), Some("66.67"));
        assert_eq!(p(1, 3).as_deref(), Some("33.33"));
        assert_eq!(p(1, 18).as_deref(), Some("5.56"));
        assert_eq!(p(199, 200).as_deref(), Some("99.50"));
        assert_eq!(
            p(19_999, 20_000).as_deref(),
            Some("99.99"),
            "not 100 before done"
        );
        assert_eq!(p(1, 20_000).as_deref(), Some("0.01"), "not 0 once started");
        assert_eq!(p(0, 27).as_deref(), Some("0"));
        assert_eq!(p(27, 27).as_deref(), Some("100"));
    }
}
