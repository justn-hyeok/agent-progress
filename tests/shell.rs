#![cfg(unix)]
use agent_progress::shell;
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    process::Command,
};
use tempfile::tempdir;

#[test]
fn public_shell_command_needs_no_plan_file_and_preserves_existing_content() {
    let dir = tempdir().unwrap();
    let rc = dir.path().join(".zshrc");
    fs::write(&rc, "# original\n").unwrap();
    for action in ["install", "status", "remove"] {
        let out = Command::new(env!("CARGO_BIN_EXE_ap"))
            .args(["shell", action, "--rc"])
            .arg(&rc)
            .env("HERDR_ENV", "0")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let result: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(result["installed"], action != "remove");
    }
    assert_eq!(fs::read_to_string(rc).unwrap(), "# original\n");
}

#[test]
fn install_is_idempotent_and_removal_preserves_unrelated_edits_and_permissions() {
    let dir = tempdir().unwrap();
    let rc = dir.path().join(".zshrc");
    let original = b"# private user config\nexport EDITOR=vim";
    fs::write(&rc, original).unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o640)).unwrap();
    let result = shell::manage("install", &rc).unwrap();
    assert_eq!(
        fs::read(result["backup"].as_str().unwrap()).unwrap(),
        original
    );
    let installed = fs::read(&rc).unwrap();
    assert_eq!(shell::manage("install", &rc).unwrap()["changed"], false);
    assert_eq!(fs::read(&rc).unwrap(), installed);
    let mut changed = installed;
    changed.extend(b"\n# user edit after installation\n");
    fs::write(&rc, changed).unwrap();
    shell::manage("remove", &rc).unwrap();
    let mut expected = original.to_vec();
    expected.extend(b"\n# user edit after installation\n");
    assert_eq!(fs::read(&rc).unwrap(), expected);
    assert_eq!(
        fs::metadata(&rc).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(shell::manage("remove", &rc).unwrap()["changed"], false);
}

#[test]
fn edited_managed_blocks_and_symlink_rc_files_are_preserved() {
    let dir = tempdir().unwrap();
    let rc = dir.path().join(".zshrc");
    fs::write(&rc, "# user config\n").unwrap();
    shell::manage("install", &rc).unwrap();
    let edited = fs::read_to_string(&rc)
        .unwrap()
        .replace("--agent codex", "--agent user-change");
    fs::write(&rc, &edited).unwrap();
    assert!(shell::manage("remove", &rc).is_err());
    assert_eq!(fs::read_to_string(&rc).unwrap(), edited);
    let link = dir.path().join("link");
    symlink(&rc, &link).unwrap();
    assert!(shell::manage("install", &link).is_err());
}

