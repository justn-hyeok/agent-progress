//! Optional project-local Codex integration, with preview, exact backups and narrow removal.
use crate::{model, recovery};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, fs::OpenOptions, path::Path};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct Edit {
    relative: String,
    existed: bool,
    before_hash: String,
    block: String,
    backup: String,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    schema: u32,
    removed: bool,
    files: Vec<Edit>,
}

fn executable_path() -> Result<std::path::PathBuf> {
    let actual = std::env::current_exe()?.canonicalize()?;
    // Keep the verified public install symlink in settings so an update changes the
    // adapter as well. Do not pin generated hooks to releases/<old-version>/ap.
    if let Some(arg) = std::env::args_os().next() {
        let candidate = std::path::PathBuf::from(arg);
        let candidates = if candidate.components().count() > 1 || candidate.is_absolute() {
            vec![if candidate.is_absolute() {
                candidate
            } else {
                std::env::current_dir()?.join(candidate)
            }]
        } else {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|dir| dir.join(&candidate))
                .collect()
        };
        for path in candidates {
            if path.is_absolute() && path.canonicalize().ok().as_ref() == Some(&actual) {
                return Ok(path);
            }
        }
    }
    Ok(actual)
}

pub fn manage(root: &Path, action: &str, writes: bool) -> Result<Value> {
    let root = root.canonicalize()?;
    let manifest = root.join("ap.project.json");
    let product = if manifest.exists() || manifest.is_symlink() {
        let project = crate::project::Project::open(&manifest)?;
        ensure!(
            project.root() == root,
            "project declaration resolves outside connection root"
        );
        Some(project)
    } else {
        None
    };
    ensure!(
        !writes || product.is_some(),
        "--allow-writes requires an existing ap.project.json; passive hooks do not expose product write tools"
    );
    if action != "preview" {
        ensure!(
            !root.join(".codex").is_symlink() && !root.join(".agent-progress").is_symlink(),
            "refusing symlink project settings directory"
        );
    }
    let directory = root.join(".agent-progress");
    if action != "preview" {
        fs::create_dir_all(&directory)?;
    }
    let receipt_path = directory.join("connection.json");
    let marker = "# BEGIN agent-progress managed\n";
    let end = "# END agent-progress managed\n";
    let executable = executable_path()?;
    let command = serde_json::to_string(executable.to_str().context("executable encoding")?)?;
    let args = if writes {
        vec!["mcp", "--allow-writes", "--project"]
    } else {
        vec!["mcp", "--project"]
    };
    let mut arguments = args.into_iter().map(String::from).collect::<Vec<_>>();
    arguments.push(root.join("ap.project.json").display().to_string());
    let hook_command = format!(
        "{} bridge --agent codex --root {}",
        crate::herdr::shell_quote(executable.to_str().context("executable encoding")?),
        crate::herdr::shell_quote(root.to_str().context("root encoding")?)
    );
    let mut hooks = String::new();
    for event in ["SessionStart", "UserPromptSubmit", "Stop"] {
        hooks.push_str(&format!("\n[[hooks.{event}]]\n[[hooks.{event}.hooks]]\ntype = \"command\"\ncommand = {}\ntimeout = 10\n", serde_json::to_string(&hook_command)?));
    }
    let configuration = if product.is_some() {
        format!(
            "[mcp_servers.agent_progress]\ncommand = {command}\nargs = {}\n{hooks}",
            serde_json::to_string(&arguments)?
        )
    } else {
        hooks
    };
    let snippets=[
        (".codex/config.toml", configuration),
        ("AGENTS.md","Use the existing goal/plan without asking users to register tasks. If ap.project.json exists, link plan items to its stable roadmap IDs; do not create a manifest just to display progress. A child suffix such as [AP-05/native] does not certify the whole AP-05 item. Distinguish reported, automated and human evidence. agent-progress observes; it never supervises or runs your work.\n".into()),
    ];
    if action == "preview" {
        let mut output = json!({"scope":"project-local only","writes_enabled":writes,"product_connected":product.is_some(),"files":snippets.iter().map(|(path,snippet)|json!({"path":root.join(path),"managed_addition":snippet})).collect::<Vec<_>>(),"mutates":false});
        if let Some(state) = crate::connection_status::inspect(&root, "codex", None).as_object() {
            output.as_object_mut().unwrap().extend(state.clone());
        }
        return Ok(output);
    }
    ensure!(
        !directory.join("connection.ap-lock").is_symlink(),
        "refusing symlink connection lock"
    );
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join("connection.ap-lock"))?;
    lock.try_lock_exclusive()
        .context("connection settings are busy")?;
    if action == "remove" {
        let mut receipt: Receipt = serde_json::from_slice(&recovery::read(&receipt_path)?)?;
        ensure!(
            receipt.schema == 1 && !receipt.removed,
            "managed integration already absent"
        );
        let mut edits = Vec::new();
        for edit in &receipt.files {
            ensure!(
                matches!(edit.relative.as_str(), "AGENTS.md" | ".codex/config.toml"),
                "invalid managed file"
            );
            let path = root.join(&edit.relative);
            let bytes = if path.exists() {
                recovery::read(&path)?
            } else {
                vec![]
            };
            let text = String::from_utf8(bytes.clone())?;
            let occurrences = text.matches(&edit.block).count();
            if occurrences == 0 && recovery::hash(&bytes) == edit.before_hash {
                continue;
            }
            ensure!(
                occurrences == 1,
                "managed block changed; keep user edits and inspect its private backup"
            );
            edits.push((path, bytes, text.replacen(&edit.block, "", 1), edit.existed));
        }
        for (path, before, after, existed) in edits {
            ensure!(
                recovery::read(&path)? == before,
                "settings changed during removal"
            );
            if !existed && after.is_empty() {
                fs::remove_file(&path)?;
            } else {
                recovery::write(&path, after.as_bytes(), true)?;
            }
        }
        receipt.removed = true;
        recovery::write(&receipt_path, &serde_json::to_vec_pretty(&receipt)?, true)?;
        return Ok(
            json!({"removed":true,"private_backups_retained":true,"global_config_changed":false}),
        );
    }
    ensure!(action == "apply", "unknown connection action");
    if receipt_path.exists() {
        let old: Receipt = serde_json::from_slice(&recovery::read(&receipt_path)?)?;
        ensure!(
            old.removed,
            "integration already managed; remove before changing it"
        );
    }
    let backup_directory = directory.join("connection-backups");
    fs::create_dir_all(&backup_directory)?;
    ensure!(
        !backup_directory.is_symlink(),
        "refusing symlink backup directory"
    );
    let mut receipt = Receipt {
        schema: 1,
        removed: false,
        files: vec![],
    };
    let mut contents = Vec::new();
    for (relative, snippet) in snippets {
        let path = root.join(relative);
        let existed = path.exists();
        let before = if existed {
            recovery::read(&path)?
        } else {
            vec![]
        };
        let text = String::from_utf8(before.clone())?;
        ensure!(
            !text.contains(marker) && !text.contains(end),
            "unowned managed markers already present"
        );
        if relative.ends_with("toml") {
            ensure!(
                !text.contains("mcp_servers.agent_progress"),
                "unmanaged same-name MCP server exists; preserve it"
            );
        }
        model::nonempty(relative, "managed path")?;
        let block = format!("\n{marker}{snippet}{end}");
        let backup = format!("connection-backups/{}.txt", Uuid::new_v4());
        receipt.files.push(Edit {
            relative: relative.into(),
            existed,
            before_hash: recovery::hash(&before),
            block: block.clone(),
            backup,
        });
        contents.push((path, before, format!("{text}{block}")));
    }
    for (edit, (_, before, _)) in receipt.files.iter().zip(&contents) {
        recovery::write(&directory.join(&edit.backup), before, false)?;
    }
    // Receipt first: an interrupted apply can be removed without guessing or deleting user text.
    recovery::write(&receipt_path, &serde_json::to_vec_pretty(&receipt)?, true)?;
    for (path, before, after) in contents {
        fs::create_dir_all(path.parent().unwrap())?;
        ensure!(
            if path.exists() {
                recovery::read(&path)? == before
            } else {
                before.is_empty()
            },
            "settings changed during apply"
        );
        recovery::write(&path, after.as_bytes(), !before.is_empty() || path.exists())?;
    }
    Ok(
        json!({"applied":true,"receipt":receipt_path,"writes_enabled":writes,"global_config_changed":false,"activation":"project must be trusted by Codex; restart the client to load its project MCP settings"}),
    )
}

