//! Local data recovery. Every replacement is explicit and preserves displaced bytes.
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

pub fn read(path: &Path) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    ensure!(
        meta.is_file() && !meta.file_type().is_symlink(),
        "expected a regular non-symlink file"
    );
    ensure!(meta.len() <= 16 * 1024 * 1024, "data exceeds 16 MiB");
    let mut bytes = Vec::new();
    File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16 * 1024 * 1024, "data exceeds 16 MiB");
    Ok(bytes)
}

pub fn hash(bytes: &[u8]) -> String {
    sha1_smol::Sha1::from(bytes).digest().to_string()
}

pub fn write(path: &Path, bytes: &[u8], replace: bool) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(parent.is_dir(), "destination directory must already exist");
    if let Ok(meta) = fs::symlink_metadata(path) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "refusing non-regular/symlink destination"
        );
    }
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    if replace {
        temp.persist(path)?;
    } else {
        temp.persist_noclobber(path)?;
    }
    File::open(parent)?
        .sync_all()
        .context("destination replaced; directory sync failed")?;
    Ok(())
}
