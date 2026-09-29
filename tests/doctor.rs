#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::{TempDir, tempdir};

struct Fixture {
    root: TempDir,
    bin: PathBuf,
    home: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempdir().unwrap();
        let bin = root.path().join("bin");
        let home = root.path().join("home");
        fs::create_dir(&bin).unwrap();
        fs::create_dir(&home).unwrap();
        Self { root, bin, home }
    }
    fn stub(&self, name: &str, body: &str) {
        let path = self.bin.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_ap"));
        command
            .current_dir(self.root.path())
            .env("PATH", &self.bin)
            .env("HOME", &self.home)
            .env("SHELL", "/fixture/unknown")
            .env("HERDR_ENV", "0")
            .env_remove("TMUX")
            .env_remove("AP_TERMINAL_SLOT")
            .env("ZDOTDIR", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("config"))
            .env_remove("HERDR_PANE_ID");
        command
    }
    fn doctor(&self, args: &[&str]) -> (Output, Value) {
        let output = self.command().arg("doctor").args(args).output().unwrap();
        let report = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("doctor: {}", String::from_utf8_lossy(&output.stderr)));
        (output, report)
    }
    fn slot(&self) -> PathBuf {
        let slot = self.root.path().join("ap-terminal-fixture");
        fs::create_dir(&slot).unwrap();
        fs::write(
            slot.join("request.json"),
            json!({"agent":"codex","args":["PRIVATE-REQUEST"],"cwd":self.root.path()}).to_string(),
        )
        .unwrap();
        slot
    }
}
fn check<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("missing {name}: {report}"))
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn optional_tools_and_plain_project_are_healthy_but_selected_missing_cli_fails_strict() {
    let f = Fixture::new();
    let (output, report) = f.doctor(&["--strict"]);
    assert!(output.status.success());
    assert_eq!(report["healthy"], true);
    assert_eq!(report["mutates"], false);
    assert_eq!(report["schema"], 1);
    assert_eq!(check(&report, "product")["status"], "optional");
    assert_eq!(check(&report, "shell")["status"], "skipped");
    assert_eq!(check(&report, "tmux")["status"], "unavailable");
    assert!(!f.root.path().join(".agent-progress").exists());
    let (output, report) = f.doctor(&["--agent", "claude", "--strict"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report["healthy"], false);
    assert_eq!(check(&report, "claude")["required"], true);
    assert!(f.doctor(&["--agent", "claude"]).0.status.success());
    f.stub(
        "claude",
        "[ \"$1\" = --version ] || exit 90\nprintf 'fixture-claude 1\\n'",
    );
    f.stub(
        "tmux",
        "[ \"$1\" = -V ] || exit 90\nprintf 'tmux fixture\\n'",
    );
    let (_, report) = f.doctor(&["--agent", "claude", "--strict"]);
    assert_eq!(report["healthy"], true);
    assert_eq!(check(&report, "tmux")["version"], "tmux fixture");
}

#[test]
fn doctor_connection_state_matches_preview_and_never_returns_private_settings() {
    let f = Fixture::new();
    f.stub("codex", "printf 'fixture-codex\\n'");
    let (_, report) = f.doctor(&["--agent", "codex"]);
    assert_eq!(check(&report, "connection:codex")["status"], "unmanaged");
    assert!(
        f.command()
            .args(["connect", "apply", "--agent", "codex"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let (_, report) = f.doctor(&["--agent", "codex"]);
    assert_eq!(check(&report, "connection:codex")["status"], "managed");
    let path = f.root.path().join(".codex/config.toml");
    let changed = fs::read_to_string(&path)
        .unwrap()
        .replace("# END agent-progress managed", "# PRIVATE-CONFIG");
    fs::write(&path, &changed).unwrap();
    let (output, report) = f.doctor(&["--agent", "codex", "--strict"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "connection:codex")["status"], "conflict");
    assert_eq!(check(&report, "connection:codex")["next_action"], "inspect");
    assert!(!report.to_string().contains("PRIVATE-CONFIG"));
    assert!(!report.to_string().contains("managed_addition"));
    assert_eq!(fs::read_to_string(path).unwrap(), changed);
}

#[test]
fn declared_product_without_state_is_not_observed_and_invalid_declarations_fail() {
    let f = Fixture::new();
    let manifest = f.root.path().join("ap.project.json");
    let id = uuid::Uuid::new_v4();
    fs::write(
        &manifest,
        json!({"schema":1,"project_id":id,"objective":"Fixture goal","roadmap":"roadmap.md"})
            .to_string(),
    )
    .unwrap();
    fs::write(
        f.root.path().join("roadmap.md"),
        "- [ ] AP-01 Fixture task\n",
    )
    .unwrap();
    let (output, report) = f.doctor(&["--project", text(&manifest), "--strict"]);
    assert!(output.status.success(), "{report}");
    assert_eq!(check(&report, "product")["status"], "not_observed");
    assert!(!f.root.path().join(".agent-progress").exists());
    let (output, report) = f.doctor(&[
        "--project",
        text(&f.root.path().join("missing.json")),
        "--strict",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "product")["status"], "unreadable");
    fs::create_dir(f.root.path().join(".agent-progress")).unwrap();
    fs::write(
        f.root
            .path()
            .join(format!(".agent-progress/project-{id}.json")),
        "PRIVATE-INVALID-STATE",
    )
    .unwrap();
    let (_, report) = f.doctor(&[]);
    assert_eq!(report["healthy"], false);
    assert_eq!(check(&report, "product")["status"], "unreadable");
    fs::write(&manifest, "PRIVATE-MALFORMED").unwrap();
    let (_, report) = f.doctor(&[]);
    assert_eq!(report["healthy"], false);
    assert!(!report.to_string().contains("PRIVATE"));
}

#[test]
fn declared_but_broken_roadmaps_are_unhealthy_before_cached_state_exists() {
    for as_directory in [true, false] {
        let f = Fixture::new();
        let manifest = f.root.path().join("ap.project.json");
        fs::write(&manifest,json!({"schema":1,"project_id":uuid::Uuid::new_v4(),"objective":"Fixture goal","roadmap":"roadmap.md"}).to_string()).unwrap();
        let roadmap = f.root.path().join("roadmap.md");
        if as_directory {
            fs::create_dir(&roadmap).unwrap();
        } else {
            fs::write(&roadmap, "- [ ] AP-01 First\n- [ ] AP-01 Duplicate\n").unwrap();
        }
        let (output, report) = f.doctor(&["--project", text(&manifest), "--strict"]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(report["healthy"], false);
        assert_eq!(check(&report, "product")["status"], "unreadable");
        assert!(!f.root.path().join(".agent-progress").exists());
    }
}

#[test]
fn explicit_shell_requires_current_while_automatic_absence_and_partial_are_informational() {
    let f = Fixture::new();
    let rc = f.home.join(".zshrc");
    fs::write(&rc, "# PRIVATE-RC\n").unwrap();
    let (output, report) = f.doctor(&["--shell", "zsh", "--rc", text(&rc), "--strict"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "shell")["status"], "absent");
    let output = f
        .command()
        .args(["doctor", "--strict"])
        .env("SHELL", "/bin/zsh")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        f.command()
            .args(["shell", "install", "--shell", "zsh", "--rc", text(&rc)])
            .output()
            .unwrap()
            .status
            .success()
    );
    let (output, report) = f.doctor(&["--shell", "zsh", "--rc", text(&rc), "--strict"]);
    assert!(output.status.success(), "{report}");
    assert_eq!(check(&report, "shell")["status"], "current");
    assert!(!report.to_string().contains("PRIVATE-RC"));
    assert!(!report.to_string().contains("managed_addition"));
    let (output, report) = f.doctor(&["--rc", text(&rc), "--strict"]);
    assert!(output.status.success(), "{report}");
    assert_eq!(check(&report, "shell")["shell"], "zsh");
    fs::write(&rc, agent_progress::shell::BLOCK).unwrap();
    let output = f
        .command()
        .args(["doctor", "--strict"])
        .env("SHELL", "/bin/zsh")
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "shell")["status"], "outdated");
    assert_eq!(check(&report, "shell")["upgrade_required"], true);
    fs::write(
        f.home.join(".bashrc"),
        agent_progress::shell::block("bash").unwrap(),
    )
    .unwrap();
    let output = f
        .command()
        .args(["doctor", "--strict"])
        .env("SHELL", "/bin/bash")
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(output.status.success(), "{report}");
    assert_eq!(check(&report, "shell")["status"], "partial");
    fs::write(&rc, "# BEGIN agent-progress shell\nPRIVATE-EDIT\n").unwrap();
    let output = f
        .command()
        .args(["doctor", "--strict"])
        .env("SHELL", "/bin/zsh")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn exact_terminal_slot_checks_waiting_liveness_owner_and_session_without_leaking_request() {
    let f = Fixture::new();
    f.stub("kill", "[ \"$1\" = -0 ] && [ \"$2\" = 4242 ]");
    let slot = f.slot();
    let args = ["--terminal-slot", text(&slot), "--strict"];
    let (output, report) = f.doctor(&args);
    assert!(output.status.success());
    assert_eq!(check(&report, "terminal-slot")["status"], "waiting");
    fs::write(slot.join("owner"), "4243").unwrap();
    let (output, report) = f.doctor(&args);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "terminal-slot")["status"], "stale");
    fs::write(slot.join("owner"), "4242").unwrap();
    let rollout = f.root.path().join("source.jsonl");
    let session = uuid::Uuid::new_v4();
    fs::write(
        &rollout,
        format!(
            "{}\n",
            json!({"type":"session_meta","payload":{"id":session,"cwd":f.root.path()}})
        ),
    )
    .unwrap();
    let selection = json!({"owner":4242,"session":session,"rollout":rollout,"cwd":f.root.path()});
    fs::write(slot.join("selection.json"), selection.to_string()).unwrap();
    let (output, report) = f.doctor(&args);
    assert!(output.status.success(), "{report}");
    assert_eq!(check(&report, "terminal-slot")["status"], "connected");
    assert!(!report.to_string().contains("PRIVATE-REQUEST"));
    for change in [
        json!({"owner":4243}),
        json!({"session":uuid::Uuid::new_v4()}),
    ] {
        let mut broken = selection.clone();
        for (key, value) in change.as_object().unwrap() {
            broken[key] = value.clone();
        }
        fs::write(slot.join("selection.json"), broken.to_string()).unwrap();
        let (output, report) = f.doctor(&args);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(check(&report, "terminal-slot")["status"], "conflict");
    }
    fs::remove_file(slot.join("owner")).unwrap();
    symlink(slot.join("missing"), slot.join("owner")).unwrap();
    assert_eq!(
        check(&f.doctor(&args).1, "terminal-slot")["status"],
        "conflict"
    );
    assert!(slot.join("owner").is_symlink());
    fs::remove_file(slot.join("owner")).unwrap();
    fs::remove_file(slot.join("selection.json")).unwrap();
    for request in [
        json!({"agent":"unsupported","args":[],"cwd":f.root.path()}),
        json!({"agent":"codex","args":[],"cwd":"relative"}),
    ] {
        fs::write(slot.join("request.json"), request.to_string()).unwrap();
        assert_eq!(
            check(&f.doctor(&args).1, "terminal-slot")["status"],
            "conflict"
        );
    }
}

#[test]
fn herdr_environment_does_not_query_inventory_and_explicit_sources_fail_without_fallback() {
    let f = Fixture::new();
    let marker = f.root.path().join("herdr-called");
    f.stub(
        "herdr",
        &format!("printf called > '{}'\nexit 1", marker.display()),
    );
    let output = f
        .command()
        .args(["doctor", "--strict"])
        .env("HERDR_ENV", "1")
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(output.status.success());
    assert_eq!(check(&report, "terminal")["backend"], "herdr");
    assert!(!marker.exists());
    let output = f
        .command()
        .arg("doctor")
        .env("TMUX", "fixture-socket")
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(check(&report, "terminal")["backend"], "tmux");
    let (output, report) = f.doctor(&[
        "--rollout",
        text(&f.root.path().join("missing.jsonl")),
        "--strict",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "source")["status"], "unreadable");
    let (output, report) = f.doctor(&["--pane", "fixture-pane", "--strict"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(check(&report, "pane")["status"], "disconnected");
}
