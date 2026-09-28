use agent_progress::{
    live::{Goal, Snapshot, StepState},
    model::Status,
    project::Project,
};
use std::{fs, path::Path};
use tempfile::tempdir;
use uuid::Uuid;

fn setup(root: &Path) -> Project {
    let id = Uuid::new_v4();
    fs::write(root.join("ap.project.json"),serde_json::json!({"schema":1,"project_id":id,"objective":"제품 전체 완성","roadmap":"plan.md"}).to_string()).unwrap();
    fs::write(root.join("plan.md"),"# Product\n- [ ] AP-01 First deliverable\n- [ ] AP-02 Second deliverable\n- [ ] AP-03 Third deliverable\n").unwrap();
    Project::discover(root).unwrap().unwrap()
}
fn source(root: &Path, session: Uuid, at: &str, steps: Vec<(&str, StepState)>) -> Snapshot {
    let mut s = Snapshot::empty(session, root.to_string_lossy().into_owned());
    s.set_goal(
        Goal {
            id: session.to_string(),
            objective: "세션의 임시 작업".into(),
            status: "active".into(),
        },
        at,
    );
    s.apply_plan(
        steps
            .into_iter()
            .map(|(s, state)| (s.into(), state))
            .collect(),
        "Codex native Plan",
        at,
        "",
    )
    .unwrap();
    s
}

#[test]
fn temporary_completed_plan_never_becomes_completed_product() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let s = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T00:00:00Z",
        vec![("temporary UI tweak", StepState::Done)],
    );
    let out = project.project(&s).unwrap();
    assert_eq!(out.counts(), (0, 3));
    assert_eq!(out.goal.unwrap().objective, "제품 전체 완성");
    assert!(out.overall.unwrap().notice.unwrap().contains("미연결"));
}

#[test]
fn root_ids_and_completion_survive_new_session_restart_and_stale_reader() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let a = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![
            ("[AP-01] First deliverable", StepState::Done),
            ("[AP-02] Second deliverable", StepState::Active),
        ],
    );
    let first = project.project(&a).unwrap();
    assert_eq!(first.counts(), (1, 3));
    let b = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T02:00:00Z",
        vec![("[AP-02] Second deliverable", StepState::Done)],
    );
    let reopened = Project::discover(root.path()).unwrap().unwrap();
    let next = reopened.project(&b).unwrap();
    assert_eq!(next.counts(), (2, 3));
    assert_eq!(first.entries[0].id, next.entries[0].id);
    assert_eq!(next.overall.unwrap().session_count, 2);
    assert_eq!(project.project(&a).unwrap().counts(), (2, 3));
    let old = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T00:00:00Z",
        vec![("[AP-01] First deliverable", StepState::Pending)],
    );
    assert_eq!(reopened.project(&old).unwrap().counts(), (2, 3));
}

#[test]
fn child_plan_progress_does_not_certify_parent_acceptance() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let session = Uuid::new_v4();
    let s = source(
        root.path(),
        session,
        "2026-09-25T01:00:00Z",
        vec![
            ("[AP-01/parse] implement", StepState::Done),
            ("[AP-01/test] test", StepState::Active),
        ],
    );
    let out = project.project(&s).unwrap();
    assert_eq!(out.counts(), (0, 3));
    let p = out.overall.unwrap();
    assert_eq!(p.plan.tasks[0].status, Status::Active);
    assert_eq!(p.parent_key.as_deref(), Some("AP-01"));
    assert_eq!(p.current_work.unwrap().title, "test");
    let s = source(
        root.path(),
        session,
        "2026-09-25T02:00:00Z",
        vec![
            ("[AP-01/parse] implement", StepState::Done),
            ("[AP-01/test] test", StepState::Done),
        ],
    );
    let out = project.project(&s).unwrap();
    assert_eq!(out.counts(), (0, 3));
    assert_eq!(out.overall.unwrap().plan.tasks[0].status, Status::Review);
}

