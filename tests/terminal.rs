#![cfg(unix)]
use agent_progress::terminal;
use serde_json::json;
use std::fs;

#[test]
fn terminal_slot_uses_exact_identity_and_rejects_a_foreign_owner() {
    let directory = tempfile::Builder::new()
        .prefix("ap-terminal-")
        .tempdir()
        .unwrap();
    let path = directory.path();
    let session = uuid::Uuid::new_v4();
    let rollout = path.join("session.jsonl");
    fs::write(
        &rollout,
        format!(
            "{}\n",
            json!({"type":"session_meta","payload":{"id":session,"cwd":path}})
        ),
    )
    .unwrap();
    fs::write(
        path.join("request.json"),
        serde_json::to_vec(&terminal::Request {
            agent: "codex".into(),
            args: vec!["resume".into()],
            cwd: path.into(),
        })
        .unwrap(),
    )
    .unwrap();
    fs::write(path.join("owner"), std::process::id().to_string()).unwrap();
    terminal::publish(
        path,
        terminal::Selection {
            session,
            rollout: rollout.clone(),
            cwd: path.into(),
            owner: std::process::id(),
        },
    )
    .unwrap();
    assert!(
        terminal::publish(
            path,
            terminal::Selection {
                session: uuid::Uuid::new_v4(),
                rollout: rollout.clone(),
                cwd: path.into(),
                owner: std::process::id()
            }
        )
        .is_err()
    );
    assert!(
        terminal::publish(
            path,
            terminal::Selection {
                session,
                rollout,
                cwd: path.into(),
                owner: 1
            }
        )
        .is_err()
    );
}

#[test]
fn noninteractive_native_commands_never_open_a_terminal_display() {
    for (agent, args) in [
        ("codex", vec!["--model", "test", "exec", "prompt"]),
        ("codex", vec!["--help"]),
        ("claude", vec!["--print", "prompt"]),
        ("opencode", vec!["run", "prompt"]),
    ] {
        assert!(!terminal::interactive_args(
            agent,
            &args.into_iter().map(str::to_owned).collect::<Vec<_>>()
        ));
    }
    assert!(terminal::interactive_args(
        "codex",
        &["--model".into(), "test".into(), "resume".into()]
    ));
}

#[test]
fn codex_native_override_and_remote_modes_do_not_create_an_unobservable_display() {
    for args in [
        vec!["--config", "model=custom"],
        vec!["--no-daemon"],
        vec!["--enable=x"],
        vec!["--remote", "ws://localhost:1234"],
        vec!["--remote"],
    ] {
        assert!(!agent_progress::codex_client::can_observe(
            &args.into_iter().map(str::to_owned).collect::<Vec<_>>()
        ));
    }
    assert!(agent_progress::codex_client::can_observe(&[
        "--remote".into(),
        "unix:///tmp/backend.sock".into(),
        "resume".into()
    ]));
}