fn executable(path: &std::path::Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn real_zsh_routes_arguments_and_native_escape_without_recursion() {
    let dir = tempdir().unwrap();
    let rc = dir.path().join(".zshrc");
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    shell::manage("install", &rc).unwrap();
    executable(
        &bin.join("ap"),
        "#!/usr/bin/python3\nimport sys,json\nprint(json.dumps({'route':'ap','args':sys.argv[1:]}))\n",
    );
    executable(
        &bin.join("codex"),
        "#!/usr/bin/python3\nimport sys,json\nprint(json.dumps({'route':'native','args':sys.argv[1:]}))\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let call = |inside: &str, script: &str, args: &[&str]| {
        let out = Command::new("/bin/zsh")
            .args(["-df", "-c", script, "test"])
            .arg(&rc)
            .args(args)
            .env("PATH", &path)
            .env("HERDR_ENV", inside)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let args = ["resume", "quoted a'b", "literal$HOME"];
    let output = call("1", "source \"$1\"; shift; codex \"$@\"", &args);
    assert_eq!(output["route"], "ap");
    assert_eq!(
        output["args"],
        serde_json::json!([
            "launch",
            "--agent",
            "codex",
            "--",
            "resume",
            "quoted a'b",
            "literal$HOME"
        ])
    );
    assert_eq!(
        call("0", "source \"$1\"; shift; codex \"$@\"", &args)["route"],
        "ap"
    );
    assert_eq!(
        call("1", "source \"$1\"; shift; command codex \"$@\"", &args)["route"],
        "native"
    );
    fs::remove_file(bin.join("ap")).unwrap();
    symlink(env!("CARGO_BIN_EXE_ap"), bin.join("ap")).unwrap();
    for args in [
        &["exec", "prompt"][..],
        &["login"][..],
        &["mcp", "list"][..],
        &["--model", "test-model", "exec", "prompt"][..],
    ] {
        let output = call("1", "source \"$1\"; shift; codex \"$@\"", args);
        assert_eq!(output["route"], "native");
        assert_eq!(output["args"], serde_json::to_value(args).unwrap());
    }
    let existing = call(
        "1",
        "function codex() { command codex \"$@\"; }; source \"$1\"; shift; codex \"$@\"",
        &args,
    );
    assert_eq!(existing["route"], "native");
    assert_eq!(
        call("1", "source \"$1\"; shift; codex \"$@\"", &["--version"])["route"],
        "native"
    );
}

#[test]
fn bash_fish_real_shells_preserve_quoting_escape_and_existing_definitions() {
    for (shell_name, executable_name, script, existing_script) in [
        (
            "bash",
            "/bin/bash",
            "source \"$1\"; shift; codex \"$@\"",
            "codex() { command codex \"$@\"; }; source \"$1\"; shift; codex \"$@\"",
        ),
        (
            "fish",
            "fish",
            "source $argv[1]; set -e argv[1]; codex $argv",
            "function codex; command codex $argv; end; source $argv[1]; set -e argv[1]; codex $argv",
        ),
    ] {
        assert!(
            Command::new(executable_name)
                .arg("--version")
                .output()
                .is_ok(),
            "{shell_name} is required for this test"
        );
        let dir = tempdir().unwrap();
        let rc = dir.path().join("config");
        fs::write(&rc, "# original\n").unwrap();
        shell::manage_for("install", &rc, shell_name).unwrap();
        assert_eq!(
            shell::manage_for("install", &rc, shell_name).unwrap()["changed"],
            false
        );
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).unwrap();
        executable(
            &bin.join("ap"),
            "#!/usr/bin/python3\nimport sys,json\nprint(json.dumps({'route':'ap','args':sys.argv[1:]}))\n",
        );
        executable(
            &bin.join("codex"),
            "#!/usr/bin/python3\nimport sys,json\nprint(json.dumps({'route':'native','args':sys.argv[1:]}))\n",
        );
        let args = ["resume", "space a'b", "literal$HOME", "", "line\nbreak"];
        for (script, expected) in [(script, "ap"), (existing_script, "native")] {
            let mut command = Command::new(executable_name);
            if shell_name == "bash" {
                command.args(["--noprofile", "--norc", "-c", script, "test"]);
            } else {
                command.args(["--no-config", "-c", script]);
            }
            let out = command
                .arg(&rc)
                .args(args)
                .env(
                    "PATH",
                    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()),
                )
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{shell_name}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let result: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(result["route"], expected);
            let received = result["args"].as_array().unwrap();
            assert_eq!(
                &received[received.len() - args.len()..],
                serde_json::to_value(args).unwrap().as_array().unwrap()
            );
        }
        shell::manage_for("remove", &rc, shell_name).unwrap();
        assert_eq!(fs::read_to_string(rc).unwrap(), "# original\n");
    }
}

#[test]
fn legacy_zsh_block_can_be_upgraded_and_wrong_shell_or_edited_blocks_are_refused() {
    let dir = tempdir().unwrap();
    let rc = dir.path().join(".zshrc");
    fs::write(&rc, format!("# original\n{}", shell::BLOCK)).unwrap();
    assert!(shell::manage_for("install", &rc, "bash").is_err());
    shell::manage("install", &rc).unwrap();
    assert!(
        fs::read_to_string(&rc)
            .unwrap()
            .contains(&shell::block("zsh").unwrap())
    );
    shell::manage("remove", &rc).unwrap();
    assert_eq!(fs::read_to_string(rc).unwrap(), "# original\n");
}