#[test]
fn roadmap_rename_keeps_id_and_omission_does_not_shrink_denominator() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let s = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![("AP-01 First deliverable", StepState::Done)],
    );
    let first = project.project(&s).unwrap();
    fs::write(
        root.path().join("plan.md"),
        "- [ ] AP-01 Renamed label\n- [ ] AP-02 Second deliverable\n",
    )
    .unwrap();
    let next = project.project(&s).unwrap();
    assert_eq!(next.counts(), (1, 3));
    assert_eq!(first.entries[0].id, next.entries[0].id);
    assert!(next.entries[0].title.contains("Renamed"));
    assert!(!next.entries[2].present);
}

#[test]
fn duplicate_ids_and_corrupt_progress_preserve_saved_bytes() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let s = source(root.path(), Uuid::new_v4(), "2026-09-25T01:00:00Z", vec![]);
    project.project(&s).unwrap();
    let path = project.state_path();
    let before = fs::read(&path).unwrap();
    fs::write(
        root.path().join("plan.md"),
        "- [ ] AP-01 One\n- [ ] AP-01 Two\n",
    )
    .unwrap();
    assert!(project.project(&s).is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::write(root.path().join("plan.md"), "- [ ] AP-01 One\n").unwrap();
    fs::write(&path, "{partial").unwrap();
    assert!(project.project(&s).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "{partial");
}

#[test]
fn root_requires_its_declared_verification_level_and_foreign_cwd_is_rejected() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let path = root.path().join("ap.project.json");
    let mut m: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    m["required"] = serde_json::json!({"AP-01":"automated"});
    fs::write(path, m.to_string()).unwrap();
    let s = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![("AP-01 First deliverable", StepState::Done)],
    );
    let out = project.project(&s).unwrap();
    assert_eq!(out.counts(), (0, 3));
    assert_eq!(out.overall.unwrap().plan.tasks[0].status, Status::Review);
    let foreign = tempdir().unwrap();
    let s = source(
        foreign.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![],
    );
    assert!(project.project(&s).is_err());
}

#[test]
fn duplicate_root_claims_do_not_choose_a_random_completion() {
    let root = tempdir().unwrap();
    let project = setup(root.path());
    let mut s = source(root.path(), Uuid::new_v4(), "2026-09-25T01:00:00Z", vec![]);
    // Exercise the product boundary even if a producer bypassed the parser's duplicate-ID check.
    s.entries = [
        ("[AP-01] Some partial work", StepState::Done),
        ("[AP-01] More partial work", StepState::Pending),
    ]
    .into_iter()
    .map(|(title, state)| agent_progress::live::Entry {
        id: Uuid::new_v4(),
        title: title.into(),
        state,
        present: true,
        was_done: false,
    })
    .collect();
    assert_eq!(project.project(&s).unwrap().counts(), (0, 3));
}

#[test]
fn new_session_pending_defaults_do_not_reopen_finished_work_but_active_can() {
    let root = tempdir().unwrap();
    let p = setup(root.path());
    let a = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![("AP-01 First deliverable", StepState::Done)],
    );
    p.project(&a).unwrap();
    let id = Uuid::new_v4();
    let b = source(
        root.path(),
        id,
        "2026-09-25T02:00:00Z",
        vec![("AP-01 First deliverable", StepState::Pending)],
    );
    assert_eq!(p.project(&b).unwrap().counts(), (1, 3));
    let b = source(
        root.path(),
        id,
        "2026-09-25T03:00:00Z",
        vec![("AP-01 First deliverable", StepState::Active)],
    );
    assert_eq!(p.project(&b).unwrap().counts(), (0, 3));
}

#[test]
fn verification_policy_can_be_satisfied_explicitly_without_registering_tasks_again() {
    use agent_progress::model::Verification;
    let root = tempdir().unwrap();
    let p = setup(root.path());
    let s = source(root.path(), Uuid::new_v4(), "2026-09-25T01:00:00Z", vec![]);
    p.project(&s).unwrap();
    let path = root.path().join("ap.project.json");
    let mut m: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    m["required"] = serde_json::json!({"AP-01":"automated"});
    fs::write(path, m.to_string()).unwrap();
    p.project(&s).unwrap();
    let pending = p
        .status("AP-01", Status::Done, "codex", "reported completion")
        .unwrap();
    assert_eq!(pending.tasks[0].status, Status::Review);
    p.evidence(
        "AP-01",
        Verification::Automated,
        "test-runner",
        "test-result.json: passed",
    )
    .unwrap();
    let completed = p
        .status("AP-01", Status::Done, "codex", "evidence recorded")
        .unwrap();
    assert_eq!(completed.tasks[0].status, Status::Done);
}

