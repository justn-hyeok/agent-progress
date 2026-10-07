use std::{fs, path::Path, process::Command};

fn ap(home: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_ap"))
        .args(args)
        .env("HOME", home)
        .env_remove("HERDR_ENV")
        .env_remove("TMUX_PANE")
        .env_remove("CODEX_THREAD_ID")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn setup() -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    for dir in [".claude/skills", ".codex/skills", ".agents/skills"] {
        fs::create_dir_all(home.path().join(dir)).unwrap();
    }
    fs::write(
        home.path().join(".claude/CLAUDE.md"),
        "# Mine\n\nKeep this.\n",
    )
    .unwrap();
    home
}

#[test]
fn install_is_complete_idempotent_and_removable() {
    let home = setup();
    let h = home.path();
    ap(h, &["skill", "install"]);
    let shared = h.join(".agents/skills/ap");
    assert!(
        fs::read_to_string(shared.join("SKILL.md"))
            .unwrap()
            .contains("name: ap")
    );
    assert!(shared.join("agents/openai.yaml").is_file());
    for root in [".claude/skills", ".codex/skills"] {
        assert_eq!(fs::read_link(h.join(root).join("ap")).unwrap(), shared);
    }
    assert!(
        !h.join(".config/opencode/skills/ap").exists(),
        "absent harness roots are not created"
    );
    let claude = fs::read_to_string(h.join(".claude/CLAUDE.md")).unwrap();
    assert!(claude.starts_with("# Mine\n\nKeep this.\n"));
    assert_eq!(claude.matches("BEGIN agent-progress").count(), 1);

    let again = ap(h, &["skill", "install"]);
    assert!(
        !again.contains("설치:") && !again.contains("연결:") && !again.contains("지침 추가"),
        "{again}"
    );
    assert_eq!(
        fs::read_to_string(h.join(".claude/CLAUDE.md")).unwrap(),
        claude
    );

    ap(h, &["skill", "remove"]);
    assert!(!shared.exists());
    assert!(fs::symlink_metadata(h.join(".claude/skills/ap")).is_err());
    assert_eq!(
        fs::read_to_string(h.join(".claude/CLAUDE.md")).unwrap(),
        "# Mine\n\nKeep this.\n"
    );
}

#[test]
fn dry_run_changes_nothing() {
    let home = setup();
    let h = home.path();
    let report = ap(h, &["skill", "install", "--dry-run"]);
    assert!(
        report.contains("설치:") && report.contains("지침 추가"),
        "{report}"
    );
    assert!(!h.join(".agents/skills/ap").exists());
    assert_eq!(
        fs::read_to_string(h.join(".claude/CLAUDE.md")).unwrap(),
        "# Mine\n\nKeep this.\n"
    );
}

#[test]
fn foreign_skill_is_left_alone() {
    let home = setup();
    let h = home.path();
    let mine = h.join(".agents/skills/ap");
    fs::create_dir_all(&mine).unwrap();
    fs::write(mine.join("SKILL.md"), "my own ap skill\n").unwrap();
    let report = ap(h, &["skill", "install"]);
    assert!(report.contains("건너뜀"), "{report}");
    assert_eq!(
        fs::read_to_string(mine.join("SKILL.md")).unwrap(),
        "my own ap skill\n"
    );
    ap(h, &["skill", "remove"]);
    assert_eq!(
        fs::read_to_string(mine.join("SKILL.md")).unwrap(),
        "my own ap skill\n"
    );
}

#[test]
fn symlinked_claude_md_is_edited_in_place() {
    let home = setup();
    let h = home.path();
    let real = h.join("stack-CLAUDE.md");
    fs::write(&real, "# Stack\n").unwrap();
    fs::remove_file(h.join(".claude/CLAUDE.md")).unwrap();
    std::os::unix::fs::symlink(&real, h.join(".claude/CLAUDE.md")).unwrap();
    ap(h, &["skill", "install"]);
    assert!(
        fs::symlink_metadata(h.join(".claude/CLAUDE.md"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::read_to_string(&real)
            .unwrap()
            .contains("BEGIN agent-progress")
    );
    ap(h, &["skill", "remove"]);
    assert_eq!(fs::read_to_string(&real).unwrap(), "# Stack\n");
}
