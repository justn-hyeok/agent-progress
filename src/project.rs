//! Persistent product goal, independent of the source thread and its temporary plan.
use crate::{
    live::{Entry, Goal, Snapshot, StepState},
    model::{self, Evidence, Plan, Session, Status, Task, Verification},
};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub project_id: Uuid,
    pub objective: String,
    pub roadmap: PathBuf,
    #[serde(default)]
    pub required: BTreeMap<String, Verification>,
    #[serde(default)]
    pub tasks: BTreeMap<String, TaskPolicy>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPolicy {
    #[serde(default)]
    pub criteria: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub revision_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectView {
    pub project_id: Uuid,
    pub manifest: PathBuf,
    pub roadmap: PathBuf,
    pub plan: Plan,
    pub session_goal: Option<Goal>,
    pub session_plan: Vec<Entry>,
    pub current_work: Option<Entry>,
    pub parent_key: Option<String>,
    pub notice: Option<String>,
    pub session_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionState {
    signature: Uuid,
    goal: Option<Goal>,
    last_plan_at: String,
    plan: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    schema: u32,
    plan: Plan,
    keys: BTreeMap<String, Uuid>,
    document_states: BTreeMap<String, StepState>,
    sessions: BTreeMap<Uuid, SessionState>,
    last_event: BTreeMap<Uuid, i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checksum: Option<String>,
}

fn decode_state(bytes: &[u8]) -> Result<State> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(expected) = value
        .as_object_mut()
        .context("product state must be object")?
        .remove("checksum")
    {
        ensure!(
            expected.as_str() == Some(&crate::recovery::hash(&serde_json::to_vec(&value)?)),
            "product state checksum mismatch; inspect backup before restoring"
        );
    }
    // v0.3 states have no checksum; validate and preserve them in the first backup.
    let state: State = serde_json::from_slice(bytes)?;
    ensure!(matches!(state.schema, 1 | 2), "unsupported product schema");
    ensure!(
        state.schema != 2 || state.checksum.is_some(),
        "v2 product state checksum missing"
    );
    state.plan.validate()?;
    let ids: HashSet<_> = state.plan.tasks.iter().map(|t| t.id).collect();
    ensure!(
        state.keys.len() == ids.len()
            && state.keys.values().copied().collect::<HashSet<_>>() == ids
            && state.keys.keys().all(|key| valid_key(key)),
        "product task mapping mismatch"
    );
    ensure!(
        state
            .document_states
            .keys()
            .all(|key| state.keys.contains_key(key))
            && state.last_event.keys().all(|id| ids.contains(id)),
        "product history mapping mismatch"
    );
    Ok(state)
}

#[derive(Clone)]
pub struct Project {
    manifest: PathBuf,
    root: PathBuf,
    project_id: Uuid,
}

#[derive(Debug)]
struct Item {
    key: String,
    title: String,
    state: StepState,
}

fn read_limited(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "expected a regular project file"
    );
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "project file exceeds 16 MiB"
    );
    Ok(bytes)
}

fn save_state(path: &Path, state: &State) -> Result<()> {
    state.plan.validate()?;
    let mut value = serde_json::to_value(state)?;
    value.as_object_mut().unwrap().remove("checksum");
    value["schema"] = 2.into();
    value["plan"]["schema_version"] = 2.into();
    let hash = crate::recovery::hash(&serde_json::to_vec(&value)?);
    value["checksum"] = hash.into();
    let bytes = serde_json::to_vec_pretty(&value)?;
    ensure!(
        bytes.len() <= 16 * 1024 * 1024,
        "product state exceeds 16 MiB"
    );
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().context("state parent missing")?)?;
    temp.write_all(&bytes)?;
    temp.as_file().sync_all()?;
    crate::fault::checkpoint("product:temp-synced");
    if path.exists() {
        let previous = crate::recovery::read(path)?;
        if let Ok(old) = decode_state(&previous)
            && matches!(old.schema, 1 | 2)
            && old.plan.id == state.plan.id
            && old.plan.validate().is_ok()
        {
            crate::recovery::write(&path.with_extension("previous.json"), &previous, true)?;
            if old.schema == 1 && !path.with_extension("v1-backup.json").exists() {
                crate::recovery::write(&path.with_extension("v1-backup.json"), &previous, false)?;
            }
        }
    }
    crate::fault::checkpoint("product:backed-up");
    temp.persist(path)?;
    crate::fault::checkpoint("product:replaced");
    File::open(path.parent().unwrap())?.sync_all()?;
    crate::fault::checkpoint("product:dir-synced");
    Ok(())
}

fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key.bytes().any(|c| c.is_ascii_digit())
        && key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn roadmap(text: &str) -> Result<Vec<Item>> {
    let mut items = Vec::new();
    let mut keys = HashSet::new();
    let mut fence: Option<char> = None;
    for line in text.lines() {
        let line = line.trim_end();
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker = trimmed.chars().next().unwrap();
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
            continue;
        }
        if fence.is_some() || line.starts_with(' ') || line.starts_with('>') {
            continue;
        }
        let Some((rest, state)) = [
            ("- [ ] ", StepState::Pending),
            ("- [>] ", StepState::Active),
            ("- [x] ", StepState::Done),
            ("- [X] ", StepState::Done),
        ]
        .iter()
        .find_map(|(p, s)| line.strip_prefix(p).map(|rest| (rest, *s))) else {
            continue;
        };
        let Some((key, title)) = rest.split_once(' ') else {
            continue;
        };
        if !valid_key(key) {
            continue;
        }
        ensure!(keys.insert(key.to_owned()), "duplicate roadmap ID: {key}");
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        model::nonempty(&title, "roadmap title")?;
        items.push(Item {
            key: key.into(),
            title,
            state,
        });
    }
    ensure!(
        !items.is_empty(),
        "roadmap has no stable-ID checklist items"
    );
    ensure!(items.len() <= 2000, "roadmap has too many items");
    Ok(items)
}

fn event_time(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.timestamp_millis())
}

/// Exact root ID, explicit child ID, or exact unique title. Never semantic guessing.
fn mapping<'a>(title: &str, state: &'a State) -> Option<(&'a str, bool)> {
    let text = title.trim();
    let token = text
        .strip_prefix('[')
        .and_then(|s| s.split_once(']').map(|(token, _)| token))
        .unwrap_or_else(|| text.split_whitespace().next().unwrap_or(""));
    if let Some((key, _)) = state.keys.get_key_value(token) {
        return Some((key, true));
    }
    if let Some((root, _)) = token.split_once('/')
        && let Some((key, _)) = state.keys.get_key_value(root)
    {
        return Some((key, false));
    }
    let mut exact = state.plan.tasks.iter().filter(|t| t.title == text);
    let task = exact.next()?;
    if exact.next().is_some() {
        return None;
    }
    state
        .keys
        .iter()
        .find(|(_, id)| **id == task.id)
        .map(|(key, _)| (key.as_str(), true))
}

fn report(task: &mut Task, status: StepState, source: &str, actor: &str) {
    task.report(
        match status {
            StepState::Pending => Status::Planned,
            StepState::Active => Status::Active,
            StepState::Done => Status::Done,
        },
        actor,
        source,
    );
}