#[test]
fn moving_project_with_its_state_retains_ids_and_completion() {
    let root = tempdir().unwrap();
    let before = root.path().join("before");
    fs::create_dir(&before).unwrap();
    let p = setup(&before);
    let s = source(
        &before,
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![("AP-01 First deliverable", StepState::Done)],
    );
    let out = p.project(&s).unwrap();
    let after = root.path().join("after");
    fs::rename(before, &after).unwrap();
    let next = Project::discover(&after)
        .unwrap()
        .unwrap()
        .project(&source(
            &after,
            Uuid::new_v4(),
            "2026-09-25T02:00:00Z",
            vec![],
        ))
        .unwrap();
    assert_eq!(out.entries[0].id, next.entries[0].id);
    assert_eq!(next.counts(), (1, 3));
}

#[test]
fn source_link_error_never_falls_back_to_temporary_hundred_percent() {
    use agent_progress::{live::Feed, runner::Runner};
    use serde_json::json;
    let root = tempdir().unwrap();
    let p = setup(root.path());
    let source_path = root.path().join("source.jsonl");
    fs::write(&source_path,format!("{}\n{}\n",json!({"type":"session_meta","payload":{"id":Uuid::new_v4(),"cwd":root.path()}}),json!({"type":"event_msg","payload":{"type":"plan_update","plan":[{"step":"temporary","status":"completed"}]}}))).unwrap();
    let mut feed = Feed::open(source_path, None, None).unwrap();
    feed.refresh().unwrap();
    assert_eq!(feed.snapshot.counts(), (1, 1));
    let initial = p.project(&feed.snapshot).unwrap();
    assert_eq!(initial.counts(), (0, 3));
    fs::write(root.path().join("plan.md"), "invalid source").unwrap();
    let mut runner = Runner::start(feed, root.path().join("cache.json"), None, Some(p), initial);
    let update = runner
        .updates
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap();
    assert!(update.error.is_some());
    assert_eq!(update.snapshot.counts(), (0, 3));
    runner.shutdown().unwrap();
}

