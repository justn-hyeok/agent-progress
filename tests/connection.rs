use agent_progress::connection;
use std::fs;
use tempfile::tempdir;

#[test]
fn connection_setup_waits_for_short_lock_contention_before_applying() {
    use fs2::FileExt;
    for agent in ["codex", "claude", "opencode"] {
        let root = tempdir().unwrap();
        let storage = root.path().join(".agent-progress");
        fs::create_dir(&storage).unwrap();
        let name = if agent == "codex" {
            "connection.ap-lock".to_owned()
        } else {
            format!("connection-{agent}.lock")
        };
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(storage.join(name))
            .unwrap();
        lock.lock_exclusive().unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(120));
            FileExt::unlock(&lock).unwrap();
        });
        let started = std::time::Instant::now();
        connection::manage_agent(root.path(), "apply", false, agent).unwrap();
        release.join().unwrap();
        assert!(started.elapsed() >= std::time::Duration::from_millis(100));
        assert_eq!(
            connection::manage_agent(root.path(), "preview", false, agent).unwrap()["connection_status"],
            "managed"
        );
    }
}

fn run_bridge(
    root: &std::path::Path,
    agent: &str,
    event: serde_json::Value,
) -> std::process::Output {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["bridge", "--agent", agent, "--root"])
        .arg(root)
        .env("HERDR_ENV", "0")
        .env_remove("AP_TERMINAL_SLOT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(event.to_string().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn ordinary_projects_connect_and_receive_native_plans_without_creating_a_product() {
    use serde_json::json;
    for agent in ["codex", "claude", "opencode"] {
        let root = tempdir().unwrap();
        let preview = connection::manage_agent(root.path(), "preview", false, agent).unwrap();
        assert_eq!(preview["connection_status"], "unmanaged");
        assert_eq!(preview["next_action"], "apply");
        assert!(!root.path().join(".agent-progress").exists());
        if agent == "codex" {
            assert_eq!(preview["product_connected"], false);
            assert!(
                !preview["files"][0]["managed_addition"]
                    .as_str()
                    .unwrap()
                    .contains("mcp_servers")
            );
            assert!(connection::manage(root.path(), "apply", true).is_err());
        }
        connection::manage_agent(root.path(), "apply", false, agent).unwrap();
        let managed = connection::manage_agent(root.path(), "preview", false, agent).unwrap();
        assert_eq!(managed["connection_status"], "managed");
        assert_eq!(managed["next_action"], "none");
        assert!(connection::manage_agent(root.path(), "apply", false, agent).is_err());
        let native = uuid::Uuid::new_v4();
        let rollout = root.path().join("source.jsonl");
        fs::write(&rollout,format!("{}\n{}\n",json!({"type":"session_meta","payload":{"id":native,"cwd":root.path()}}),json!({"type":"event_msg","payload":{"type":"plan_update","plan":[{"step":"Standalone task","status":"completed"}]}}))).unwrap();
        let event = if agent == "codex" {
            json!({"cwd":root.path(),"session_id":"native","transcript_path":rollout})
        } else {
            json!({"cwd":root.path(),"session_id":"native","todos":[{"content":"Standalone task","status":"completed"}]})
        };
        let output = run_bridge(root.path(), agent, event);
        assert!(
            output.status.success(),
            "{}: {}",
            agent,
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["continue"],
            true
        );
        if agent != "codex" {
            let session = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("agent-progress:{agent}:native").as_bytes(),
            );
            let path = root
                .path()
                .join(format!(".agent-progress/bridges/{agent}-{session}.jsonl"));
            let mut feed = agent_progress::live::Feed::open(path, Some(session), None).unwrap();
            feed.refresh_all().unwrap();
            assert_eq!(feed.snapshot.counts(), (1, 1));
        }
        assert!(!root.path().join("ap.project.json").exists());
        assert!(
            !fs::read_dir(root.path().join(".agent-progress"))
                .unwrap()
                .any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("project-"))
        );
        connection::manage_agent(root.path(), "remove", false, agent).unwrap();
        assert_eq!(
            connection::manage_agent(root.path(), "preview", false, agent).unwrap()["connection_status"],
            "unmanaged"
        );
    }
}

