//! Read-only connection ownership checks. Diagnostics never contain settings or paths.
use crate::recovery;
use serde_json::{Value, json};
use std::{fs, io::ErrorKind, path::Path};

const BEGIN: &str = "# BEGIN agent-progress managed\n";
const END: &str = "# END agent-progress managed\n";
const EVENTS: [&str; 3] = ["SessionStart", "PostToolUse", "Stop"];
type Check<T> = Result<T, &'static str>;

pub(crate) fn inspect(root: &Path, agent: &str, desired_hook: Option<&Value>) -> Value {
    let result = inspect_inner(root, agent, desired_hook);
    let (status, action, diagnostic) = match result {
        Ok(true) => ("managed", "none", "owned_configuration_intact"),
        Ok(false) => ("unmanaged", "apply", "no_active_connection"),
        Err(reason) => ("conflict", "inspect", reason),
    };
    json!({"connection_status":status,"next_action":action,"diagnostic":diagnostic})
}

fn check_path(root: &Path, relative: &str, directory: bool) -> Check<()> {
    let path = root.join(relative);
    for ancestor in path.ancestors().take_while(|p| p.starts_with(root)) {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err("symlink_path");
                }
                let expects_dir = ancestor != path || directory;
                if (expects_dir && !meta.is_dir()) || (!expects_dir && !meta.is_file()) {
                    return Err("invalid_path_type");
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err("unreadable_path"),
        }
    }
    Ok(())
}

fn read(root: &Path, relative: &str) -> Check<Option<Vec<u8>>> {
    check_path(root, relative, false)?;
    let path = root.join(relative);
    match fs::symlink_metadata(&path) {
        Ok(_) => recovery::read(&path)
            .map(Some)
            .map_err(|_| "unreadable_file"),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err("unreadable_file"),
    }
}

fn hash_valid(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn parse_receipt(bytes: Option<Vec<u8>>) -> Check<Option<Value>> {
    bytes
        .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| "invalid_receipt"))
        .transpose()
}

fn inspect_inner(root: &Path, agent: &str, desired_hook: Option<&Value>) -> Check<bool> {
    check_path(root, ".agent-progress/connection-backups", true)?;
    match agent {
        "codex" => codex(root),
        "claude" | "opencode" => native(root, agent, desired_hook),
        _ => Err("unsupported_agent"),
    }
}

fn codex(root: &Path) -> Check<bool> {
    let receipt = parse_receipt(read(root, ".agent-progress/connection.json")?)?;
    let paths = ["AGENTS.md", ".codex/config.toml"];
    let mut blocks = [None, None];
    let mut active = false;
    if let Some(receipt) = &receipt {
        if receipt["schema"].as_u64() != Some(1) || !receipt["removed"].is_boolean() {
            return Err("invalid_receipt");
        }
        let files = receipt["files"].as_array().ok_or("invalid_receipt")?;
        if files.len() != paths.len() {
            return Err("invalid_receipt");
        }
        for file in files {
            let index = paths
                .iter()
                .position(|path| file["relative"].as_str() == Some(path))
                .ok_or("invalid_receipt")?;
            let block = file["block"].as_str().ok_or("invalid_receipt")?;
            if blocks[index].is_some()
                || !file["existed"].is_boolean()
                || !hash_valid(&file["before_hash"])
                || !file["backup"].is_string()
                || !block.starts_with(&format!("\n{BEGIN}"))
                || !block.ends_with(END)
                || block.matches(BEGIN).count() != 1
                || block.matches(END).count() != 1
            {
                return Err("invalid_receipt");
            }
            blocks[index] = Some(block);
        }
        active = receipt["removed"] == false;
    }
    for (index, path) in paths.iter().enumerate() {
        let bytes = read(root, path)?;
        if active && bytes.is_none() {
            return Err("owned_file_missing");
        }
        let text = String::from_utf8(bytes.unwrap_or_default()).map_err(|_| "invalid_settings")?;
        if active {
            let block = blocks[index].ok_or("invalid_receipt")?;
            if text.matches(block).count() != 1
                || text.matches(BEGIN).count() != 1
                || text.matches(END).count() != 1
            {
                return Err("owned_block_changed");
            }
        } else if text.contains(BEGIN) || text.contains(END) {
            return Err("unowned_managed_markers");
        } else if index == 1 && text.contains("mcp_servers.agent_progress") {
            return Err("unowned_mcp_server");
        }
    }
    Ok(active)
}