#[test]
fn code_change_invalidates_automated_evidence_and_holds_dependents() {
    use agent_progress::model::Verification;
    let root = tempdir().unwrap();
    let p = setup(root.path());
    fs::write(root.path().join("code.rs"), "version one").unwrap();
    let path = root.path().join("ap.project.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["required"] = serde_json::json!({"AP-01":"automated"});
    manifest["tasks"] = serde_json::json!({
        "AP-01":{"criteria":["saved safely"],"revision_paths":["code.rs"]},
        "AP-02":{"depends_on":["AP-01"]}
    });
    fs::write(&path, manifest.to_string()).unwrap();
    let s = source(root.path(), Uuid::new_v4(), "2026-09-25T01:00:00Z", vec![]);
    p.project(&s).unwrap();
    let revision = p.code_revision("AP-01").unwrap();
    assert!(
        p.evidence(
            "AP-01",
            Verification::Automated,
            "runner",
            "missing revision"
        )
        .is_err()
    );
    p.evidence_at(
        "AP-01",
        Verification::Automated,
        "runner",
        "passed",
        revision.as_deref(),
    )
    .unwrap();
    p.status("AP-01", Status::Done, "codex", "accept").unwrap();
    p.status("AP-02", Status::Done, "codex", "accept dependent")
        .unwrap();
    fs::write(root.path().join("code.rs"), "version two").unwrap();
    assert!(
        p.evidence_at(
            "AP-01",
            Verification::Automated,
            "runner",
            "outdated run",
            revision.as_deref()
        )
        .is_err()
    );
    let next = p.project(&s).unwrap().overall.unwrap().plan;
    assert_eq!(next.tasks[0].status, Status::Review);
    assert_eq!(next.tasks[1].status, Status::Blocked);
    assert!(next.tasks[0].evidence.iter().all(|e| e.stale));
    let bytes = fs::read(p.state_path()).unwrap();
    p.project(&s).unwrap();
    assert_eq!(
        bytes,
        fs::read(p.state_path()).unwrap(),
        "freshness event must not repeat"
    );
    manifest["tasks"]["AP-01"]["revision_paths"] = serde_json::json!(["../secret"]);
    fs::write(path, manifest.to_string()).unwrap();
    assert!(p.project(&s).is_err());
    assert_eq!(bytes, fs::read(p.state_path()).unwrap());
}

#[test]
fn file_and_live_reports_share_criteria_dependencies_and_evidence_semantics() {
    use agent_progress::{model::Verification, store};
    let root = tempdir().unwrap();
    let p = setup(root.path());
    let path = root.path().join("ap.project.json");
    let mut m: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    m["required"] = serde_json::json!({"AP-01":"automated"});
    fs::write(path, m.to_string()).unwrap();
    let empty = source(root.path(), Uuid::new_v4(), "2026-09-25T01:00:00Z", vec![]);
    let initial = p.project(&empty).unwrap().overall.unwrap().plan;
    let file = root.path().join("shared.md");
    store::create(&file, &initial).unwrap();
    let id = initial.tasks[0].id;
    let file_plan = store::update(&file, None, |plan| {
        plan.task_mut(id)?
            .report(Status::Done, "agent", "completed report");
        plan.record("agent", "report", Some(id), "completed report")
    })
    .unwrap();
    let live = p
        .project(&source(
            root.path(),
            Uuid::new_v4(),
            "2026-09-25T02:00:00Z",
            vec![("AP-01 First deliverable", StepState::Done)],
        ))
        .unwrap()
        .overall
        .unwrap()
        .plan;
    assert_eq!(file_plan.tasks[0].status, live.tasks[0].status);
    assert_eq!(file_plan.counts(), live.counts());
    assert_eq!(file_plan.tasks[0].criteria, live.tasks[0].criteria);
    assert_eq!(file_plan.tasks[0].required, Verification::Automated);
    assert!(!file_plan.tasks[0].verified());
}

#[test]
fn explicit_split_and_merge_keep_lineage_but_never_inherit_completion() {
    let root = tempdir().unwrap();
    let p = setup(root.path());
    let s = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![("AP-01 First deliverable", StepState::Done)],
    );
    let original = p.project(&s).unwrap().entries[0].id;
    let split = p
        .replace(
            &["AP-01".into()],
            &["AP-02".into(), "AP-03".into()],
            "codex",
            "split scope",
        )
        .unwrap();
    assert_eq!(split.tasks[0].status, Status::Cancelled);
    assert_eq!(split.counts(), (0, 2));
    assert_eq!(split.tasks[1].derived_from, vec![original]);
    p.project(&source(
        root.path(),
        Uuid::new_v4(),
        "2099-01-01T00:00:00Z",
        vec![("AP-01 old report", StepState::Done)],
    ))
    .unwrap();
    assert_eq!(p.plan().unwrap().tasks[0].status, Status::Cancelled);
    let merged = p
        .replace(
            &["AP-02".into(), "AP-03".into()],
            &["AP-01".into()],
            "codex",
            "merge scope",
        )
        .unwrap();
    assert_eq!(merged.counts(), (0, 1));
    assert_eq!(merged.tasks[0].derived_from.len(), 2);
    let before = fs::read(p.state_path()).unwrap();
    assert!(
        p.replace(
            &["AP-01".into()],
            &["AP-01".into()],
            "codex",
            "invalid overlap"
        )
        .is_err()
    );
    assert_eq!(before, fs::read(p.state_path()).unwrap());
    p.note(
        "AP-01",
        "codex",
        "decision needed",
        Some("review criteria".into()),
    )
    .unwrap();
    let plan = p.plan().unwrap();
    assert_eq!(plan.tasks[0].notes.last().unwrap(), "decision needed");
    assert_eq!(plan.tasks[0].next_action, "review criteria");
}