#[test]
fn nested_manifest_free_connection_cannot_project_into_a_parent_product() {
    use serde_json::json;
    for agent in ["codex", "claude", "opencode"] {
        let parent = tempdir().unwrap();
        let child = parent.path().join("ordinary");
        fs::create_dir(&child).unwrap();
        let product_id = uuid::Uuid::new_v4();
        fs::write(parent.path().join("ap.project.json"),json!({"schema":1,"project_id":product_id,"objective":"Parent product","roadmap":"plan.md"}).to_string()).unwrap();
        fs::write(parent.path().join("plan.md"), "- [ ] AP-01 Parent task\n").unwrap();
        assert!(
            agent_progress::project::Project::discover(&child)
                .unwrap()
                .is_some()
        );
        connection::manage_agent(&child, "apply", false, agent).unwrap();
        assert!(
            agent_progress::project::Project::discover(&child)
                .unwrap()
                .is_none()
        );
        let native = uuid::Uuid::new_v4();
        let transcript = child.join("source.jsonl");
        if agent == "codex" {
            fs::write(&transcript,format!("{}\n{}\n",json!({"type":"session_meta","payload":{"id":native,"cwd":child}}),json!({"type":"event_msg","payload":{"type":"plan_update","plan":[{"step":"AP-01 Parent task","status":"completed"}]}}))).unwrap();
        }
        let event = if agent == "codex" {
            json!({"cwd":child,"session_id":"native","transcript_path":transcript})
        } else {
            json!({"cwd":child,"session_id":"native","todos":[{"content":"AP-01 Parent task","status":"completed"}]})
        };
        assert!(run_bridge(&child, agent, event).status.success());
        assert!(child.join(".agent-progress/passive-root").is_file());
        let stream = if agent == "codex" {
            transcript
        } else {
            let session = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("agent-progress:{agent}:native").as_bytes(),
            );
            child.join(format!(".agent-progress/bridges/{agent}-{session}.jsonl"))
        };
        for phase in ["connected", "removed"] {
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_ap"))
                .args(["follow", "--rollout"])
                .arg(&stream)
                .args(["--once", "--codex-home"])
                .arg(&child)
                .current_dir(&child)
                .env("HERDR_ENV", "0")
                .env_remove("AP_TERMINAL_SLOT")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{agent}/{phase}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(snapshot["overall"].is_null(), "{agent}/{phase}");
            assert!(
                !parent
                    .path()
                    .join(format!(".agent-progress/project-{product_id}.json"))
                    .exists()
            );
            if phase == "connected" {
                connection::manage_agent(&child, "remove", false, agent).unwrap();
            }
        }
        let sibling = parent.path().join("unconfigured");
        fs::create_dir(&sibling).unwrap();
        assert_eq!(
            agent_progress::project::Project::discover(&sibling)
                .unwrap()
                .unwrap()
                .root(),
            parent.path().canonicalize().unwrap()
        );
    }
}

#[test]
fn concurrent_first_native_hooks_keep_the_passive_boundary_and_all_events() {
    use serde_json::json;
    use std::sync::{Arc, Barrier};
    for round in 0..3 {
        let parent = tempdir().unwrap();
        let child = parent.path().join(format!("child-{round}"));
        fs::create_dir(&child).unwrap();
        fs::create_dir_all(child.join(".agent-progress/bridges")).unwrap();
        fs::write(parent.path().join("ap.project.json"),json!({"schema":1,"project_id":uuid::Uuid::new_v4(),"objective":"Parent","roadmap":"plan.md"}).to_string()).unwrap();
        fs::write(parent.path().join("plan.md"), "- [ ] AP-01 Parent task\n").unwrap();
        let gate = Arc::new(Barrier::new(8));
        std::thread::scope(|scope| {
            let tasks=(0..8).map(|i|{
                let gate=gate.clone();let child=child.clone();
                scope.spawn(move || {
                    let event=json!({"cwd":child,"session_id":format!("parallel-{i}"),"todos":[{"content":"AP-01 Parent task","status":"completed"}]});
                    gate.wait();
                    agent_progress::bridge::ingest("claude",&event,Some(&child))
                })
            }).collect::<Vec<_>>();
            for task in tasks {
                assert!(task.join().unwrap().is_ok());
            }
        });
        assert!(
            agent_progress::project::Project::discover(&child)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fs::read_dir(child.join(".agent-progress/bridges"))
                .unwrap()
                .filter(|e| e
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".jsonl"))
                .count(),
            8
        );
        assert!(!parent.path().join(".agent-progress").exists());
    }
}

