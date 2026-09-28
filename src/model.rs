use anyhow::{Result, bail, ensure};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Planned,
    Active,
    Review,
    Blocked,
    Done,
    Cancelled,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planned => "예정",
            Self::Active => "진행",
            Self::Review => "검증 대기",
            Self::Blocked => "막힘",
            Self::Done => "완료",
            Self::Cancelled => "취소",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Verification {
    Reported,
    Automated,
    Human,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub kind: Verification,
    pub reference: String,
    pub actor: String,
    pub recorded_at: u64,
    pub stale: bool,
    /// Fingerprint of explicitly declared code inputs; absent for legacy records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub agent: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub criteria: Vec<String>,
    pub status: Status,
    pub depends_on: Vec<Uuid>,
    pub required: Verification,
    pub evidence: Vec<Evidence>,
    pub session: Option<Session>,
    pub blocker: Option<String>,
    pub next_action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<Uuid>,
}

impl Task {
    pub fn new(
        title: String,
        criteria: Vec<String>,
        required: Verification,
        depends_on: Vec<Uuid>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            title,
            criteria,
            status: Status::Planned,
            depends_on,
            required,
            evidence: vec![],
            session: None,
            blocker: None,
            next_action: String::new(),
            notes: vec![],
            derived_from: vec![],
        }
    }
    pub fn verified(&self) -> bool {
        self.evidence
            .iter()
            .any(|e| e.kind == self.required && !e.stale)
    }

    pub fn report(&mut self, status: Status, actor: &str, reference: &str) {
        if status == Status::Done {
            self.evidence.push(Evidence {
                kind: Verification::Reported,
                reference: reference.into(),
                actor: actor.into(),
                recorded_at: now(),
                stale: false,
                code_revision: None,
            });
            self.status = if self.verified() {
                Status::Done
            } else {
                Status::Review
            };
        } else {
            if matches!(status, Status::Planned | Status::Active | Status::Cancelled) {
                for evidence in &mut self.evidence {
                    evidence.stale = true;
                }
            }
            self.status = status;
        }
        if status != Status::Blocked {
            self.blocker = None;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub revision: u64,
    pub at: u64,
    pub actor: String,
    pub action: String,
    pub task: Option<Uuid>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema_version: u32,
    pub id: Uuid,
    pub project: String,
    pub goal: String,
    pub created_at: u64,
    pub revision: u64,
    pub tasks: Vec<Task>,
    pub history: Vec<Event>,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn nonempty(value: &str, field: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{field} must not be empty");
    // Escape sequences and newlines in labels must not control the terminal.
    ensure!(
        !value.chars().any(char::is_control),
        "{field} contains control characters"
    );
    Ok(())
}

impl Plan {
    pub fn new(project: String, goal: String, actor: &str) -> Result<Self> {
        let mut plan = Self {
            schema_version: 2,
            id: Uuid::new_v4(),
            project,
            goal,
            created_at: now(),
            revision: 0,
            tasks: vec![],
            history: vec![],
        };
        plan.record(actor, "create", None, "계획 생성")?;
        plan.validate()?;
        Ok(plan)
    }
    pub fn record(
        &mut self,
        actor: &str,
        action: &str,
        task: Option<Uuid>,
        reason: &str,
    ) -> Result<()> {
        nonempty(actor, "actor")?;
        nonempty(reason, "reason")?;
        self.schema_version = 2;
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("revision overflow"))?;
        self.history.push(Event {
            revision: self.revision,
            at: now(),
            actor: actor.into(),
            action: action.into(),
            task,
            reason: reason.into(),
        });
        Ok(())
    }
    pub fn task_mut(&mut self, id: Uuid) -> Result<&mut Task> {
        self.tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("unknown task: {id}"))
    }

    /// Invalidate the task and hold its dependents without discarding any evidence.
    pub fn invalidate(&mut self, id: Uuid, reason: &str) -> Result<()> {
        self.task_mut(id)?;
        nonempty(reason, "invalidation reason")?;
        let mut affected = HashSet::from([id]);
        loop {
            let count = affected.len();
            for task in &self.tasks {
                if task.depends_on.iter().any(|dep| affected.contains(dep)) {
                    affected.insert(task.id);
                }
            }
            if count == affected.len() {
                break;
            }
        }
        for task in &mut self.tasks {
            if affected.contains(&task.id) {
                for e in &mut task.evidence {
                    e.stale = true;
                }
                if matches!(task.status, Status::Done | Status::Active | Status::Review) {
                    task.status = if task.id == id {
                        Status::Review
                    } else {
                        Status::Blocked
                    };
                    task.blocker =
                        (task.id != id).then(|| format!("선행 작업 근거 재검증 필요: {id}"));
                    task.next_action = format!("근거 재검증: {reason}");
                }
            }
        }
        Ok(())
    }

    pub fn hold_dependencies(&mut self) {
        loop {
            let done: HashSet<_> = self
                .tasks
                .iter()
                .filter(|t| t.status == Status::Done)
                .map(|t| t.id)
                .collect();
            let mut changed = false;
            for task in &mut self.tasks {
                if matches!(task.status, Status::Active | Status::Review | Status::Done)
                    && let Some(dep) = task.depends_on.iter().find(|id| !done.contains(id))
                {
                    task.status = Status::Blocked;
                    task.blocker = Some(format!("선행 작업 완료 필요: {dep}"));
                    task.next_action = "선행 작업 검증/완료 후 다시 시작".into();
                    for evidence in &mut task.evidence {
                        evidence.stale = true;
                    }
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Explicit replacement. No completed status or evidence is inherited.
    pub fn replace(&mut self, originals: &[Uuid], replacements: &[Uuid]) -> Result<()> {
        ensure!(
            !originals.is_empty() && !replacements.is_empty(),
            "replacement needs both sides"
        );
        let old: HashSet<_> = originals.iter().copied().collect();
        let new: HashSet<_> = replacements.iter().copied().collect();
        ensure!(
            old.len() == originals.len()
                && new.len() == replacements.len()
                && old.is_disjoint(&new),
            "duplicate/overlapping replacement IDs"
        );
        for id in old.union(&new) {
            self.task_mut(*id)?;
        }
        // Reject a dependency rewrite whose semantics cannot be inferred safely.
        ensure!(
            !self
                .tasks
                .iter()
                .any(|t| !old.contains(&t.id) && t.depends_on.iter().any(|id| old.contains(id))),
            "dependent tasks must be updated explicitly before replacement"
        );
        for id in originals {
            self.invalidate(*id, "계획 분할/병합")?;
            self.task_mut(*id)?
                .report(Status::Cancelled, "plan", "계획 분할/병합");
        }
        for id in replacements {
            self.invalidate(*id, "완료 조건 변경")?;
            let task = self.task_mut(*id)?;
            task.report(Status::Planned, "plan", "새 완료 조건 검증 필요");
            task.derived_from.extend(originals);
            task.derived_from.sort();
            task.derived_from.dedup();
        }
        Ok(())
    }
    pub fn counts(&self) -> (usize, usize) {
        (
            self.tasks
                .iter()
                .filter(|t| t.status == Status::Done)
                .count(),
            self.tasks
                .iter()
                .filter(|t| t.status != Status::Cancelled)
                .count(),
        )
    }

    pub fn last_completed(&self) -> Option<&Task> {
        self.history
            .iter()
            .rev()
            .filter(|e| {
                e.action.contains("status")
                    || e.action.contains("report")
                    || e.action == "product:source"
            })
            .find_map(|e| {
                e.task.and_then(|id| {
                    self.tasks
                        .iter()
                        .find(|t| t.id == id && t.status == Status::Done)
                })
            })
            .or_else(|| {
                self.tasks
                    .iter()
                    .filter(|t| t.status == Status::Done)
                    .max_by_key(|t| t.evidence.iter().map(|e| e.recorded_at).max().unwrap_or(0))
            })
    }

    pub fn next_executable(&self) -> Option<&Task> {
        [Status::Active, Status::Review, Status::Planned]
            .into_iter()
            .find_map(|status| {
                self.tasks.iter().find(|t| {
                    t.status == status
                        && t.depends_on.iter().all(|id| {
                            self.tasks
                                .iter()
                                .any(|d| d.id == *id && d.status == Status::Done)
                        })
                })
            })
    }
    pub fn progress(&self) -> String {
        let (done, total) = self.counts();
        if let Some(percent) = (done * 100).checked_div(total) {
            format!(
                "체크율 {done}/{total} ({}%) · 남음 {} · 제품 완성도/남은 시간 아님",
                percent,
                total - done
            )
        } else {
            "체크율 미정 (대상 0개)".into()
        }
    }
    pub fn set_status(
        &mut self,
        id: Uuid,
        status: Status,
        reason: &str,
        next: Option<String>,
    ) -> Result<()> {
        let task = self.task_mut(id)?;
        task.status = status;
        task.blocker = (status == Status::Blocked).then(|| reason.to_owned());
        if let Some(next) = next {
            task.next_action = next;
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.schema_version, 1 | 2),
            "unsupported schema version {}",
            self.schema_version
        );
        nonempty(&self.project, "project")?;
        nonempty(&self.goal, "goal")?;
        ensure!(
            self.revision > 0 && self.history.len() as u64 == self.revision,
            "history/revision mismatch"
        );
        let ids: HashSet<_> = self.tasks.iter().map(|t| t.id).collect();
        ensure!(ids.len() == self.tasks.len(), "duplicate task IDs");
        for (i, e) in self.history.iter().enumerate() {
            ensure!(e.revision == i as u64 + 1, "history out of order");
            nonempty(&e.actor, "event actor")?;
            nonempty(&e.action, "event action")?;
            nonempty(&e.reason, "event reason")?;
            ensure!(
                e.task.is_none_or(|id| ids.contains(&id)),
                "event references missing task"
            );
        }
        for task in &self.tasks {
            nonempty(&task.title, "task title")?;
            ensure!(
                !task.criteria.is_empty(),
                "task {} needs completion criteria",
                task.id
            );
            for criterion in &task.criteria {
                nonempty(criterion, "criterion")?;
            }
            for note in &task.notes {
                nonempty(note, "task note")?;
            }
            ensure!(
                task.derived_from
                    .iter()
                    .all(|id| ids.contains(id) && *id != task.id),
                "invalid task lineage"
            );
            if !task.next_action.is_empty() {
                nonempty(&task.next_action, "next action")?;
            }
            if let Some(session) = &task.session {
                nonempty(&session.agent, "agent")?;
                nonempty(&session.id, "session")?;
            }
            if task.status == Status::Blocked {
                nonempty(task.blocker.as_deref().unwrap_or(""), "blocker")?;
                nonempty(&task.next_action, "blocked task next action")?;
            } else {
                ensure!(task.blocker.is_none(), "non-blocked task has blocker");
            }
            for e in &task.evidence {
                nonempty(&e.reference, "evidence")?;
                nonempty(&e.actor, "evidence actor")?;
            }
            let mut deps = HashSet::new();
            for dep in &task.depends_on {
                ensure!(
                    ids.contains(dep) && *dep != task.id && deps.insert(dep),
                    "invalid/duplicate dependency for {}",
                    task.id
                );
                if matches!(task.status, Status::Active | Status::Review | Status::Done) {
                    ensure!(
                        self.tasks
                            .iter()
                            .any(|t| t.id == *dep && t.status == Status::Done),
                        "dependency {dep} is not done"
                    );
                }
            }
            if task.status == Status::Done {
                ensure!(
                    task.verified(),
                    "task {} needs fresh {:?} evidence",
                    task.id,
                    task.required
                );
            }
        }
        fn visit(
            plan: &Plan,
            id: Uuid,
            visiting: &mut HashSet<Uuid>,
            visited: &mut HashSet<Uuid>,
        ) -> Result<()> {
            if visited.contains(&id) {
                return Ok(());
            }
            if !visiting.insert(id) {
                bail!("dependency cycle");
            }
            for dep in &plan.tasks.iter().find(|t| t.id == id).unwrap().depends_on {
                visit(plan, *dep, visiting, visited)?;
            }
            visiting.remove(&id);
            visited.insert(id);
            Ok(())
        }
        let mut visited = HashSet::new();
        for task in &self.tasks {
            visit(self, task.id, &mut HashSet::new(), &mut visited)?;
        }
        Ok(())
    }
}
