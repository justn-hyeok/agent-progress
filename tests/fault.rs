use agent_progress::{live::Snapshot, project::Project};
use std::{fs, process::Command};
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn abrupt_exit_at_every_product_write_boundary_retains_valid_old_or_new_state() {
    for stage in [
        "product:temp-synced",
        "product:backed-up",
        "product:replaced",
        "product:dir-synced",
    ] {
        let root = tempdir().unwrap();
        fs::write(root.path().join("ap.project.json"),serde_json::json!({"schema":1,"project_id":Uuid::new_v4(),"objective":"fault fixture","roadmap":"plan.md"}).to_string()).unwrap();
        fs::write(root.path().join("plan.md"), "- [ ] AP-01 Work\n").unwrap();
        let p = Project::discover(root.path()).unwrap().unwrap();
        p.project(&Snapshot::empty(
            Uuid::new_v4(),
            root.path().display().to_string(),
        ))
        .unwrap();
        let before = p.plan().unwrap().revision;
        let output = Command::new(env!("CARGO_BIN_EXE_ap"))
            .args(["product", "note", "AP-01", "new durable note"])
            .current_dir(root.path())
            .env("AP_FAULT_STAGE", stage)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(86), "{stage}");
        let after = p.plan().unwrap();
        assert!(after.revision == before || after.revision == before + 1);
        let expected = matches!(stage, "product:replaced" | "product:dir-synced");
        assert_eq!(!after.tasks[0].notes.is_empty(), expected);
        assert_eq!(after.counts(), (0, 1));
    }
}

#[test]
fn abrupt_exit_at_every_file_write_boundary_preserves_a_complete_plan() {
    for stage in ["file:temp-synced", "file:replaced", "file:dir-synced"] {
        let root = tempdir().unwrap();
        let file = root.path().join("plan.md");
        let command = |args: &[&str], fault: Option<&str>| {
            let mut c = Command::new(env!("CARGO_BIN_EXE_ap"));
            c.arg("--file").arg(&file).args(args);
            if let Some(fault) = fault {
                c.env("AP_FAULT_STAGE", fault);
            }
            c.output().unwrap()
        };
        assert!(
            command(&["init", "--project", "fixture", "--goal", "durable"], None)
                .status
                .success()
        );
        let added = command(
            &["add", "one", "--criterion", "saved", "--reason", "fixture"],
            None,
        );
        assert!(added.status.success());
        let id = String::from_utf8(added.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned();
        assert_eq!(
            command(&["note", &id, "durable note"], Some(stage))
                .status
                .code(),
            Some(86)
        );
        let plan = agent_progress::store::read(&file).unwrap();
        assert_eq!(!plan.tasks[0].notes.is_empty(), stage != "file:temp-synced");
        assert_eq!(plan.counts(), (0, 1));
    }
}
