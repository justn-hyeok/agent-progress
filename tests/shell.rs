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
        "native"
    );
    assert_eq!(
        call("1", "source \"$1\"; shift; command codex \"$@\"", &args)["route"],
        "native"
    );
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
    fs::remove_file(bin.join("ap")).unwrap();
    symlink(env!("CARGO_BIN_EXE_ap"), bin.join("ap")).unwrap();
    assert_eq!(
        call("1", "source \"$1\"; shift; codex \"$@\"", &["--version"])["route"],
        "native"
    );
}
