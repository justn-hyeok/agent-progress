//! `ap skill install|remove`: make agents pick up the `ap` skill. The shared copy lives
//! in ~/.agents/skills/ap (read directly by most harnesses); harnesses that only read
//! their own root get a symlink; Claude Code also gets a marked CLAUDE.md block because
//! it does not reach for a listed skill on its own. Only what `ap` created is removed.

use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

const SKILL_MD: &str = include_str!("../skills/ap/SKILL.md");
const OPENAI_YAML: &str = include_str!("../skills/ap/agents/openai.yaml");
const MARKER: &str = ".ap-managed";
const BEGIN: &str = "<!-- BEGIN agent-progress (managed by `ap skill`) -->";
const END: &str = "<!-- END agent-progress -->";
const CLAUDE_BLOCK: &str = "### Progress Pane (ap)\n\n- For any multi-step task (implementation, debugging, review, research), use the `ap` skill without being asked: record the goal and steps with `ap goal` / `ap add`, then `ap start` / `ap done` / `ap block` as you go. Recording never opens a pane by itself; when the user wants to watch, they run `ap open` (or ask you to). Skip it for one-shot questions or trivial single actions.\n";

/// Harness skill roots that do not read ~/.agents/skills directly.
const LINK_ROOTS: [&str; 4] = [
    ".claude/skills",
    ".codex/skills",
    ".config/opencode/skills",
    ".gemini/antigravity-cli/skills",
];

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

fn shared(home: &Path) -> PathBuf {
    home.join(".agents/skills/ap")
}

fn files() -> [(&'static str, &'static str); 2] {
    [("SKILL.md", SKILL_MD), ("agents/openai.yaml", OPENAI_YAML)]
}

fn write_skill(dir: &Path) -> Result<()> {
    for (rel, body) in files() {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().context("skill path")?)?;
        fs::write(path, body)?;
    }
    fs::write(dir.join(MARKER), "installed by `ap skill install`\n")?;
    Ok(())
}

fn current(dir: &Path) -> bool {
    files()
        .iter()
        .all(|(rel, body)| fs::read_to_string(dir.join(rel)).is_ok_and(|s| s == *body))
}

fn claude_md(home: &Path) -> Option<PathBuf> {
    home.join(".claude")
        .is_dir()
        .then(|| home.join(".claude/CLAUDE.md"))
}

fn block() -> String {
    format!("{BEGIN}\n{CLAUDE_BLOCK}{END}\n")
}

fn strip_block(text: &str) -> Option<String> {
    let start = text.find(BEGIN)?;
    let end = text[start..].find(END)? + start + END.len();
    let rest = text[end..].strip_prefix('\n').unwrap_or(&text[end..]);
    Some(format!("{}{}", text[..start].trim_end_matches('\n'), {
        if rest.is_empty() {
            "\n".to_string()
        } else {
            format!("\n\n{}", rest.trim_start_matches('\n'))
        }
    }))
}

/// Writes go through a symlinked CLAUDE.md to its target; a timestamped backup is kept.
fn write_with_backup(path: &Path, text: &str) -> Result<()> {
    if path.exists() {
        let stamp = crate::store::now();
        let backup = PathBuf::from(format!("{}.ap-backup-{stamp}", path.display()));
        fs::copy(path, &backup)?;
    }
    fs::write(path, text)?;
    Ok(())
}

pub fn install(dry_run: bool) -> Result<Vec<String>> {
    let home = home()?;
    let mut report = Vec::new();
    let dir = shared(&home);
    let managed = dir.join(MARKER).exists();
    let shared_ok = if fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        report.push(format!(
            "유지: {} (심볼릭 링크, 사용자 관리)",
            dir.display()
        ));
        true
    } else if !dir.exists() {
        report.push(format!("설치: {}", dir.display()));
        if !dry_run {
            write_skill(&dir)?;
        }
        true
    } else if current(&dir) {
        if !managed && !dry_run {
            fs::write(dir.join(MARKER), "adopted by `ap skill install`\n")?;
        }
        report.push(format!("최신: {}", dir.display()));
        true
    } else if managed {
        report.push(format!("갱신: {}", dir.display()));
        if !dry_run {
            write_skill(&dir)?;
        }
        true
    } else {
        report.push(format!(
            "건너뜀: {} (ap가 설치하지 않은 다른 내용)",
            dir.display()
        ));
        false
    };
    if shared_ok {
        for root in LINK_ROOTS {
            let root = home.join(root);
            if !root.is_dir() {
                continue;
            }
            let link = root.join("ap");
            match fs::read_link(&link) {
                Ok(target) if target == dir => report.push(format!("최신: {}", link.display())),
                Ok(_) => report.push(format!("건너뜀: {} (다른 대상)", link.display())),
                Err(_) if link.exists() => {
                    report.push(format!("건너뜀: {} (이미 있음)", link.display()))
                }
                Err(_) => {
                    report.push(format!("연결: {}", link.display()));
                    if !dry_run {
                        std::os::unix::fs::symlink(&dir, &link)?;
                    }
                }
            }
        }
    }
    if let Some(path) = claude_md(&home) {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let wanted = if text.contains(&block()) {
            report.push(format!("최신: {} (지침 블록)", path.display()));
            None
        } else {
            let base = strip_block(&text).unwrap_or(text);
            Some(if base.trim().is_empty() {
                block()
            } else {
                format!("{}\n\n{}", base.trim_end_matches('\n'), block())
            })
        };
        if let Some(new) = wanted {
            report.push(format!("지침 추가: {}", path.display()));
            if !dry_run {
                write_with_backup(&path, &new)?;
            }
        }
    }
    report.push(
        "참고: GJC는 스캔 폴더에 실제 디렉터리 복사, Hermes는 skills.external_dirs 등록이 필요합니다"
            .into(),
    );
    Ok(report)
}

pub fn remove(dry_run: bool) -> Result<Vec<String>> {
    let home = home()?;
    let mut report = Vec::new();
    let dir = shared(&home);
    if let Some(path) = claude_md(&home)
        && let Ok(text) = fs::read_to_string(&path)
        && let Some(rest) = strip_block(&text)
    {
        report.push(format!("지침 제거: {}", path.display()));
        if !dry_run {
            write_with_backup(&path, &rest)?;
        }
    }
    for root in LINK_ROOTS {
        let link = home.join(root).join("ap");
        if fs::read_link(&link).is_ok_and(|t| t == dir) {
            report.push(format!("연결 해제: {}", link.display()));
            if !dry_run {
                fs::remove_file(&link)?;
            }
        }
    }
    if dir.join(MARKER).exists() {
        report.push(format!("제거: {}", dir.display()));
        if !dry_run {
            fs::remove_dir_all(&dir)?;
        }
    } else if dir.exists() {
        report.push(format!("유지: {} (ap가 설치하지 않음)", dir.display()));
    }
    if report.is_empty() {
        bail!("ap가 설치한 스킬이 없습니다");
    }
    Ok(report)
}