#[test]
fn default_bash_setup_covers_login_and_nonlogin_and_fish_creates_its_config_directory() {
    for shell_name in ["bash", "fish"] {
        let dir = tempdir().unwrap();
        for action in ["preview", "install", "remove"] {
            let out = Command::new(env!("CARGO_BIN_EXE_ap"))
                .args(["shell", action, "--shell", shell_name])
                .env("HOME", dir.path())
                .env("XDG_CONFIG_HOME", dir.path().join(".config"))
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            if action == "preview" {
                assert!(!dir.path().join(".bashrc").exists());
                assert!(!dir.path().join(".config").exists());
            }
            if action == "install" && shell_name == "bash" {
                assert!(dir.path().join(".bashrc").exists());
                assert!(dir.path().join(".profile").exists());
            }
            if action == "install" && shell_name == "fish" {
                assert!(dir.path().join(".config/fish/config.fish").exists());
            }
        }
    }
}

#[test]
fn bash_setup_preserves_login_precedence_and_posix_profile_behavior() {
    for name in [".bash_login", ".profile"] {
        let dir = tempdir().unwrap();
        let rc = dir.path().join(name);
        fs::write(&rc, "export AP_EXISTING_PROFILE=retained\n").unwrap();
        for action in ["install", "remove"] {
            let out = Command::new(env!("CARGO_BIN_EXE_ap"))
                .args(["shell", action, "--shell", "bash"])
                .env("HOME", dir.path())
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(!dir.path().join(".bash_profile").exists());
            let login = Command::new("/bin/bash")
                .args(["--login", "-c", "printf '%s' \"$AP_EXISTING_PROFILE\""])
                .env("HOME", dir.path())
                .output()
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&login.stdout), "retained");
            if name == ".profile" {
                let sh = Command::new("/bin/sh")
                    .args([
                        "-c",
                        ". \"$1\"; printf '%s' \"$AP_EXISTING_PROFILE\"",
                        "test",
                    ])
                    .arg(&rc)
                    .output()
                    .unwrap();
                assert!(sh.status.success());
                assert!(sh.stderr.is_empty());
                assert_eq!(sh.stdout, b"retained");
            }
        }
        assert_eq!(
            fs::read_to_string(rc).unwrap(),
            "export AP_EXISTING_PROFILE=retained\n"
        );
    }
}

#[test]
fn bash_removal_finds_old_managed_login_file_after_user_changes_precedence() {
    let dir = tempdir().unwrap();
    let old = dir.path().join(".profile");
    fs::write(&old, "# original\n").unwrap();
    let run = |action: &str| {
        Command::new(env!("CARGO_BIN_EXE_ap"))
            .args(["shell", action, "--shell", "bash"])
            .env("HOME", dir.path())
            .output()
            .unwrap()
    };
    assert!(run("install").status.success());
    let new = dir.path().join(".bash_profile");
    fs::write(&new, "# user added higher precedence\n").unwrap();
    assert!(run("remove").status.success());
    assert_eq!(fs::read_to_string(old).unwrap(), "# original\n");
    assert_eq!(
        fs::read_to_string(new).unwrap(),
        "# user added higher precedence\n"
    );
}

