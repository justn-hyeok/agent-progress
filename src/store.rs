use crate::model::Plan;
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

const START: &str = "```ap-plan\n";
const END: &str = "\n```\n";
const LIMIT: u64 = 16 * 1024 * 1024;

pub fn encode(plan: &Plan) -> Result<String> {
    plan.validate()?;
    Ok(format!(
        "# agent-progress\n\n{}\n\n프로젝트: {}\n\n{}\n\n아래 ap-plan 블록이 원본입니다. 요약은 CLI 저장 시 갱신됩니다.\n\n{START}{}{END}",
        plan.goal,
        plan.project,
        plan.progress(),
        serde_json::to_string_pretty(plan)?
    ))
}

pub fn decode(text: &str) -> Result<Plan> {
    let (_, rest) = text.split_once(START).context("missing ap-plan block")?;
    let (json, tail) = rest.split_once(END).context("incomplete ap-plan block")?;
    ensure!(!tail.contains(START), "multiple ap-plan blocks");
    let plan: Plan = serde_json::from_str(json).context("invalid plan JSON")?;
    plan.validate()?;
    Ok(plan)
}

fn read_text(path: &Path) -> Result<String> {
    use std::io::Read;
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    ensure!(file.metadata()?.is_file(), "plan must be a regular file");
    let mut text = String::new();
    file.take(LIMIT + 1).read_to_string(&mut text)?;
    ensure!(text.len() as u64 <= LIMIT, "plan exceeds 16 MiB limit");
    Ok(text)
}

pub fn read(path: &Path) -> Result<Plan> {
    decode(&read_text(path)?)
}

fn target(path: &Path, create: bool) -> Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if create {
        fs::create_dir_all(parent)?;
    }
    let result = parent
        .canonicalize()?
        .join(path.file_name().context("plan filename required")?);
    if let Ok(meta) = fs::symlink_metadata(&result) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "refusing non-regular or symlink plan"
        );
    }
    Ok(result)
}

fn lock(path: &Path) -> Result<File> {
    let mut name = path.as_os_str().to_os_string();
    name.push(".ap-lock");
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(PathBuf::from(name))?;
    file.try_lock_exclusive()
        .context("plan is busy; retry after the other writer finishes")?;
    Ok(file) // OS releases lock on drop or process exit. Never unlink lock files.
}

fn write_atomic(path: &Path, content: &str, create: bool) -> Result<()> {
    ensure!(content.len() as u64 <= LIMIT, "plan exceeds 16 MiB limit");
    let parent = path.parent().context("missing parent")?;
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(content.as_bytes())?;
    temp.as_file().sync_all()?;
    crate::fault::checkpoint("file:temp-synced");
    if create {
        temp.persist_noclobber(path)?;
    } else {
        temp.persist(path)?;
    }
    crate::fault::checkpoint("file:replaced");
    File::open(parent)?
        .sync_all()
        .context("file replaced but directory sync failed; inspect plan before retry")?;
    crate::fault::checkpoint("file:dir-synced");
    Ok(())
}

pub fn create(path: &Path, plan: &Plan) -> Result<()> {
    let content = encode(plan)?;
    let path = target(path, true)?;
    let _lock = lock(&path)?;
    ensure!(!path.exists(), "plan already exists; refusing overwrite");
    write_atomic(&path, &content, true)
}

pub fn update(
    path: &Path,
    expected: Option<u64>,
    f: impl FnOnce(&mut Plan) -> Result<()>,
) -> Result<Plan> {
    let path = target(path, false)?;
    let _lock = lock(&path)?;
    let before = read_text(&path)?;
    let mut plan = decode(&before)?;
    if let Some(expected) = expected {
        ensure!(
            plan.revision == expected,
            "revision conflict: expected {expected}, found {}",
            plan.revision
        );
    }
    let revision = plan.revision;
    f(&mut plan)?;
    ensure!(
        plan.revision == revision + 1,
        "mutation must record exactly one event"
    );
    let content = encode(&plan)?;
    ensure!(
        read_text(&path)? == before,
        "file changed during update; refusing overwrite"
    );
    let backup = path.with_extension("v1-backup.md");
    if decode(&before)?.schema_version == 1 && !backup.exists() {
        crate::recovery::write(&backup, before.as_bytes(), false)?;
    }
    write_atomic(&path, &content, false)?;
    Ok(plan)
}