pub fn manage_agent(root: &Path, action: &str, writes: bool, agent: &str) -> Result<Value> {
    if agent == "codex" {
        return manage(root, action, writes);
    }
    ensure!(matches!(agent, "claude" | "opencode"), "unsupported agent");
    let root = root.canonicalize()?;
    let manifest = root.join("ap.project.json");
    if manifest.exists() || manifest.is_symlink() {
        let project = crate::project::Project::open(&manifest)?;
        ensure!(
            project.root() == root,
            "project declaration resolves outside connection root"
        );
    }
    let directory = root.join(".agent-progress");
    if action != "preview" {
        ensure!(!directory.is_symlink(), "refusing symlink storage");
    }
    let executable = executable_path()?;
    let command = format!(
        "{} bridge --agent {} --root {}",
        crate::herdr::shell_quote(executable.to_str().context("binary encoding")?),
        agent,
        crate::herdr::shell_quote(root.to_str().context("root encoding")?)
    );
    let relative = if agent == "claude" {
        ".claude/settings.local.json"
    } else {
        ".opencode/plugins/agent-progress.js"
    };
    let path = root.join(relative);
    for parent in path.ancestors().take_while(|p| *p != root) {
        if action != "preview" {
            ensure!(!parent.is_symlink(), "refusing symlink settings path");
        }
    }
    let receipt_path = directory.join(format!("connection-{agent}.json"));
    let events = ["SessionStart", "PostToolUse", "Stop"];
    let hook = json!({"hooks":[{"type":"command","command":command,"timeout":10}]});
    let plugin = format!(
        r#"// agent-progress managed observer; no process supervision, no transcript copies.
import {{ spawn }} from "node:child_process";
export const AgentProgress = async ({{ directory }}) => {{
 const roles = new Map();
 return {{
  event: async ({{ event }}) => {{
    const p = event.properties ?? {{}};
    if (event.type === "message.updated") {{ roles.set(p.info?.id, p.info?.role); return; }}
    const id = p.sessionID ?? p.info?.sessionID ?? p.info?.id ?? p.part?.sessionID;
    if (!id || !["session.created","session.updated","todo.updated","message.part.updated"].includes(event.type)) return;
    const input = {{ cwd: directory, session_id: id, hook_event_name: event.type }};
    if (event.type === "todo.updated") input.todos = p.todos;
    if (event.type === "message.part.updated") {{
      if (p.part?.type !== "text" || !p.part?.time?.end || roles.get(p.part?.messageID) !== "assistant") return;
      input.text = p.part.text;
    }}
    await new Promise((resolve) => {{
      const child = spawn({exe}, ["bridge", "--agent", "opencode", "--root", {root}], {{stdio:["pipe","ignore","ignore"]}});
      const timer = setTimeout(() => {{ child.kill(); resolve(); }}, 10000);
      child.on("error", () => {{clearTimeout(timer); resolve();}});
      child.on("exit", () => {{clearTimeout(timer); resolve();}});
      child.stdin.on("error", () => {{}});
      child.stdin.end(JSON.stringify(input));
    }});
  }}
 }};
}};
"#,
        exe = serde_json::to_string(executable.to_str().context("binary encoding")?)?,
        root = serde_json::to_string(root.to_str().context("root encoding")?)?
    );
    if action == "preview" {
        let mut output = json!({"agent":agent,"mutates":false,"path":path,"command":command,"events":events,"plugin":if agent=="opencode" {Some(plugin.as_str())} else {None}});
        if let Some(state) =
            crate::connection_status::inspect(&root, agent, Some(&hook)).as_object()
        {
            output.as_object_mut().unwrap().extend(state.clone());
        }
        return Ok(output);
    }
    fs::create_dir_all(&directory)?;
    let lock_path = directory.join(format!("connection-{agent}.lock"));
    ensure!(!lock_path.is_symlink(), "refusing symlink connection lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path)?;
    lock.try_lock_exclusive().context("settings busy")?;
    if action == "remove" {
        let receipt: Value = serde_json::from_slice(&recovery::read(&receipt_path)?)?;
        ensure!(
            receipt["relative"] == relative,
            "invalid integration receipt"
        );
        let before = recovery::read(&path)?;
        if agent == "opencode" {
            ensure!(
                recovery::hash(&before)
                    == receipt["installed_hash"].as_str().context("missing hash")?,
                "plugin edited; preserve user changes"
            );
            fs::remove_file(&path)?;
        } else {
            let mut settings: Value = serde_json::from_slice(&before)?;
            for event in events {
                let entries = settings["hooks"][event]
                    .as_array_mut()
                    .context("hook settings changed")?;
                let expected = &receipt["hook"];
                ensure!(
                    entries.iter().filter(|x| *x == expected).count() == 1,
                    "managed hook edited; preserve user changes"
                );
                entries.retain(|x| x != expected);
                if entries.is_empty() {
                    settings["hooks"].as_object_mut().unwrap().remove(event);
                }
            }
            if settings["hooks"].as_object().is_some_and(|x| x.is_empty()) {
                settings.as_object_mut().unwrap().remove("hooks");
            }
            recovery::write(&path, &serde_json::to_vec_pretty(&settings)?, true)?;
        }
        // Keep the receipt/backup as recovery evidence; permit a later explicit re-apply.
        let mut removed = receipt;
        removed["removed"] = json!(true);
        recovery::write(&receipt_path, &serde_json::to_vec_pretty(&removed)?, true)?;
        return Ok(json!({"removed":true,"agent":agent,"private_backup_retained":true}));
    }
    ensure!(action == "apply", "invalid integration action");
    if receipt_path.exists() {
        let old: Value = serde_json::from_slice(&recovery::read(&receipt_path)?)?;
        ensure!(old["removed"] == true, "integration already managed");
    }
    let before = if path.exists() {
        recovery::read(&path)?
    } else {
        vec![]
    };
    let after = if agent == "claude" {
        let mut settings: Value = if before.is_empty() {
            json!({})
        } else {
            serde_json::from_slice(&before)?
        };
        let object = settings
            .as_object_mut()
            .context("settings must be an object")?;
        let hooks = object
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .context("hooks must be an object")?;
        for event in events {
            let list = hooks
                .entry(event)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .context("hook entries must be an array")?;
            ensure!(!list.iter().any(|v| v == &hook), "unowned same hook exists");
            list.push(hook.clone());
        }
        serde_json::to_vec_pretty(&settings)?
    } else {
        ensure!(!path.exists(), "unowned plugin exists");
        plugin.into_bytes()
    };
    let backup_directory = directory.join("connection-backups");
    ensure!(
        !backup_directory.is_symlink(),
        "refusing symlink backup directory"
    );
    fs::create_dir_all(&backup_directory)?;
    let backup = backup_directory.join(format!("{agent}-{}.json", Uuid::new_v4()));
    recovery::write(&backup, &before, false)?;
    fs::create_dir_all(path.parent().context("settings parent")?)?;
    let receipt = json!({"agent":agent,"relative":relative,"backup":backup,"installed_hash":recovery::hash(&after),"hook":hook,"removed":false});
    recovery::write(&receipt_path, &serde_json::to_vec_pretty(&receipt)?, true)?;
    ensure!(
        if path.exists() {
            recovery::read(&path)? == before
        } else {
            before.is_empty()
        },
        "settings changed during apply"
    );
    recovery::write(&path, &after, true)?;
    Ok(json!({"applied":true,"agent":agent,"global_config_changed":false}))
}
