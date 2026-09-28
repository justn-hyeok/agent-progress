use agent_progress::connection;
use std::fs;
use tempfile::tempdir;

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