impl Project {
    pub fn discover(cwd: &Path) -> Result<Option<Self>> {
        let cwd = cwd.canonicalize()?;
        for dir in cwd.ancestors() {
            let path = dir.join("ap.project.json");
            if path.exists() || path.is_symlink() {
                return Self::open(&path).map(Some);
            }
        }
        Ok(None)
    }
    pub fn open(path: &Path) -> Result<Self> {
        let path = path.canonicalize()?;
        let manifest: Manifest = serde_json::from_slice(&read_limited(&path)?)?;
        ensure!(manifest.schema == 1, "unsupported project manifest");
        model::nonempty(&manifest.objective, "product objective")?;
        let root = path
            .parent()
            .context("project directory missing")?
            .to_owned();
        ensure!(
            !root.join(".ap-importing").exists(),
            "project import incomplete; preserve source bundle and choose a new empty destination"
        );
        Ok(Self {
            manifest: path,
            root,
            project_id: manifest.project_id,
        })
    }
    fn config(&self) -> Result<(Manifest, PathBuf)> {
        let storage = self.root.join(".agent-progress");
        ensure!(
            !storage.is_symlink() && (!storage.exists() || storage.is_dir()),
            "refusing symlink/non-directory product storage"
        );
        let state = self.state_path();
        ensure!(!state.is_symlink(), "refusing symlink product state");
        ensure!(
            !state.with_extension("ap-lock").is_symlink(),
            "refusing symlink product lock"
        );
        let m: Manifest = serde_json::from_slice(&read_limited(&self.manifest)?)?;
        ensure!(
            m.schema == 1 && m.project_id == self.project_id,
            "project identity changed; reconnect explicitly"
        );
        model::nonempty(&m.objective, "product objective")?;
        ensure!(
            m.roadmap.is_relative(),
            "roadmap must be relative to project"
        );
        let path = self.root.join(&m.roadmap).canonicalize()?;
        ensure!(path.starts_with(&self.root), "roadmap escapes project");
        Ok((m, path))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// Check declaration/roadmap paths without generating or changing cached progress.
    pub fn validate_declaration(&self) -> Result<()> {
        let (_, document) = self.config()?;
        let text = String::from_utf8(read_limited(&document)?)?;
        roadmap(&text)?;
        Ok(())
    }
    /// Read-only diagnostic view; an older objective cannot pass as the current product.
    pub fn diagnostic_plan(&self) -> Result<Plan> {
        let (manifest, _) = self.config()?;
        let state = self.validated_state(&crate::recovery::read(&self.state_path())?)?;
        ensure!(
            state.plan.goal == manifest.objective,
            "cached objective differs; explicit migration required"
        );
        Ok(state.plan)
    }

    pub fn state_path(&self) -> PathBuf {
        self.root
            .join(".agent-progress")
            .join(format!("project-{}.json", self.project_id))
    }

    /// Offline product view. Reading an explicitly selected product never imports another session.
    pub fn snapshot(&self) -> Result<Snapshot> {
        let (manifest, _) = self.config()?;
        let state = self.validated_state(&crate::recovery::read(&self.state_path())?)?;
        let mut output = Snapshot::empty(Uuid::nil(), self.root.display().to_string());
        output.goal = Some(Goal {
            id: self.project_id.to_string(),
            objective: manifest.objective,
            status: "product".into(),
        });
        output.plan_seen = true;
        output.source = "저장된 제품 계획 · 활동 미관측".into();
        output.entries = state
            .plan
            .tasks
            .iter()
            .filter(|t| t.status != Status::Cancelled)
            .map(|t| {
                let key = state
                    .keys
                    .iter()
                    .find(|(_, id)| **id == t.id)
                    .map(|(key, _)| key.as_str())
                    .unwrap_or("?");
                Entry {
                    id: t.id,
                    title: format!("{key} · {} · {}", t.title, t.status.label()),
                    state: match t.status {
                        Status::Done => StepState::Done,
                        Status::Active => StepState::Active,
                        _ => StepState::Pending,
                    },
                    present: true,
                    was_done: t.evidence.iter().any(|e| e.kind == Verification::Reported),
                }
            })
            .collect();
        let current = output
            .entries
            .iter()
            .find(|e| e.state == StepState::Active)
            .cloned();
        output.overall = Some(ProjectView {
            project_id: self.project_id,
            manifest: self.manifest.clone(),
            roadmap: manifest.roadmap,
            plan: state.plan,
            session_goal: None,
            session_plan: vec![],
            current_work: current,
            parent_key: None,
            notice: Some("오프라인 제품 기록; 에이전트 활동은 미관측".into()),
            session_count: state.sessions.len(),
        });
        Ok(output)
    }

    fn validated_state(&self, bytes: &[u8]) -> Result<State> {
        let state = decode_state(bytes)?;
        ensure!(
            matches!(state.schema, 1 | 2) && state.plan.id == self.project_id,
            "backup belongs to another product/schema"
        );
        state.plan.validate()?;
        let ids: HashSet<_> = state.plan.tasks.iter().map(|t| t.id).collect();
        ensure!(
            state.keys.len() == ids.len()
                && state.keys.values().copied().collect::<HashSet<_>>() == ids,
            "backup task mapping mismatch"
        );
        Ok(state)
    }

    pub fn backup(&self, output: &Path) -> Result<()> {
        let bytes = crate::recovery::read(&self.state_path())?;
        self.validated_state(&bytes)?;
        crate::recovery::write(output, &bytes, false)
    }

    pub fn restore(
        &self,
        backup: &Path,
        expected: Option<&str>,
        actor: &str,
    ) -> Result<serde_json::Value> {
        let path = self.state_path();
        let candidate = crate::recovery::read(backup)?;
        let mut state = self.validated_state(&candidate)?;
        let (manifest, _) = self.config()?;
        ensure!(
            state.plan.goal == manifest.objective,
            "backup objective differs; explicit migration required"
        );
        let current = crate::recovery::read(&path)?;
        let digest = crate::recovery::hash(&current);
        let (done, total) = state.plan.counts();
        let preview = serde_json::json!({"current_hash":digest,"backup_revision":state.plan.revision,"done":done,"total":total,"apply":"repeat with --expect-hash current_hash; displaced data is preserved"});
        let Some(expected) = expected else {
            return Ok(preview);
        };
        ensure!(digest == expected, "restore conflict; preview again");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("ap-lock"))?;
        lock.try_lock_exclusive()
            .context("product progress is busy")?;
        ensure!(
            crate::recovery::read(&path)? == current,
            "restore conflict; state changed"
        );
        let displaced = path.with_extension(format!("displaced-{}.json", Uuid::new_v4()));
        crate::recovery::write(&displaced, &current, false)?;
        state.plan.record(
            actor,
            "product:restore",
            None,
            "명시적 백업 복원; 이전 bytes 보존",
        )?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .min(i64::MAX as u128) as i64;
        for task in &state.plan.tasks {
            state.last_event.insert(task.id, now);
        }
        save_state(&path, &state)?;
        Ok(serde_json::json!({"restored":true,"displaced":displaced,"plan":state.plan}))
    }

