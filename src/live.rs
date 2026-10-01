//! Read-only Codex session adapter. Only goal/plan fields leave this module.
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1_smol::Sha1;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Pending,
    Active,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: Uuid,
    pub title: String,
    pub state: StepState,
    pub present: bool,
    pub was_done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub objective: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub at: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archived {
    pub goal: Option<Goal>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u32,
    pub session: Uuid,
    pub project: String,
    pub goal: Option<Goal>,
    pub entries: Vec<Entry>,
    pub archives: Vec<Archived>,
    pub changes: Vec<Change>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_goal: Option<String>,
    pub activity: String,
    pub plan_seen: bool,
    pub last_plan_at: String,
    pub warning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall: Option<crate::project::ProjectView>,
}

impl Snapshot {
    /// Only an explicit goal or an accepted native/reported plan opens an automatic observer.
    pub fn has_progress(&self) -> bool {
        self.goal.is_some() || self.plan_seen
    }
    pub fn empty(session: Uuid, project: String) -> Self {
        Self {
            schema: 1,
            session,
            project,
            goal: None,
            entries: vec![],
            archives: vec![],
            changes: vec![],
            source: "계획 대기".into(),
            plan_goal: None,
            activity: "활동 미관측".into(),
            plan_seen: false,
            last_plan_at: String::new(),
            warning: None,
            overall: None,
        }
    }
    pub fn counts(&self) -> (usize, usize) {
        (
            self.entries
                .iter()
                .filter(|e| e.state == StepState::Done)
                .count(),
            self.entries.len(),
        )
    }
    pub fn change(&mut self, at: &str, text: String) {
        self.changes.push(Change {
            at: at.into(),
            text,
        });
    }
    pub fn set_goal(&mut self, goal: Goal, at: &str) {
        if self.goal.as_ref().is_some_and(|g| {
            g.objective == goal.objective && (g.id == goal.id || g.status == "agent-reported")
        }) {
            self.goal = Some(goal);
            return;
        }
        // A late-arriving first goal labels the existing plan, rather than losing it.
        if self.goal.is_some() && !self.entries.is_empty() {
            self.archives.push(Archived {
                goal: self.goal.take(),
                entries: std::mem::take(&mut self.entries),
            });
            self.plan_seen = false;
        }
        self.change(at, format!("목표 변경: {}", goal.objective));
        self.goal = Some(goal);
    }
    pub fn apply_plan(
        &mut self,
        steps: Vec<(String, StepState)>,
        source: &str,
        at: &str,
        reason: &str,
    ) -> Result<()> {
        ensure!(steps.len() <= 2000, "plan has more than 2000 steps");
        let mut unique = HashSet::new();
        for (title, _) in &steps {
            ensure!(!title.trim().is_empty(), "empty plan step");
            ensure!(
                unique.insert(entry_key(title)),
                "duplicate plan step; distinct titles are required"
            );
        }
        let previous = self.entries.clone();
        for entry in &mut self.entries {
            entry.present = false;
        }
        let mut ordered = Vec::new();
        for (title, state) in steps {
            let key = entry_key(&title);
            let mut entry =
                if let Some(i) = self.entries.iter().position(|e| entry_key(&e.title) == key) {
                    self.entries.remove(i)
                } else {
                    let scope = self.goal.as_ref().map(|g| g.id.as_str()).unwrap_or("plan");
                    Entry {
                        id: Uuid::new_v5(&self.session, format!("{scope}:{key}").as_bytes()),
                        title: title.clone(),
                        state,
                        present: true,
                        was_done: false,
                    }
                };
            entry.was_done |= entry.state == StepState::Done || state == StepState::Done;
            entry.title = clean(&title);
            entry.state = state;
            entry.present = true;
            ordered.push(entry);
        }
        // Omission is not cancellation. Keep both completed and unfinished history.
        ordered.append(&mut self.entries);
        self.entries = ordered;
        if previous != self.entries || !self.plan_seen {
            let added = self
                .entries
                .iter()
                .filter(|e| !previous.iter().any(|old| old.id == e.id))
                .count();
            let missing = self
                .entries
                .iter()
                .filter(|e| !e.present && e.state != StepState::Done)
                .count();
            let note = if reason.trim().is_empty() {
                "에이전트 계획 갱신"
            } else {
                reason
            };
            self.change(at, format!("{note} · 추가 {added} · 누락 미완료 {missing}"));
            for entry in self.entries.clone() {
                if let Some(old) = previous.iter().find(|old| old.id == entry.id)
                    && old.state != entry.state
                {
                    let label = |s| match s {
                        StepState::Pending => "예정",
                        StepState::Active => "진행",
                        StepState::Done => "완료 보고",
                    };
                    self.change(
                        at,
                        format!(
                            "{}: {} → {}",
                            entry.title,
                            label(old.state),
                            label(entry.state)
                        ),
                    );
                }
            }
            self.last_plan_at = at.into();
        }
        self.plan_seen = true;
        self.source = source.into();
        Ok(())
    }
}

fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn entry_key(text: &str) -> String {
    let text = text.trim();
    let token = text
        .strip_prefix('[')
        .and_then(|s| s.split_once(']').map(|(id, _)| id))
        .unwrap_or_else(|| text.split_whitespace().next().unwrap_or(""));
    if token.bytes().any(|b| b.is_ascii_digit())
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || "-_/".contains(c))
    {
        format!("id:{token}")
    } else {
        normalize(text)
    }
}
pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect()
}

pub fn session_header(path: &Path) -> Result<(Uuid, String)> {
    ensure!(
        fs::metadata(path)?.is_file(),
        "rollout must be a regular file"
    );
    let file = File::open(path).context("cannot read Codex rollout")?;
    let mut line = String::new();
    BufReader::new(file.take(1024 * 1024)).read_line(&mut line)?;
    let value: Value = serde_json::from_str(&line).context("invalid rollout header")?;
    ensure!(value["type"] == "session_meta", "not a Codex rollout");
    let id = value["payload"]["id"]
        .as_str()
        .context("session ID missing")?
        .parse()?;
    let cwd = value["payload"]["cwd"]
        .as_str()
        .context("session cwd missing")?;
    Ok((id, clean(cwd)))
}

pub fn find_session(home: &Path, session: Uuid) -> Result<PathBuf> {
    fn walk(dir: &Path, needle: &str, matches: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
        if depth > 4 || !dir.exists() {
            return Ok(());
        }
        for e in fs::read_dir(dir)? {
            let e = e?;
            let kind = e.file_type()?;
            if kind.is_dir() {
                walk(&e.path(), needle, matches, depth + 1)?;
            } else if kind.is_file() && e.file_name().to_string_lossy().ends_with(needle) {
                matches.push(e.path());
            }
        }
        Ok(())
    }
    let mut matches = Vec::new();
    walk(
        &home.join("sessions"),
        &format!("{session}.jsonl"),
        &mut matches,
        0,
    )?;
    ensure!(
        matches.len() == 1,
        "expected one exact session rollout, found {}",
        matches.len()
    );
    let path = matches.remove(0);
    ensure!(
        session_header(&path)?.0 == session,
        "session header mismatch"
    );
    Ok(path)
}

pub fn read_goal(home: &Path, session: Uuid) -> Result<Option<Goal>> {
    Ok(read_goal_record(home, session)?.map(|(goal, _)| goal))
}

fn read_goal_record(home: &Path, session: Uuid) -> Result<Option<(Goal, i64)>> {
    let db = home.join("goals_1.sqlite");
    if !db.exists() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_millis(100))?;
    conn.query_row(
        "SELECT goal_id, objective, status, created_at_ms FROM thread_goals WHERE thread_id = ?1",
        [session.to_string()],
        |r| {
            Ok((
                Goal {
                    id: r.get(0)?,
                    objective: r.get(1)?,
                    status: r.get(2)?,
                },
                r.get(3)?,
            ))
        },
    )
    .optional()
    .map(|v| {
        v.map(|(mut g, epoch)| {
            g.objective = clean(&g.objective);
            (g, epoch)
        })
    })
    .map_err(Into::into)
}

