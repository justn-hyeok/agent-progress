//! Opt-in shell command integration. Native executables and unrelated rc content stay intact.
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

const BEGIN: &str = "# BEGIN agent-progress shell";
const END: &str = "# END agent-progress shell";
pub const BLOCK: &str = r#"
# BEGIN agent-progress shell
# Managed by ap shell install/remove. Existing codex definitions take precedence.
if (( ! $+functions[codex] && ! $+aliases[codex] )); then
  function codex() {
    local _ap_command='' _ap_arg _ap_skip=0
    for _ap_arg in "$@"; do
      if (( _ap_skip )); then _ap_skip=0; continue; fi
      case $_ap_arg in
        --) break ;;
        -c|--config|-m|--model|-p|--profile|-C|--cd|-i|--image|-a|--ask-for-approval|-s|--sandbox|--enable|--disable|--remote|--remote-auth-token-env) _ap_skip=1 ;;
        -*) ;;
        *) _ap_command=$_ap_arg; break ;;
      esac
    done
    case $_ap_command in
      exec|e|login|logout|mcp|review|app-server|exec-server|doctor|update|features|completion|sandbox|queue|archive|unarchive|delete|migrate-rollouts|cloud|remote-control|app)
        command codex "$@"; return $? ;;
    esac
    if [[ ${HERDR_ENV:-0} == 1 ]] && command -v ap >/dev/null 2>&1; then
      command ap launch --agent codex -- "$@"
    else
      command codex "$@"
    fi
  }
fi
# END agent-progress shell
"#;

pub fn block(shell: &str) -> Result<String> {
    ensure!(
        matches!(shell, "zsh" | "bash" | "fish"),
        "unsupported shell"
    );
    let guard = if shell == "zsh" {
        "if (( ! $+functions[codex] && ! $+aliases[codex] )); then"
    } else {
        "if [ -n \"${BASH_VERSION-}\" ] && ! declare -F codex >/dev/null && ! alias codex >/dev/null 2>&1; then"
    };
    let body = if shell == "fish" {
        r#"if not functions -q codex
  function codex --wraps codex
    if command -q ap
      command ap launch --agent codex -- $argv
    else
      command codex $argv
    end
  end
end"#
            .to_owned()
    } else {
        format!(
            r#"{guard}
  codex() {{
    if command -v ap >/dev/null 2>&1; then
      command ap launch --agent codex -- "$@"
    else
      command codex "$@"
    fi
  }}
fi"#
        )
    };
    Ok(format!(
        "\n{BEGIN}\n# Managed by ap shell install/remove. Existing codex definitions take precedence.\n{body}\n{END}\n"
    ))
}

pub fn default_rc_for(shell: &str) -> Result<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").context("shell home unavailable")?);
    match shell {
        "zsh" => default_rc(),
        // The public default bash command also manages the active login file.
        "bash" => Ok(home.join(".bashrc")),
        "fish" => Ok(std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("fish/config.fish")),
        _ => anyhow::bail!("unsupported shell"),
    }
}

pub fn default_rc() -> Result<PathBuf> {
    let home = std::env::var_os("ZDOTDIR")
        .or_else(|| std::env::var_os("HOME"))
        .context("shell home unavailable")?;
    Ok(PathBuf::from(home).join(".zshrc"))
}

fn managed(bytes: &str, shell: &str) -> Result<Option<std::ops::Range<usize>>> {
    let begins = bytes.matches(BEGIN).count();
    let ends = bytes.matches(END).count();
    ensure!(
        begins == ends && begins <= 1,
        "duplicate or incomplete ap shell markers; preserving rc file"
    );
    if begins == 0 {
        return Ok(None);
    }
    let current = block(shell)?;
    for candidate in [current.as_str(), if shell == "zsh" { BLOCK } else { "" }] {
        if !candidate.is_empty()
            && let Some(index) = bytes.find(candidate)
        {
            return Ok(Some(index..index + candidate.len()));
        }
    }
    anyhow::bail!("managed shell block was edited; preserving rc file")
}

fn read(path: &Path) -> Result<Vec<u8>> {
    if path.exists() || path.is_symlink() {
        crate::recovery::read(path)
    } else {
        Ok(Vec::new())
    }
}

pub fn manage(action: &str, rc: &Path) -> Result<Value> {
    manage_for(action, rc, "zsh")
}

pub fn manage_for(action: &str, rc: &Path, shell: &str) -> Result<Value> {
    let block = block(shell)?;
    ensure!(
        matches!(action, "preview" | "status" | "install" | "remove"),
        "unknown shell action"
    );
    let rc = if rc.is_absolute() {
        rc.to_owned()
    } else {
        std::env::current_dir()?.join(rc)
    };
    ensure!(
        !rc.is_symlink(),
        "refusing symlink rc file; choose its explicit regular target"
    );
    let before = read(&rc)?;
    let text = std::str::from_utf8(&before).context("rc file must be UTF-8; preserving it")?;
    let span = managed(text, shell)?;
    if matches!(action, "status" | "preview") {
        let mut output = json!({"shell":shell,"rc":rc,"installed":span.is_some(),"mutates":false});
        if action == "preview" {
            output["managed_addition"] = json!(block);
        }
        return Ok(output);
    }
    if (action == "install" && text.contains(&block)) || (action == "remove" && span.is_none()) {
        return Ok(json!({"shell":shell,"rc":rc,"installed":span.is_some(),"changed":false}));
    }
    let parent = rc.parent().context("rc parent unavailable")?;
    // fish's default directory may not exist yet. Refuse symlink ancestors before creation.
    for ancestor in parent.ancestors() {
        ensure!(!ancestor.is_symlink(), "refusing symlink rc directory");
        if ancestor.exists() {
            break;
        }
    }
    fs::create_dir_all(parent)?;
    let name = rc
        .file_name()
        .context("rc filename unavailable")?
        .to_string_lossy();
    let lock_path = parent.join(format!("{name}.agent-progress.lock"));
    ensure!(!lock_path.is_symlink(), "refusing symlink shell lock");
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)?;
    lock.try_lock_exclusive()
        .context("shell configuration is busy")?;
    ensure!(
        read(&rc)? == before,
        "rc changed during setup; retry without overwriting edits"
    );
    let mut after = text.to_owned();
    if let Some(span) = span {
        after.replace_range(span, "");
    }
    if action == "install" {
        after.push_str(&block);
    }
    let backup = parent.join(format!(
        "{name}.agent-progress-{}.bak",
        crate::recovery::hash(&before)
    ));
    if backup.exists() || backup.is_symlink() {
        ensure!(
            crate::recovery::read(&backup)? == before,
            "shell backup conflicts; preserving rc file"
        );
    } else {
        crate::recovery::write(&backup, &before, false)?;
    }
    let permissions = fs::metadata(&rc).ok().map(|m| m.permissions());
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(after.as_bytes())?;
    if let Some(permissions) = permissions {
        temp.as_file().set_permissions(permissions)?;
    }
    temp.as_file().sync_all()?;
    ensure!(
        read(&rc)? == before,
        "rc changed during setup; preserving concurrent edits"
    );
    temp.persist(&rc)
        .context("could not replace rc file atomically")?;
    Ok(
        json!({"shell":shell,"rc":rc,"installed":action=="install","changed":true,"backup":backup,
        "activate":"Open a new terminal or source this rc file. Bash login profiles must source .bashrc. Removal takes effect in new shells; clear a loaded function with unset -f codex (bash), unfunction codex (zsh), or functions -e codex (fish)."}),
    )
}