#[test]
fn malformed_manifests_and_foreign_native_events_are_not_hidden_as_passive_projects() {
    use serde_json::json;
    let root = tempdir().unwrap();
    fs::write(root.path().join("ap.project.json"), "{").unwrap();
    for agent in ["codex", "claude", "opencode"] {
        assert!(connection::manage_agent(root.path(), "preview", false, agent).is_err());
        assert!(
            !run_bridge(
                root.path(),
                agent,
                json!({"cwd":root.path(),"session_id":"s"})
            )
            .status
            .success()
        );
    }
    assert!(!root.path().join(".agent-progress").exists());
    let plain = tempdir().unwrap();
    let foreign = tempdir().unwrap();
    let event = json!({"cwd":foreign.path(),"session_id":"wrong-root","todos":[{"content":"Foreign","status":"completed"}]});
    assert!(!run_bridge(plain.path(), "claude", event).status.success());
    assert!(!plain.path().join(".agent-progress").exists());
}

#[test]
fn connection_preview_is_read_only_and_conflict_output_never_contains_user_config() {
    for agent in ["codex", "claude", "opencode"] {
        let root = tempdir().unwrap();
        connection::manage_agent(root.path(), "apply", false, agent).unwrap();
        let (relative, receipt) = match agent {
            "codex" => (".codex/config.toml", "connection.json"),
            "claude" => (".claude/settings.local.json", "connection-claude.json"),
            _ => (
                ".opencode/plugins/agent-progress.js",
                "connection-opencode.json",
            ),
        };
        let path = root.path().join(relative);
        let original = fs::read(&path).unwrap();
        let receipt_path = root.path().join(".agent-progress").join(receipt);
        let receipt_before = fs::read(&receipt_path).unwrap();
        let changed = if agent == "claude" {
            let mut v: serde_json::Value = serde_json::from_slice(&original).unwrap();
            v["hooks"]["Stop"] = serde_json::json!([]);
            v["private_user_value"] = serde_json::json!("NEVER-PRINT-THIS");
            serde_json::to_vec(&v).unwrap()
        } else {
            let mut v = original;
            v.extend(b"\nNEVER-PRINT-THIS\n");
            if agent == "codex" {
                String::from_utf8(v)
                    .unwrap()
                    .replace("# END agent-progress managed", "# EDITED managed end")
                    .into_bytes()
            } else {
                v
            }
        };
        fs::write(&path, &changed).unwrap();
        let preview = connection::manage_agent(root.path(), "preview", false, agent).unwrap();
        assert_eq!(preview["connection_status"], "conflict");
        assert_eq!(preview["next_action"], "inspect");
        assert!(!preview.to_string().contains("NEVER-PRINT-THIS"));
        assert_eq!(fs::read(&path).unwrap(), changed);
        assert_eq!(fs::read(receipt_path).unwrap(), receipt_before);
    }
}