#[test]
fn shell_status_distinguishes_current_legacy_absent_and_edited_without_mutation() {
    let dir = tempdir().unwrap();
    let rc = dir.path().join("rc");
    let absent = shell::inspect_selected("zsh", Some(&rc)).unwrap();
    assert_eq!(absent["status"], "absent");
    assert_eq!(absent["upgrade_required"], false);
    assert!(!rc.exists());
    fs::write(&rc, format!("# user\n{}", shell::BLOCK)).unwrap();
    let before = fs::read(&rc).unwrap();
    let old = shell::manage_for("status", &rc, "zsh").unwrap();
    assert_eq!(old["installed"], true);
    assert_eq!(old["status"], "outdated");
    assert_eq!(old["upgrade_required"], true);
    assert_eq!(old["next_action"], "upgrade");
    assert_eq!(fs::read(&rc).unwrap(), before);
    shell::manage_for("install", &rc, "zsh").unwrap();
    let current = shell::inspect_selected("zsh", Some(&rc)).unwrap();
    assert_eq!(current["status"], "current");
    assert_eq!(current["upgrade_required"], false);
    let edited = fs::read_to_string(&rc)
        .unwrap()
        .replace("--agent codex", "--agent NEVER-PRINT-THIS");
    fs::write(&rc, &edited).unwrap();
    let conflict = shell::inspect_selected("zsh", Some(&rc)).unwrap();
    assert_eq!(conflict["status"], "conflict");
    assert_eq!(conflict["upgrade_required"], false);
    assert_eq!(conflict["next_action"], "inspect");
    assert!(!conflict.to_string().contains("NEVER-PRINT-THIS"));
    assert!(shell::manage_for("install", &rc, "zsh").is_err());
    assert_eq!(fs::read_to_string(&rc).unwrap(), edited);
}

#[test]
fn default_bash_status_and_upgrade_include_previous_owned_login_file() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".profile"), shell::block("bash").unwrap()).unwrap();
    fs::write(
        dir.path().join(".bash_profile"),
        "# user changed precedence\n",
    )
    .unwrap();
    let call = |action: &str| {
        Command::new(env!("CARGO_BIN_EXE_ap"))
            .args(["shell", action, "--shell", "bash"])
            .env("HOME", dir.path())
            .output()
            .unwrap()
    };
    let initial = call("status");
    assert!(initial.status.success());
    let initial: Value = serde_json::from_slice(&initial.stdout).unwrap();
    assert_eq!(initial["status"], "partial");
    assert_eq!(initial["files"].as_array().unwrap().len(), 3);
    assert!(call("install").status.success());
    let installed = call("status");
    assert_eq!(
        serde_json::from_slice::<Value>(&installed.stdout).unwrap()["status"],
        "current"
    );
    assert!(call("remove").status.success());
    let removed = call("status");
    assert_eq!(
        serde_json::from_slice::<Value>(&removed.stdout).unwrap()["status"],
        "absent"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".bash_profile")).unwrap(),
        "# user changed precedence\n"
    );
}

#[test]
fn inactive_bash_login_file_with_unreadable_owned_block_is_conflict() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join(".bashrc"), shell::block("bash").unwrap()).unwrap();
    fs::write(
        dir.path().join(".bash_profile"),
        shell::block("bash").unwrap(),
    )
    .unwrap();
    let mut damaged = shell::block("bash").unwrap().into_bytes();
    damaged.push(0xFF);
    fs::write(dir.path().join(".profile"), &damaged).unwrap();
    let before = fs::read(dir.path().join(".profile")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["shell", "status", "--shell", "bash"])
        .env("HOME", dir.path())
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "conflict");
    assert_eq!(report["next_action"], "inspect");
    assert_eq!(report["files"].as_array().unwrap().len(), 3);
    assert_eq!(fs::read(dir.path().join(".profile")).unwrap(), before);
    let apply = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(["shell", "install", "--shell", "bash"])
        .env("HOME", dir.path())
        .output()
        .unwrap();
    assert!(!apply.status.success());
    assert_eq!(fs::read(dir.path().join(".profile")).unwrap(), before);
}

#[test]
fn shell_setup_waits_for_a_transient_lock_without_overwriting_other_content() {
    use fs2::FileExt;
    let dir = tempdir().unwrap();
    let rc = dir.path().join("rc");
    fs::write(&rc, "# preserved\n").unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join("rc.agent-progress.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        FileExt::unlock(&lock).unwrap();
    });
    shell::manage_for("install", &rc, "zsh").unwrap();
    thread.join().unwrap();
    assert!(fs::read_to_string(rc).unwrap().starts_with("# preserved\n"));
}