#[test]
fn restore_previews_conflicts_and_preserves_corrupt_bytes_and_foreign_identity() {
    let root = tempdir().unwrap();
    let p = setup(root.path());
    let s = source(
        root.path(),
        Uuid::new_v4(),
        "2026-09-25T01:00:00Z",
        vec![("AP-01 First deliverable", StepState::Done)],
    );
    p.project(&s).unwrap();
    let backup = root.path().join("saved.json");
    p.backup(&backup).unwrap();
    assert!(p.backup(&backup).is_err());
    p.note("AP-01", "codex", "checkpoint after backup", None)
        .unwrap();
    fs::write(p.state_path(), b"{partial data").unwrap();
    let preview = p.restore(&backup, None, "codex").unwrap();
    assert_eq!(fs::read(p.state_path()).unwrap(), b"{partial data");
    assert!(p.restore(&backup, Some("wrong"), "codex").is_err());
    let result = p
        .restore(&backup, preview["current_hash"].as_str(), "codex")
        .unwrap();
    assert_eq!(
        fs::read(result["displaced"].as_str().unwrap()).unwrap(),
        b"{partial data"
    );
    assert_eq!(p.plan().unwrap().counts(), (1, 3));
    assert!(p.state_path().with_extension("previous.json").exists());
    let foreign = tempdir().unwrap();
    let other = setup(foreign.path());
    other
        .project(&source(
            foreign.path(),
            Uuid::new_v4(),
            "2026-09-25T01:00:00Z",
            vec![],
        ))
        .unwrap();
    let other_backup = foreign.path().join("backup.json");
    other.backup(&other_backup).unwrap();
    assert!(p.restore(&other_backup, None, "codex").is_err());
    let export = root.path().join("export.json");
    p.export(&export).unwrap();
    let bundle: serde_json::Value = serde_json::from_slice(&fs::read(export).unwrap()).unwrap();
    assert_eq!(
        bundle["manifest"]["project_id"],
        p.plan().unwrap().id.to_string()
    );
    assert_eq!(p.resume().unwrap()["counts"], serde_json::json!([1, 3]));
    let destination = root.path().join("imported");
    let imported = Project::import_bundle(&root.path().join("export.json"), &destination).unwrap();
    assert_eq!(p.plan().unwrap().id, imported.plan().unwrap().id);
    assert_eq!(imported.plan().unwrap().counts(), (1, 3));
    assert!(Project::import_bundle(&root.path().join("export.json"), &destination).is_err());
    let before = fs::read(p.state_path()).unwrap();
    let mut corrupt: serde_json::Value = serde_json::from_slice(&before).unwrap();
    corrupt["plan"]["tasks"][0]["title"] = "silently changed".into();
    fs::write(p.state_path(), corrupt.to_string()).unwrap();
    assert!(
        p.plan().is_err(),
        "valid JSON tampering must not silently pass"
    );
}

#[cfg(unix)]
#[test]
fn product_storage_symlink_is_refused_without_touching_outside_files() {
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let p = setup(root.path());
    std::os::unix::fs::symlink(outside.path(), root.path().join(".agent-progress")).unwrap();
    assert!(p.sync().is_err());
    assert!(p.plan().is_err());
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[test]
fn resume_uses_completion_history_and_prioritizes_active_executable_work() {
    let root = tempdir().unwrap();
    let p = setup(root.path());
    p.sync().unwrap();
    p.status("AP-03", Status::Done, "codex", "first completion")
        .unwrap();
    p.status("AP-01", Status::Done, "codex", "latest completion")
        .unwrap();
    p.note("AP-03", "codex", "later note is not completion", None)
        .unwrap();
    p.status("AP-02", Status::Active, "codex", "continue work")
        .unwrap();
    let summary = p.resume().unwrap();
    assert_eq!(summary["last_completed"]["title"], "First deliverable");
    assert_eq!(summary["next"]["status"], "active");
}