#[cfg(unix)]
#[test]
fn launcher_preserves_explicit_auto_open_opt_out_arguments_and_native_exit_status() {
    use std::{os::unix::fs::PermissionsExt, process::Command};
    let root = tempdir().unwrap();
    for agent in ["codex", "claude", "opencode"] {
        let script = root.path().join(agent);
        fs::write(&script,"#!/usr/bin/python3\nimport os,json,sys\nprint(json.dumps({'auto_open':os.environ.get('AP_AUTO_OPEN'),'args':sys.argv[1:]}))\nsys.exit(7)\n").unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o755)).unwrap();
        for (opt_out, first_arg) in [
            (true, "--version"),
            (true, "resume"),
            (false, "--version"),
            (false, "resume"),
        ] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_ap"));
            command
                .args([
                    "launch",
                    "--agent",
                    agent,
                    "--",
                    first_arg,
                    "space a'b",
                    "literal$HOME",
                ])
                .env("PATH", format!("{}:/usr/bin:/bin", root.path().display()))
                .env("HERDR_ENV", "0")
                .env_remove("AP_TERMINAL_SLOT");
            if opt_out {
                command.env("AP_AUTO_OPEN", "0");
            } else {
                command.env_remove("AP_AUTO_OPEN");
            }
            let output = command.output().unwrap();
            assert_eq!(output.status.code(), Some(7));
            let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(value["auto_open"], if opt_out { "0" } else { "1" });
            assert_eq!(
                value["args"],
                serde_json::json!([first_arg, "space a'b", "literal$HOME"])
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn preview_reports_unsafe_paths_as_conflicts_without_reading_or_changing_targets() {
    use std::os::unix::fs::symlink;
    for (agent, relative) in [
        ("codex", ".codex"),
        ("claude", ".claude"),
        ("opencode", ".opencode"),
    ] {
        let root = tempdir().unwrap();
        let other = tempdir().unwrap();
        fs::write(other.path().join("private"), "NEVER-PRINT-THIS").unwrap();
        symlink(other.path(), root.path().join(relative)).unwrap();
        let preview = connection::manage_agent(root.path(), "preview", false, agent).unwrap();
        assert_eq!(preview["connection_status"], "conflict");
        assert_eq!(preview["next_action"], "inspect");
        assert!(!preview.to_string().contains("NEVER-PRINT-THIS"));
        assert!(!root.path().join(".agent-progress").exists());
        assert_eq!(
            fs::read_to_string(other.path().join("private")).unwrap(),
            "NEVER-PRINT-THIS"
        );
        assert!(connection::manage_agent(root.path(), "apply", false, agent).is_err());
    }
}

#[cfg(unix)]
#[test]
fn declaration_symlinks_cannot_make_a_connection_write_into_another_product_root() {
    use std::os::unix::fs::symlink;
    let root = tempdir().unwrap();
    let other = tempdir().unwrap();
    let declaration = other.path().join("ap.project.json");
    fs::write(&declaration,serde_json::json!({"schema":1,"project_id":uuid::Uuid::new_v4(),"objective":"Foreign product","roadmap":"plan.md"}).to_string()).unwrap();
    fs::write(other.path().join("plan.md"), "- [ ] AP-01 Foreign\n").unwrap();
    symlink(&declaration, root.path().join("ap.project.json")).unwrap();
    for agent in ["codex", "claude", "opencode"] {
        assert!(connection::manage_agent(root.path(), "apply", false, agent).is_err());
        assert!(!run_bridge(root.path(),agent,serde_json::json!({"cwd":root.path(),"session_id":"s","todos":[{"content":"AP-01 Foreign","status":"completed"}]})).status.success());
    }
    assert!(!root.path().join(".agent-progress").exists());
    assert!(!other.path().join(".agent-progress").exists());
}

#[cfg(unix)]
#[test]
fn automatic_root_discovery_does_not_treat_a_broken_declaration_as_a_plain_project() {
    use std::{
        io::Write,
        os::unix::fs::symlink,
        process::{Command, Stdio},
    };
    let root = tempdir().unwrap();
    symlink(
        root.path().join("missing.json"),
        root.path().join("ap.project.json"),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["bridge", "--agent", "claude"])
        .current_dir(root.path())
        .env("HERDR_ENV", "0")
        .env_remove("AP_TERMINAL_SLOT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(serde_json::json!({"cwd":root.path(),"session_id":"broken-declaration","todos":[{"content":"Task","status":"completed"}]}).to_string().as_bytes()).unwrap();
    assert!(!child.wait_with_output().unwrap().status.success());
    assert!(!root.path().join(".agent-progress").exists());
}

#[test]
fn optional_integration_preserves_user_text_and_remove_is_narrow() {
    let root = tempdir().unwrap();
    fs::write(root.path().join("ap.project.json"),serde_json::json!({"schema":1,"project_id":uuid::Uuid::new_v4(),"objective":"fixture","roadmap":"plan.md"}).to_string()).unwrap();
    fs::write(
        root.path().join("AGENTS.md"),
        "User instructions without trailing newline",
    )
    .unwrap();
    fs::create_dir(root.path().join(".codex")).unwrap();
    let config = root.path().join(".codex/config.toml");
    fs::write(&config, "model = \"existing\"\n").unwrap();
    connection::manage(root.path(), "preview", false).unwrap();
    assert!(!root.path().join(".agent-progress").exists());
    connection::manage(root.path(), "apply", false).unwrap();
    assert!(
        fs::read_to_string(&config)
            .unwrap()
            .contains("[mcp_servers.agent_progress]")
    );
    let modified = format!(
        "{}\n# Later user setting\n",
        fs::read_to_string(&config).unwrap()
    );
    fs::write(&config, modified).unwrap();
    assert!(connection::manage(root.path(), "apply", true).is_err());
    connection::manage(root.path(), "remove", false).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("AGENTS.md")).unwrap(),
        "User instructions without trailing newline"
    );
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "model = \"existing\"\n\n# Later user setting\n"
    );
    assert!(
        root.path()
            .join(".agent-progress/connection-backups")
            .is_dir()
    );
    fs::write(
        &config,
        "[mcp_servers.agent_progress]\ncommand = \"user-owned\"\n",
    )
    .unwrap();
    assert!(connection::manage(root.path(), "apply", false).is_err());
    assert!(fs::read_to_string(&config).unwrap().contains("user-owned"));
}