fn structured_plan(value: &Value) -> Result<Vec<(String, StepState)>> {
    let plan = value.as_array().context("plan must be an array")?;
    plan.iter()
        .map(|v| {
            let title = clean(v["step"].as_str().context("step title missing")?);
            let state = match v["status"].as_str().context("step status missing")? {
                "pending" => StepState::Pending,
                "in_progress" | "inProgress" => StepState::Active,
                "completed" => StepState::Done,
                _ => bail!("unknown plan status"),
            };
            Ok((title, state))
        })
        .collect()
}

/// Explicit agent plan sections only. Never turn arbitrary prose/user lists into tasks.
pub type PlanSteps = Vec<(String, StepState)>;
pub(crate) fn plan_lines(text: &str) -> Vec<&str> {
    // A sample inside a code block/quote is not the agent's actual plan.
    let mut fence: Option<(char, usize)> = None;
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('`') || trimmed.starts_with('~') {
                let marker = trimmed.chars().next().unwrap();
                let count = trimmed.chars().take_while(|c| *c == marker).count();
                if count >= 3 {
                    match fence {
                        None => fence = Some((marker, count)),
                        Some((open, len))
                            if marker == open
                                && count >= len
                                && trimmed[count..].trim().is_empty() =>
                        {
                            fence = None
                        }
                        _ => {}
                    }
                    return false;
                }
            }
            fence.is_none()
                && !trimmed.starts_with('>')
                && !line.starts_with("    ")
                && !line.starts_with('\t')
        })
        .collect()
}