fn valid_hook(hook: &Value) -> bool {
    hook.is_object()
        && hook["hooks"].as_array().is_some_and(|entries| {
            !entries.is_empty()
                && entries.iter().all(|entry| {
                    entry["type"] == "command"
                        && entry["command"].as_str().is_some_and(|s| !s.is_empty())
                })
        })
}

fn native(root: &Path, agent: &str, desired_hook: Option<&Value>) -> Check<bool> {
    let relative = if agent == "claude" {
        ".claude/settings.local.json"
    } else {
        ".opencode/plugins/agent-progress.js"
    };
    let receipt = parse_receipt(read(
        root,
        &format!(".agent-progress/connection-{agent}.json"),
    )?)?;
    if let Some(receipt) = &receipt
        && (receipt["agent"] != agent
            || receipt["relative"] != relative
            || !receipt["removed"].is_boolean()
            || !hash_valid(&receipt["installed_hash"])
            || !valid_hook(&receipt["hook"]))
    {
        return Err("invalid_receipt");
    }
    let active = receipt.as_ref().is_some_and(|r| r["removed"] == false);
    let bytes = read(root, relative)?;
    if active && bytes.is_none() {
        return Err("owned_file_missing");
    }
    if agent == "opencode" {
        return match (active, bytes) {
            (false, None) => Ok(false),
            (false, Some(_)) => Err("unowned_plugin"),
            (true, Some(bytes))
                if receipt.as_ref().unwrap()["installed_hash"] == recovery::hash(&bytes) =>
            {
                Ok(true)
            }
            _ => Err("owned_plugin_changed"),
        };
    }
    let bytes = bytes.unwrap_or_default();
    let settings: Value = if bytes.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&bytes).map_err(|_| "invalid_settings")?
    };
    let settings = settings.as_object().ok_or("invalid_settings")?;
    let hooks = settings
        .get("hooks")
        .map(|hooks| hooks.as_object().ok_or("invalid_hooks"))
        .transpose()?;
    for event in EVENTS {
        let entries = hooks
            .and_then(|hooks| hooks.get(event))
            .map(|entries| entries.as_array().ok_or("invalid_hook_entries"))
            .transpose()?;
        let installed = receipt.as_ref().map(|r| &r["hook"]);
        if active {
            if entries.map_or(0, |entries| {
                entries
                    .iter()
                    .filter(|entry| Some(*entry) == installed)
                    .count()
            }) != 1
            {
                return Err("owned_hook_changed");
            }
        } else if entries.is_some_and(|entries| {
            entries
                .iter()
                .any(|entry| Some(entry) == installed || Some(entry) == desired_hook)
        }) {
            return Err("unowned_hook");
        }
    }
    Ok(active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn put(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn save(root: &Path, relative: &str, value: &Value) {
        put(root, relative, &serde_json::to_vec(value).unwrap());
    }

    fn assert_state(root: &Path, agent: &str, hook: Option<&Value>, expected: &str) -> Value {
        let result = inspect(root, agent, hook);
        assert_eq!(result["connection_status"], expected, "{result}");
        assert_eq!(result.as_object().unwrap().len(), 3);
        assert_eq!(
            result["next_action"],
            match expected {
                "managed" => "none",
                "unmanaged" => "apply",
                _ => "inspect",
            }
        );
        result
    }

    fn codex_fixture(root: &Path) -> Value {
        let block = format!("\n{BEGIN}private-value\n{END}");
        let files: Vec<Value> = ["AGENTS.md", ".codex/config.toml"]
            .iter()
            .map(|path| {
                put(root, path, block.as_bytes());
                json!({"relative":path,"existed":false,"before_hash":recovery::hash(b""),
                "block":block,"backup":"connection-backups/private.txt"})
            })
            .collect();
        let receipt = json!({"schema":1,"removed":false,"files":files});
        save(root, ".agent-progress/connection.json", &receipt);
        receipt
    }

    fn native_fixture(root: &Path, agent: &str) -> (Value, Value) {
        let hook = json!({"hooks":[{"type":"command","command":"private-command","timeout":10}]});
        let (relative, bytes) =
            if agent == "claude" {
                (".claude/settings.local.json", serde_json::to_vec(&json!({"hooks":{
                "SessionStart":[hook.clone()],"PostToolUse":[hook.clone()],"Stop":[hook.clone()]
            }})).unwrap())
            } else {
                (
                    ".opencode/plugins/agent-progress.js",
                    b"private-plugin".to_vec(),
                )
            };
        put(root, relative, &bytes);
        let receipt = json!({"agent":agent,"relative":relative,"installed_hash":recovery::hash(&bytes),
            "hook":hook,"removed":false});
        save(
            root,
            &format!(".agent-progress/connection-{agent}.json"),
            &receipt,
        );
        (receipt, hook)
    }

    #[test]
    fn empty_inspection_is_private_and_creates_nothing() {
        let dir = tempdir().unwrap();
        for agent in ["codex", "claude", "opencode"] {
            assert_state(dir.path(), agent, None, "unmanaged");
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn codex_preserves_user_appends_and_reports_interrupted_or_modified_install() {
        let dir = tempdir().unwrap();
        let receipt = codex_fixture(dir.path());
        let block = receipt["files"][0]["block"].as_str().unwrap();
        let appended = format!("{block}\n# unrelated private user content\n");
        put(dir.path(), "AGENTS.md", appended.as_bytes());
        assert_state(dir.path(), "codex", None, "managed");
        assert_eq!(
            fs::read(dir.path().join("AGENTS.md")).unwrap(),
            appended.as_bytes()
        );
        put(
            dir.path(),
            "AGENTS.md",
            format!("{block}{block}").as_bytes(),
        );
        let output = assert_state(dir.path(), "codex", None, "conflict");
        assert!(!output.to_string().contains("private"));
        fs::remove_file(dir.path().join("AGENTS.md")).unwrap();
        assert_state(dir.path(), "codex", None, "conflict");
    }

    #[test]
    fn codex_rejects_invalid_receipts_and_unowned_remnants() {
        let dir = tempdir().unwrap();
        let receipt = codex_fixture(dir.path());
        for changed in [
            json!({"schema":2,"removed":false,"files":receipt["files"]}),
            json!({"schema":1,"removed":false,"files":[receipt["files"][0].clone(),receipt["files"][0].clone()]}),
            json!({"schema":1,"removed":false,"files":[]}),
        ] {
            save(dir.path(), ".agent-progress/connection.json", &changed);
            assert_state(dir.path(), "codex", None, "conflict");
        }
        let mut removed = receipt;
        removed["removed"] = json!(true);
        save(dir.path(), ".agent-progress/connection.json", &removed);
        assert_state(dir.path(), "codex", None, "conflict");
        fs::remove_file(dir.path().join(".agent-progress/connection.json")).unwrap();
        assert_state(dir.path(), "codex", None, "conflict");
        put(dir.path(), "AGENTS.md", b"unrelated");
        put(
            dir.path(),
            ".codex/config.toml",
            b"[mcp_servers.agent_progress]\n",
        );
        assert_state(dir.path(), "codex", None, "conflict");
        put(dir.path(), ".codex/config.toml", b"# unrelated\n");
        assert_state(dir.path(), "codex", None, "unmanaged");
    }

    #[test]
    fn claude_allows_unrelated_fields_but_rejects_duplicate_missing_or_unowned_hooks() {
        let dir = tempdir().unwrap();
        let (mut receipt, hook) = native_fixture(dir.path(), "claude");
        let path = ".claude/settings.local.json";
        let mut settings: Value =
            serde_json::from_slice(&fs::read(dir.path().join(path)).unwrap()).unwrap();
        settings["private"] = json!("secret-value");
        settings["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(json!({"unrelated":true}));
        save(dir.path(), path, &settings);
        let before = fs::read(dir.path().join(path)).unwrap();
        assert_state(dir.path(), "claude", Some(&hook), "managed");
        assert_eq!(fs::read(dir.path().join(path)).unwrap(), before);
        settings["hooks"]["Stop"]
            .as_array_mut()
            .unwrap()
            .push(hook.clone());
        save(dir.path(), path, &settings);
        assert_state(dir.path(), "claude", Some(&hook), "conflict");
        receipt["removed"] = json!(true);
        save(
            dir.path(),
            ".agent-progress/connection-claude.json",
            &receipt,
        );
        assert_state(dir.path(), "claude", None, "conflict");
        fs::remove_file(dir.path().join(".agent-progress/connection-claude.json")).unwrap();
        assert_state(dir.path(), "claude", Some(&hook), "conflict");
        save(dir.path(), path, &json!({"private":"secret-value"}));
        assert_state(dir.path(), "claude", Some(&hook), "unmanaged");
    }

    #[test]
    fn claude_rejects_malformed_settings_and_receipts_without_leaking_them() {
        let dir = tempdir().unwrap();
        for bytes in [
            b"secret-invalid-json".as_slice(),
            b"[]",
            b"{\"hooks\":null}",
            b"{\"hooks\":{\"Stop\":{}}}",
        ] {
            put(dir.path(), ".claude/settings.local.json", bytes);
            let result = assert_state(dir.path(), "claude", None, "conflict");
            assert!(!result.to_string().contains("secret"));
        }
        let (mut receipt, hook) = native_fixture(dir.path(), "claude");
        receipt["relative"] = json!("../private");
        save(
            dir.path(),
            ".agent-progress/connection-claude.json",
            &receipt,
        );
        assert_state(dir.path(), "claude", Some(&hook), "conflict");
    }

    #[test]
    fn opencode_checks_installed_bytes_and_receipt_ownership() {
        let dir = tempdir().unwrap();
        let (mut receipt, hook) = native_fixture(dir.path(), "opencode");
        assert_state(dir.path(), "opencode", Some(&hook), "managed");
        put(
            dir.path(),
            ".opencode/plugins/agent-progress.js",
            b"user-edited-private-plugin",
        );
        assert_state(dir.path(), "opencode", Some(&hook), "conflict");
        receipt["removed"] = json!(true);
        save(
            dir.path(),
            ".agent-progress/connection-opencode.json",
            &receipt,
        );
        assert_state(dir.path(), "opencode", Some(&hook), "conflict");
        fs::remove_file(dir.path().join(".opencode/plugins/agent-progress.js")).unwrap();
        assert_state(dir.path(), "opencode", Some(&hook), "unmanaged");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_files_and_parents_including_broken_links() {
        use std::os::unix::fs::symlink;
        for relative in [
            ".agent-progress",
            ".agent-progress/connection.json",
            ".codex",
            "AGENTS.md",
            ".claude",
            ".opencode/plugins",
        ] {
            let dir = tempdir().unwrap();
            let target = dir.path().join(relative);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            symlink(dir.path().join("missing-private-target"), &target).unwrap();
            let agent = if relative.starts_with(".claude") {
                "claude"
            } else if relative.starts_with(".opencode") {
                "opencode"
            } else {
                "codex"
            };
            assert_state(dir.path(), agent, None, "conflict");
            assert!(target.is_symlink());
        }
    }
}