    pub fn export(&self, output: &Path) -> Result<()> {
        let (manifest, roadmap) = self.config()?;
        let state = self.validated_state(&crate::recovery::read(&self.state_path())?)?;
        let bundle = serde_json::json!({"schema":1,"manifest":manifest,"roadmap":String::from_utf8(read_limited(&roadmap)?)?,"state":state});
        crate::recovery::write(output, &serde_json::to_vec_pretty(&bundle)?, false)
    }

    pub fn import_bundle(input: &Path, destination: &Path) -> Result<Self> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Bundle {
            schema: u32,
            manifest: Manifest,
            roadmap: String,
            state: State,
        }
        let bytes = crate::recovery::read(input)?;
        let bundle: Bundle = serde_json::from_slice(&bytes)?;
        ensure!(
            bundle.schema == 1
                && bundle.manifest.schema == 1
                && matches!(bundle.state.schema, 1 | 2)
                && bundle.manifest.project_id == bundle.state.plan.id
                && bundle.manifest.objective == bundle.state.plan.goal,
            "invalid bundle identity/schema"
        );
        let encoded = serde_json::to_vec(&bundle.state)?;
        decode_state(&encoded)?;
        ensure!(
            bundle.manifest.roadmap.is_relative()
                && bundle
                    .manifest
                    .roadmap
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "bundle roadmap must stay inside destination"
        );
        ensure!(
            bundle.manifest.roadmap != Path::new("ap.project.json")
                && !bundle.manifest.roadmap.starts_with(".agent-progress"),
            "bundle roadmap collides with managed data"
        );
        roadmap(&bundle.roadmap)?;
        ensure!(
            !destination.exists(),
            "import destination must not exist; refusing overwrite"
        );
        fs::create_dir(destination)?;
        crate::recovery::write(
            &destination.join(".ap-importing"),
            crate::recovery::hash(&bytes).as_bytes(),
            false,
        )?;
        let roadmap_path = destination.join(&bundle.manifest.roadmap);
        fs::create_dir_all(roadmap_path.parent().context("roadmap parent missing")?)?;
        crate::recovery::write(&roadmap_path, bundle.roadmap.as_bytes(), false)?;
        crate::recovery::write(
            &destination.join("ap.project.json"),
            &serde_json::to_vec_pretty(&bundle.manifest)?,
            false,
        )?;
        let directory = destination.join(".agent-progress");
        fs::create_dir_all(&directory)?;
        save_state(
            &directory.join(format!("project-{}.json", bundle.manifest.project_id)),
            &bundle.state,
        )?;
        fs::remove_file(destination.join(".ap-importing"))?;
        File::open(destination)?.sync_all()?;
        Self::open(&destination.join("ap.project.json"))
    }

    pub fn resume(&self) -> Result<serde_json::Value> {
        let plan = self.plan()?;
        let task = |t: &Task| serde_json::json!({"id":t.id,"title":t.title,"status":t.status,"blocker":t.blocker,"next_action":t.next_action,"evidence":t.evidence});
        Ok(
            serde_json::json!({"goal":plan.goal,"counts":plan.counts(),"last_completed":plan.last_completed().map(task),"blocked":plan.tasks.iter().filter(|t| t.status == Status::Blocked).map(task).collect::<Vec<_>>(),"next":plan.next_executable().map(task),"changes":plan.history.iter().rev().take(5).collect::<Vec<_>>(),"state":self.state_path()}),
        )
    }

    pub fn plan(&self) -> Result<Plan> {
        self.config()?;
        let state = decode_state(&read_limited(&self.state_path())?)?;
        ensure!(
            matches!(state.schema, 1 | 2) && state.plan.id == self.project_id,
            "product state identity mismatch"
        );
        state.plan.validate()?;
        Ok(state.plan)
    }

    pub fn code_revision(&self, key: &str) -> Result<Option<String>> {
        let (manifest, _) = self.config()?;
        let Some(policy) = manifest.tasks.get(key) else {
            return Ok(None);
        };
        if policy.revision_paths.is_empty() {
            return Ok(None);
        }
        let mut hash = sha1_smol::Sha1::new();
        let mut paths = policy.revision_paths.clone();
        paths.sort();
        paths.dedup();
        for relative in paths {
            ensure!(
                relative.is_relative()
                    && relative
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                "revision path must stay inside project"
            );
            let path = self.root.join(&relative);
            hash.update(relative.to_string_lossy().as_bytes());
            hash.update(&[0]);
            if path.exists() {
                ensure!(
                    path.canonicalize()?.starts_with(&self.root),
                    "revision path escapes project"
                );
                hash.update(&read_limited(&path)?);
            } else {
                hash.update(b"missing");
            }
            hash.update(&[0]);
        }
        Ok(Some(hash.digest().to_string()))
    }

    fn change_plan(
        &self,
        actor: &str,
        action: &str,
        reason: &str,
        change: impl FnOnce(&mut Plan, &BTreeMap<String, Uuid>) -> Result<()>,
    ) -> Result<Plan> {
        self.sync()?;
        self.config()?;
        let path = self.state_path();
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("ap-lock"))?;
        lock.try_lock_exclusive()
            .context("product progress is busy")?;
        let before = read_limited(&path)?;
        let mut state = decode_state(&before)?;
        ensure!(
            matches!(state.schema, 1 | 2) && state.plan.id == self.project_id,
            "product state identity mismatch"
        );
        state.plan.validate()?;
        let previous_tasks = serde_json::to_value(&state.plan.tasks)?;
        change(&mut state.plan, &state.keys)?;
        state.plan.hold_dependencies();
        state.plan.record(actor, action, None, reason)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
            .min(i64::MAX as u128) as i64;
        for (i, task) in state.plan.tasks.iter().enumerate() {
            if previous_tasks[i] != serde_json::to_value(task)? {
                state.last_event.insert(task.id, now);
            }
        }
        ensure!(
            read_limited(&path)? == before,
            "product state changed during update"
        );
        save_state(&path, &state)?;
        Ok(state.plan)
    }

    pub fn invalidate(&self, key: &str, actor: &str, reason: &str) -> Result<Plan> {
        self.change_plan(actor, "product:invalidate", reason, |plan, keys| {
            plan.invalidate(*keys.get(key).context("unknown product task ID")?, reason)
        })
    }

    pub fn replace(
        &self,
        originals: &[String],
        replacements: &[String],
        actor: &str,
        reason: &str,
    ) -> Result<Plan> {
        self.change_plan(actor, "product:replace", reason, |plan, keys| {
            let ids = |items: &[String]| -> Result<Vec<Uuid>> {
                items
                    .iter()
                    .map(|key| {
                        keys.get(key).copied().with_context(|| {
                            format!("unknown task: {key}; add its stable ID to roadmap first")
                        })
                    })
                    .collect()
            };
            plan.replace(&ids(originals)?, &ids(replacements)?)
        })
    }

    pub fn note(&self, key: &str, actor: &str, text: &str, next: Option<String>) -> Result<Plan> {
        self.edit(key, actor, text, "product:note", |task| {
            task.notes.push(text.into());
            if let Some(next) = next {
                task.next_action = next;
            }
            Ok(())
        })
    }

    fn edit(
        &self,
        key: &str,
        actor: &str,
        reason: &str,
        action: &str,
        edit: impl FnOnce(&mut Task) -> Result<()>,
    ) -> Result<Plan> {
        self.sync()?;
        let (manifest, _) = self.config()?;
        let path = self.state_path();
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("ap-lock"))?;
        lock.try_lock_exclusive()
            .context("product progress is busy")?;
        let before = read_limited(&path)?;
        let mut state = decode_state(&before)?;
        ensure!(
            matches!(state.schema, 1 | 2) && state.plan.id == self.project_id,
            "product state identity mismatch"
        );
        let id = *state.keys.get(key).context("unknown product task ID")?;
        ensure!(
            state.plan.goal == manifest.objective,
            "product objective changed; migration required"
        );
        let required = *manifest
            .required
            .get(key)
            .unwrap_or(&Verification::Reported);
        let task = state.plan.task_mut(id)?;
        if task.required != required {
            task.required = required;
            if task.status == Status::Done && !task.verified() {
                task.status = Status::Review;
            }
            state.plan.record(
                "manifest",
                "product:verification-policy",
                Some(id),
                "제품 항목의 요구 검증 정책 변경",
            )?;
        }
        edit(state.plan.task_mut(id)?)?;
        state.plan.hold_dependencies();
        state.plan.record(actor, action, Some(id), reason)?;
        state.last_event.insert(
            id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis() as i64,
        );
        ensure!(
            read_limited(&path)? == before,
            "product state changed during update"
        );
        save_state(&path, &state)?;
        Ok(state.plan)
    }

    pub fn status(&self, key: &str, status: Status, actor: &str, reason: &str) -> Result<Plan> {
        self.edit(key, actor, reason, "product:explicit-status", |task| {
            match status {
                Status::Done => report(task, StepState::Done, reason, actor),
                Status::Planned => report(task, StepState::Pending, reason, actor),
                Status::Blocked => {
                    task.status = status;
                    task.blocker = Some(reason.into());
                    task.next_action = "막힌 이유를 해결한 뒤 재개".into();
                }
                _ => task.report(status, actor, reason),
            }
            Ok(())
        })
    }

    pub fn evidence(
        &self,
        key: &str,
        kind: Verification,
        actor: &str,
        reference: &str,
    ) -> Result<Plan> {
        self.evidence_at(key, kind, actor, reference, None)
    }

    pub fn evidence_at(
        &self,
        key: &str,
        kind: Verification,
        actor: &str,
        reference: &str,
        revision: Option<&str>,
    ) -> Result<Plan> {
        let current = self.code_revision(key)?;
        if kind != Verification::Reported && current.is_some() {
            ensure!(
                revision == current.as_deref(),
                "code revision missing or changed; capture product revision before running the check"
            );
        } else if revision.is_some() {
            ensure!(revision == current.as_deref(), "code revision mismatch");
        }
        self.edit(key, actor, reference, "product:evidence", |task| {
            task.evidence.push(Evidence {
                kind,
                actor: actor.into(),
                reference: reference.into(),
                recorded_at: model::now(),
                stale: false,
                code_revision: revision.map(String::from),
            });
            Ok(())
        })
    }

    pub fn sync(&self) -> Result<()> {
        self.project(&Snapshot::empty(
            Uuid::nil(),
            self.root.display().to_string(),
        ))?;
        Ok(())
    }

    /// Load/merge under a process lock. Other readers never overwrite a stale copy.
    pub fn project(&self, source: &Snapshot) -> Result<Snapshot> {
        ensure!(
            source.overall.is_none(),
            "cannot import a product projection as a session plan"
        );
        let (manifest, document) = self.config()?;
        ensure!(
            Path::new(&source.project)
                .canonicalize()?
                .starts_with(&self.root),
            "source session belongs to a different project"
        );
        let text = String::from_utf8(read_limited(&document)?)?;
        let items = roadmap(&text)?;
        let path = self.state_path();
        fs::create_dir_all(path.parent().unwrap())?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path.with_extension("ap-lock"))?;
        lock.try_lock_exclusive()
            .context("product progress is busy")?;
        let mut state: State = if path.exists() {
            decode_state(&read_limited(&path)?).context("invalid product progress; retained")?
        } else {
            let mut plan = Plan::new(
                manifest.project_id.to_string(),
                manifest.objective.clone(),
                "agent-progress",
            )?;
            plan.id = manifest.project_id;
            State {
                schema: 1,
                plan,
                keys: BTreeMap::new(),
                document_states: BTreeMap::new(),
                sessions: BTreeMap::new(),
                last_event: BTreeMap::new(),
                checksum: None,
            }
        };
        ensure!(
            matches!(state.schema, 1 | 2) && state.plan.id == manifest.project_id,
            "product state identity mismatch"
        );
        state.plan.validate()?;
        ensure!(
            state.plan.goal == manifest.objective,
            "product objective changed; explicit migration required"
        );
        let before = serde_json::to_vec(&state)?;
        let mut current = HashSet::new();
        for item in &items {
            current.insert(item.key.clone());
            let id = *state
                .keys
                .entry(item.key.clone())
                .or_insert_with(|| Uuid::new_v5(&manifest.project_id, item.key.as_bytes()));
            if !state.plan.tasks.iter().any(|t| t.id == id) {
                let mut task = Task::new(
                    item.title.clone(),
                    vec![format!("{}: {}", item.key, item.title)],
                    *manifest
                        .required
                        .get(&item.key)
                        .unwrap_or(&Verification::Reported),
                    vec![],
                );
                task.id = id;
                state.plan.tasks.push(task);
                state.plan.record(
                    "agent-progress",
                    "product:add",
                    Some(id),
                    &format!("{} 원본 계획에서 가져옴", item.key),
                )?;
            }
            let task = state.plan.task_mut(id)?;
            let required = *manifest
                .required
                .get(&item.key)
                .unwrap_or(&Verification::Reported);
            if task.required != required {
                task.required = required;
                if task.status == Status::Done && !task.verified() {
                    task.status = Status::Review;
                }
                state.plan.record(
                    "manifest",
                    "product:verification-policy",
                    Some(id),
                    "제품 항목의 요구 검증 정책 변경",
                )?;
            }
            let task = state.plan.task_mut(id)?;
            if task.title != item.title {
                task.title = item.title.clone();
                state.plan.record(
                    "agent-progress",
                    "product:rename",
                    Some(id),
                    &format!("{} 이름 변경, ID 유지", item.key),
                )?;
            }
            if state.document_states.get(&item.key) != Some(&item.state) {
                let was_known = state.document_states.contains_key(&item.key);
                let task = state.plan.task_mut(id)?;
                report(
                    task,
                    item.state,
                    &format!("{}#{}", manifest.roadmap.display(), item.key),
                    "roadmap",
                );
                state.plan.record(
                    "roadmap",
                    "product:status",
                    Some(id),
                    &format!("{} 원본 체크 변경: {:?}", item.key, item.state),
                )?;
                state.document_states.insert(item.key.clone(), item.state);
                if was_known || item.state != StepState::Pending {
                    let at = fs::metadata(&document)?
                        .modified()?
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_millis()
                        .min(i64::MAX as u128) as i64;
                    state.last_event.insert(id, at);
                }
            }
        }
        ensure!(
            manifest
                .required
                .keys()
                .all(|key| state.keys.contains_key(key)),
            "verification policy references an unknown task ID"
        );
        ensure!(
            manifest
                .tasks
                .keys()
                .all(|key| state.keys.contains_key(key)),
            "task policy references an unknown task ID"
        );
        for (key, policy) in &manifest.tasks {
            let id = state.keys[key];
            let deps: Vec<_> = policy
                .depends_on
                .iter()
                .map(|key| {
                    state
                        .keys
                        .get(key)
                        .copied()
                        .context("unknown dependency ID")
                })
                .collect::<Result<_>>()?;
            let task = state.plan.task_mut(id)?;
            let criteria = if policy.criteria.is_empty() {
                task.criteria.clone()
            } else {
                policy.criteria.clone()
            };
            if task.criteria != criteria || task.depends_on != deps {
                state.plan.invalidate(id, "완료 조건/선행 작업 변경")?;
                let task = state.plan.task_mut(id)?;
                task.criteria = criteria;
                task.depends_on = deps;
                state.plan.record(
                    "manifest",
                    "product:criteria",
                    Some(id),
                    "완료 조건/선행 작업 갱신; 기존 근거 재검증",
                )?;
            }
            if let Some(revision) = self.code_revision(key)? {
                let stale = state.plan.task_mut(id)?.evidence.iter().any(|e| {
                    !e.stale
                        && e.kind != Verification::Reported
                        && e.code_revision.as_deref() != Some(&revision)
                });
                if stale {
                    state.plan.invalidate(id, "지정 코드 입력 변경")?;
                    state.plan.record(
                        "agent-progress",
                        "product:freshness",
                        Some(id),
                        "지정 코드 입력 변경; 근거 재검증 필요",
                    )?;
                }
            }
        }
        let order: BTreeMap<_, _> = items
            .iter()
            .enumerate()
            .map(|(i, item)| (state.keys[&item.key], i))
            .collect();
        let previous_order: Vec<_> = state.plan.tasks.iter().map(|t| t.id).collect();
        state
            .plan
            .tasks
            .sort_by_key(|task| order.get(&task.id).copied().unwrap_or(usize::MAX));
        if previous_order != state.plan.tasks.iter().map(|t| t.id).collect::<Vec<_>>() {
            state.plan.record(
                "roadmap",
                "product:reorder",
                None,
                "원본 계획 순서 변경, 작업 ID와 기록 유지",
            )?;
        }
        let signature = Uuid::new_v5(
            &source.session,
            &serde_json::to_vec(&(&source.goal, &source.entries, &source.last_plan_at))?,
        );
        let new_input = state
            .sessions
            .get(&source.session)
            .is_none_or(|s| s.signature != signature);
        let mut unmapped = 0;
        let mut mapped: BTreeMap<String, Vec<(&Entry, bool)>> = BTreeMap::new();
        for entry in source.entries.iter().filter(|e| e.present) {
            if let Some((key, direct)) = mapping(&entry.title, &state) {
                mapped
                    .entry(key.to_owned())
                    .or_default()
                    .push((entry, direct));
            } else {
                unmapped += 1;
            }
        }
        if new_input {
            if let Some(at) = event_time(&source.last_plan_at) {
                for (key, entries) in &mapped {
                    let roots: Vec<_> = entries.iter().filter(|(_, direct)| *direct).collect();
                    if roots.len() > 1 {
                        unmapped += entries.len();
                        continue;
                    }
                    let direct = !roots.is_empty();
                    let status = if let Some((e, _)) = roots.first() {
                        e.state
                    } else if entries.iter().any(|(e, _)| e.state == StepState::Active) {
                        StepState::Active
                    } else if entries.iter().all(|(e, _)| e.state == StepState::Done) {
                        StepState::Done
                    } else {
                        StepState::Pending
                    };
                    let id = state.keys[key];
                    if state.last_event.get(&id).is_some_and(|last| *last >= at) {
                        continue;
                    }
                    let task = state.plan.task_mut(id)?;
                    let old = task.status;
                    // A new session's default pending plan is not an explicit reopen.
                    // Reopen with an active report, a document transition, or product status.
                    if direct
                        && status == StepState::Pending
                        && old == Status::Done
                        && task
                            .session
                            .as_ref()
                            .is_none_or(|s| s.id != source.session.to_string())
                    {
                        continue;
                    }
                    if direct {
                        if task.status == Status::Cancelled {
                            continue;
                        }
                        report(
                            task,
                            status,
                            &format!(
                                "{} · {} · {}",
                                source.source, source.session, source.last_plan_at
                            ),
                            "source",
                        );
                    } else if task.status != Status::Done {
                        task.status = match status {
                            StepState::Active => Status::Active,
                            StepState::Done => Status::Review,
                            StepState::Pending => task.status,
                        };
                        task.next_action =
                            "세부 단계 완료와 제품 항목의 수용 조건 확인을 구분".into();
                    }
                    let changed_session = task
                        .session
                        .as_ref()
                        .is_none_or(|s| s.id != source.session.to_string());
                    task.session = Some(Session {
                        agent: "codex".into(),
                        id: source.session.to_string(),
                    });
                    if task.status != old || direct || changed_session {
                        state.plan.record(
                            "source",
                            "product:source",
                            Some(id),
                            &format!("{key} ← {} 단계 ({})", entries.len(), source.session),
                        )?;
                    }
                    state.last_event.insert(id, at);
                }
            }
            if source.plan_seen || source.goal.is_some() {
                state.sessions.insert(
                    source.session,
                    SessionState {
                        signature,
                        goal: source.goal.clone(),
                        last_plan_at: source.last_plan_at.clone(),
                        plan: source.entries.clone(),
                    },
                );
            }
        }
        state.plan.hold_dependencies();
        state.plan.validate()?;
        let after = serde_json::to_vec(&state)?;
        if before != after || !path.exists() {
            ensure!(
                after.len() <= 16 * 1024 * 1024,
                "product state exceeds 16 MiB"
            );
            save_state(&path, &state)?;
        }
        let mut active = source
            .entries
            .iter()
            .find(|e| e.present && e.state == StepState::Active)
            .cloned();
        let parent_key = active
            .as_ref()
            .and_then(|e| mapping(&e.title, &state).map(|(k, _)| k.to_owned()));
        if parent_key.is_some()
            && let Some(work) = &mut active
            && let Some(rest) = work
                .title
                .strip_prefix('[')
                .and_then(|s| s.split_once(']').map(|(_, rest)| rest))
        {
            work.title = rest.trim().to_owned();
        }
        let missing = state.keys.keys().filter(|k| !current.contains(*k)).count();
        let notice = if unmapped > 0 {
            Some(format!(
                "세부 계획 {unmapped}개는 제품 항목 ID 미연결 · 전체 완료율에 반영하지 않음"
            ))
        } else if missing > 0 {
            Some(format!("원본에서 빠진 제품 항목 {missing}개 보존"))
        } else {
            None
        };
        let mut output = source.clone();
        output.goal = Some(Goal {
            id: manifest.project_id.to_string(),
            objective: manifest.objective,
            status: "product".into(),
        });
        output.entries = state
            .plan
            .tasks
            .iter()
            .filter(|t| t.status != Status::Cancelled)
            .map(|t| {
                let key = state
                    .keys
                    .iter()
                    .find(|(_, id)| **id == t.id)
                    .map(|(k, _)| k.as_str())
                    .unwrap_or("?");
                let suffix = match t.status {
                    Status::Review => " · 검증 대기",
                    Status::Blocked => " · 막힘",
                    _ => "",
                };
                Entry {
                    id: t.id,
                    title: format!("{key} · {}{suffix}", t.title),
                    state: match t.status {
                        Status::Done => StepState::Done,
                        Status::Active => StepState::Active,
                        _ => StepState::Pending,
                    },
                    present: current.contains(key),
                    was_done: t.evidence.iter().any(|e| e.kind == Verification::Reported),
                }
            })
            .collect();
        output.source = format!("{} → 제품 계획", source.source);
        output.overall = Some(ProjectView {
            project_id: manifest.project_id,
            manifest: self.manifest.clone(),
            roadmap: manifest.roadmap,
            plan: state.plan,
            session_goal: source.goal.clone(),
            session_plan: source.entries.clone(),
            current_work: active,
            parent_key,
            notice,
            session_count: state.sessions.len(),
        });
        Ok(output)
    }
}