pub fn markdown_plan(text: &str) -> Option<(Option<String>, PlanSteps)> {
    let lines = plan_lines(text);
    let start = lines.iter().position(|line| {
        matches!(
            line.trim(),
            "### 진행 계획" | "## 진행 계획" | "## Plan" | "### Plan" | "<proposed_plan>"
        )
    })?;
    let mut goal = None;
    let mut steps = Vec::new();
    for line in &lines[start + 1..] {
        let line = line.trim();
        if line.starts_with('#') || line == "</proposed_plan>" {
            break;
        }
        if let Some(v) = line
            .strip_prefix("목표:")
            .or_else(|| line.strip_prefix("Goal:"))
        {
            goal = Some(clean(v.trim()));
        }
        for (prefix, state) in [
            ("- [ ] ", StepState::Pending),
            ("- [x] ", StepState::Done),
            ("- [X] ", StepState::Done),
            ("- [>] ", StepState::Active),
        ] {
            if let Some(title) = line.strip_prefix(prefix) {
                steps.push((clean(title), state));
                break;
            }
        }
    }
    (!steps.is_empty()).then_some((goal, steps))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feed {
    pub snapshot: Snapshot,
    path: PathBuf,
    home: Option<PathBuf>,
    offset: u64,
    identity: (u64, u64),
    pending: HashMap<String, Value>,
    last_message: Option<String>,
    native_plan: bool,
    native_goal: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plan_warning: Option<String>,
    #[serde(default)]
    goal_epoch: Option<i64>,
    #[serde(default)]
    format_version: u32,
    #[serde(default)]
    prefix_hash: String,
    #[serde(default)]
    state_hash: String,
    #[serde(skip)]
    digest: Sha1,
    #[serde(skip)]
    stamp: Option<(u64, i64, i64)>,
    #[serde(skip)]
    cancel: Option<Arc<AtomicBool>>,
    #[serde(skip)]
    expected_cache: Option<Uuid>,
}

impl Feed {
    pub fn open(path: PathBuf, expected: Option<Uuid>, home: Option<PathBuf>) -> Result<Self> {
        let (session, project) = session_header(&path)?;
        ensure!(expected.is_none_or(|id| id == session), "wrong session");
        let meta = fs::metadata(&path)?;
        Ok(Self {
            snapshot: Snapshot::empty(session, project),
            path,
            home,
            offset: 0,
            identity: (meta.dev(), meta.ino()),
            pending: HashMap::new(),
            last_message: None,
            native_plan: false,
            native_goal: false,
            plan_warning: None,
            goal_epoch: None,
            format_version: 2,
            prefix_hash: Sha1::new().digest().to_string(),
            state_hash: String::new(),
            digest: Sha1::new(),
            stamp: None,
            cancel: None,
            expected_cache: None,
        })
    }
    pub fn refresh(&mut self) -> Result<bool> {
        let before = serde_json::to_vec(&self.snapshot)?;
        let mut file = File::open(&self.path).context("session file unavailable")?;
        let meta = file.metadata()?;
        ensure!(
            (meta.dev(), meta.ino()) == self.identity && meta.len() >= self.offset,
            "session file replaced or truncated; reconnect explicitly"
        );
        let stamp = (meta.len(), meta.mtime(), meta.mtime_nsec());
        if self.stamp != Some(stamp) && self.offset > 0 {
            let digest = hash_prefix(&mut file, self.offset, self.cancel.as_deref())?;
            ensure!(
                digest.digest().to_string() == self.prefix_hash,
                "committed session bytes changed; last good progress retained"
            );
            self.digest = digest;
        }
        self.poll_goal();
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take((meta.len() - self.offset).min(16 * 1024 * 1024))
            .read_to_end(&mut bytes)?;
        let end = bytes
            .iter()
            .rposition(|b| *b == b'\n')
            .map(|i| i + 1)
            .unwrap_or(0);
        ensure!(
            end > 0 || bytes.len() < 16 * 1024 * 1024,
            "session record exceeds 16 MiB"
        );
        for line in bytes[..end].split_inclusive(|b| *b == b'\n') {
            ensure!(
                !self
                    .cancel
                    .as_ref()
                    .is_some_and(|c| c.load(Ordering::Relaxed)),
                "cancelled"
            );
            let value: Value = serde_json::from_slice(line)
                .context("malformed complete session record; last good state retained")?;
            self.consume(&value)?;
            self.offset += line.len() as u64;
            self.digest.update(line);
            self.prefix_hash = self.digest.digest().to_string();
        }
        self.stamp = Some(stamp);
        if self.snapshot.warning.is_none() {
            self.snapshot.warning = self.plan_warning.clone();
        }
        Ok(before != serde_json::to_vec(&self.snapshot)?)
    }
    fn poll_goal(&mut self) {
        if let Some(home) = &self.home {
            match read_goal_record(home, self.snapshot.session) {
                Ok(Some((goal, epoch))) => {
                    let new_scope = !self.native_goal
                        || self
                            .snapshot
                            .goal
                            .as_ref()
                            .is_none_or(|g| g.id != goal.id || g.objective != goal.objective)
                        || self.goal_epoch != Some(epoch);
                    if new_scope {
                        // Existing work without a native goal belongs to the old scope.
                        if !self.snapshot.entries.is_empty() {
                            self.snapshot.archives.push(Archived {
                                goal: self.snapshot.goal.take(),
                                entries: std::mem::take(&mut self.snapshot.entries),
                            });
                        }
                        self.snapshot.plan_seen = false;
                        self.snapshot.source = "계획 대기".into();
                        self.snapshot.plan_goal = None;
                        self.snapshot.last_plan_at.clear();
                        self.native_plan = false;
                        self.plan_warning = None;
                        self.last_message = None;
                        self.pending.clear();
                    }
                    self.native_goal = true;
                    self.goal_epoch = Some(epoch);
                    self.snapshot.set_goal(goal, "goal");
                    self.snapshot.warning = None;
                }
                Ok(None) => {
                    if self.native_goal {
                        self.snapshot.warning =
                            Some("원본 goal이 삭제됨 · 마지막 목표 보존".into());
                    } else {
                        self.snapshot.warning = None;
                    }
                }
                Err(_) => {
                    self.snapshot.warning =
                        Some("goal 저장소를 읽을 수 없음 · plan은 계속 표시".into())
                }
            }
        }
    }
    fn consume(&mut self, record: &Value) -> Result<()> {
        let payload = &record["payload"];
        // Persisted Plan/AgentMessage timestamps use the same event clock as goals.
        // Prefer that clock over the formatted outer rollout timestamp.
        let native_at = payload["completed_at_ms"]
            .as_i64()
            .and_then(chrono::DateTime::from_timestamp_millis)
            .map(|t| t.to_rfc3339());
        let at = native_at
            .as_deref()
            .unwrap_or_else(|| record["timestamp"].as_str().unwrap_or(""));
        if payload["thread_id"]
            .as_str()
            .is_some_and(|id| id != self.snapshot.session.to_string())
        {
            return Ok(());
        }
        let eligible = self.goal_epoch.is_none_or(|epoch| {
            chrono::DateTime::parse_from_rfc3339(at)
                .ok()
                .is_some_and(|t| t.timestamp_millis() >= epoch)
        });
        match record["type"].as_str() {
            Some("session_meta") => ensure!(
                payload["id"].as_str() == Some(self.snapshot.session.to_string().as_str()),
                "session identity changed"
            ),
            Some("event_msg") => match payload["type"].as_str() {
                Some("task_started") => self.snapshot.activity = "작업 중 · 세션 이벤트".into(),
                Some("task_complete") => self.snapshot.activity = "응답 종료 · 세션 이벤트".into(),
                Some("turn_aborted") => self.snapshot.activity = "중단 · 세션 이벤트".into(),
                Some("plan_update") if eligible => {
                    self.apply_native(
                        &payload["plan"],
                        at,
                        payload["explanation"].as_str().unwrap_or(""),
                    )?;
                    if let Some(source) = payload["adapter_source"].as_str() {
                        self.snapshot.source = clean(source);
                    }
                }
                Some("item_completed") => {
                    let item = &payload["item"];
                    if item["type"] == "AgentMessage" && eligible {
                        self.message(item, at)?;
                        if let Some(source) = payload["adapter_source"].as_str() {
                            self.snapshot.source = clean(source);
                        }
                    } else if (item["type"] == "Plan" || item["type"] == "plan") && eligible {
                        self.native_plan_text(item["text"].as_str().unwrap_or(""), at)?;
                    }
                }
                _ => {}
            },
            Some("response_item") if eligible => {
                if payload["type"] == "function_call"
                    && payload["name"]
                        .as_str()
                        .is_some_and(|n| n == "update_plan" || n == "functions.update_plan")
                {
                    // Invalid requests are often followed by a tool error and a retry.
                    // Do not poison the stream before the tool has accepted a plan.
                    if let Some(args) = payload["arguments"]
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        && structured_plan(&args["plan"]).is_ok()
                        && let Some(id) = payload["call_id"].as_str()
                    {
                        self.pending.insert(id.into(), serde_json::json!({"plan":args["plan"], "explanation":args["explanation"]}));
                    }
                } else if payload["type"] == "function_call_output" {
                    if let Some(args) = payload["call_id"]
                        .as_str()
                        .and_then(|id| self.pending.remove(id))
                    {
                        let output = payload["output"].as_str().unwrap_or("");
                        if output.trim() == "Plan updated" {
                            self.apply_native(
                                &args["plan"],
                                at,
                                args["explanation"].as_str().unwrap_or(""),
                            )?;
                        }
                    }
                } else if payload["type"] == "message" && payload["role"] == "assistant" {
                    self.message(payload, at)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    fn apply_native(&mut self, plan: &Value, at: &str, reason: &str) -> Result<()> {
        let steps = structured_plan(plan)?;
        self.snapshot
            .apply_plan(steps, "Codex plan", at, &clean(reason))?;
        self.snapshot.plan_goal = None;
        self.native_plan = true;
        self.clear_plan_warning();
        Ok(())
    }
    fn native_plan_text(&mut self, text: &str, at: &str) -> Result<()> {
        let mut steps = Vec::new();
        for line in plan_lines(text) {
            let line = line.trim();
            for (prefix, state) in [
                ("- [ ] ", StepState::Pending),
                ("- [>] ", StepState::Active),
                ("- [x] ", StepState::Done),
                ("- [X] ", StepState::Done),
            ] {
                if let Some(title) = line.strip_prefix(prefix) {
                    steps.push((clean(title), state));
                    break;
                }
            }
        }
        if steps.is_empty() {
            // Native Plan mode also emits numbered implementation plans.
            for line in plan_lines(text) {
                if line.starts_with(' ') || line.starts_with('>') {
                    continue;
                }
                if let Some((number, title)) = line.split_once(". ")
                    && !number.is_empty()
                    && number.bytes().all(|b| b.is_ascii_digit())
                {
                    steps.push((clean(title), StepState::Pending));
                }
            }
        }
        if steps.is_empty() {
            self.native_plan = true;
            self.snapshot.source = "Codex native Plan".into();
            self.plan_warning = Some("native 계획에 체크 가능한 항목 없음 · 이전 기록 유지".into());
            return Ok(());
        }
        self.snapshot
            .apply_plan(steps, "Codex native Plan", at, "native Plan 항목 반영")?;
        self.snapshot.plan_goal = None;
        self.native_plan = true;
        self.clear_plan_warning();
        Ok(())
    }
    fn clear_plan_warning(&mut self) {
        let old = self.plan_warning.take();
        if self.snapshot.warning == old {
            self.snapshot.warning = None;
        }
    }
    fn message(&mut self, item: &Value, at: &str) -> Result<()> {
        let text = item["content"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let Some((goal, steps)) = markdown_plan(&text) else {
            return Ok(());
        };
        let replace_native = self.native_plan
            && !self.native_goal
            && goal.as_ref().is_some_and(|goal| {
                self.snapshot
                    .goal
                    .as_ref()
                    .is_none_or(|current| current.objective != *goal)
            });
        // Dual response_item + item_completed serialization must count once.
        let key = Uuid::new_v5(&self.snapshot.session, text.as_bytes()).to_string();
        if self.last_message.as_ref() == Some(&key) {
            return Ok(());
        }
        let mut next = self.snapshot.clone();
        if replace_native && !next.entries.is_empty() {
            next.archives.push(Archived {
                goal: next.goal.take(),
                entries: std::mem::take(&mut next.entries),
            });
        }
        if let Some(goal) = goal {
            next.plan_goal = Some(goal.clone());
            if !self.native_goal {
                next.set_goal(
                    Goal {
                        id: Uuid::new_v5(&next.session, goal.as_bytes()).to_string(),
                        objective: goal,
                        status: "agent-reported".into(),
                    },
                    at,
                );
            }
        }
        let source = if self.native_plan && !replace_native {
            "Codex native Plan · 진행 보고"
        } else {
            "에이전트 진행 계획"
        };
        next.apply_plan(steps, source, at, "대화의 명시적 계획 반영")?;
        self.snapshot = next;
        if replace_native {
            self.native_plan = false;
            self.pending.clear();
        }
        self.last_message = Some(key);
        Ok(())
    }

    pub fn restore_checkpoint(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }
        let bytes = checkpoint_bytes(path)?;
        self.expected_cache = Some(Uuid::new_v5(&Uuid::NAMESPACE_OID, &bytes));
        let mut saved: Self = serde_json::from_slice(&bytes)
            .context("invalid progress checkpoint; preserve it and select a different --cache")?;
        ensure!(
            saved.snapshot.schema == 1
                && saved.snapshot.session == self.snapshot.session
                && saved.path == self.path
                && saved.identity == self.identity,
            "progress checkpoint/source mismatch"
        );
        if saved.format_version < 2 {
            // Replay older checkpoints so the current plan goal can be recovered from
            // the original source without dropping their prior saved evidence.
            let backup = path.with_extension(format!("legacy-{}.json", Uuid::new_v4()));
            let mut temp =
                tempfile::NamedTempFile::new_in(path.parent().context("cache parent missing")?)?;
            temp.write_all(&bytes)?;
            temp.as_file().sync_all()?;
            temp.persist_noclobber(backup)?;
            return Ok(()); // Replay from the beginning using the corrected parser.
        }
        ensure!(saved.format_version == 2, "unsupported checkpoint format");
        verify_checkpoint_hash(&bytes)?;
        let mut source = File::open(&self.path)?;
        saved.digest = hash_prefix(&mut source, saved.offset, self.cancel.as_deref())?;
        ensure!(
            saved.digest.digest().to_string() == saved.prefix_hash,
            "checkpoint does not match source bytes"
        );
        ensure!(
            fs::metadata(&self.path)?.len() >= saved.offset,
            "source truncated; saved progress retained"
        );
        let home = self.home.take();
        let expected_cache = self.expected_cache;
        *self = saved;
        self.home = home;
        self.expected_cache = expected_cache;
        Ok(())
    }

    pub fn set_cancel(&mut self, cancel: Arc<AtomicBool>) {
        self.cancel = Some(cancel);
    }

    pub fn position(&self) -> u64 {
        self.offset
    }

    /// `--once` and first load must not silently stop at the first 16 MiB chunk.
    pub fn refresh_all(&mut self) -> Result<()> {
        let target = fs::metadata(&self.path)?.len();
        loop {
            let before = self.offset;
            self.refresh()?;
            if self.offset == before || self.offset >= target {
                return Ok(());
            }
        }
    }

    pub fn save_checkpoint(&mut self, path: &Path) -> Result<()> {
        use fs2::FileExt;
        let parent = path.parent().context("cache directory missing")?;
        fs::create_dir_all(parent)?;
        let lock_path = path.with_extension("ap-lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)?;
        lock.try_lock_exclusive()
            .context("another progress window is saving")?;
        if path.exists() {
            let previous_bytes = checkpoint_bytes(path)?;
            ensure!(
                self.expected_cache == Some(Uuid::new_v5(&Uuid::NAMESPACE_OID, &previous_bytes)),
                "checkpoint changed by another reader; refusing overwrite"
            );
            let previous: Self =
                serde_json::from_slice(&previous_bytes).context("invalid checkpoint retained")?;
            if previous.format_version >= 1 {
                verify_checkpoint_hash(&previous_bytes)?;
            }
            ensure!(
                previous.snapshot.session == self.snapshot.session
                    && previous.path == self.path
                    && previous.identity == self.identity
                    && previous.offset <= self.offset,
                "newer checkpoint exists; refusing overwrite"
            );
        }
        let mut value = serde_json::to_value(&*self)?;
        let checksum = checkpoint_hash(&mut value)?;
        value["state_hash"] = checksum.into();
        let bytes = serde_json::to_vec_pretty(&value)?;
        ensure!(bytes.len() <= 16 * 1024 * 1024, "checkpoint too large");
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        crate::fault::checkpoint("checkpoint:temp-synced");
        temp.persist(path)?;
        crate::fault::checkpoint("checkpoint:replaced");
        self.expected_cache = Some(Uuid::new_v5(&Uuid::NAMESPACE_OID, &bytes));
        File::open(parent)?.sync_all()?;
        crate::fault::checkpoint("checkpoint:dir-synced");
        Ok(())
    }
}

fn checkpoint_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "checkpoint must be a regular file"
    );
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16 * 1024 * 1024, "checkpoint too large");
    Ok(bytes)
}

fn checkpoint_hash(value: &mut Value) -> Result<String> {
    value
        .as_object_mut()
        .context("invalid checkpoint")?
        .remove("state_hash");
    Ok(Uuid::new_v5(&Uuid::NAMESPACE_OID, &serde_json::to_vec(value)?).to_string())
}

fn verify_checkpoint_hash(bytes: &[u8]) -> Result<()> {
    let mut value: Value = serde_json::from_slice(bytes)?;
    let expected = value["state_hash"]
        .as_str()
        .context("checkpoint checksum missing")?
        .to_owned();
    ensure!(
        checkpoint_hash(&mut value)? == expected,
        "checkpoint checksum mismatch; file retained"
    );
    Ok(())
}

fn hash_prefix(file: &mut File, length: u64, cancel: Option<&AtomicBool>) -> Result<Sha1> {
    file.rewind()?;
    let mut reader = file.take(length);
    let mut digest = Sha1::new();
    let mut buf = [0u8; 65536];
    let mut read = 0;
    loop {
        ensure!(
            !cancel.is_some_and(|c| c.load(Ordering::Relaxed)),
            "cancelled"
        );
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
        read += n as u64;
    }
    ensure!(read == length, "source truncated during verification");
    Ok(digest)
}
