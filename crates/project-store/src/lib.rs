//! Atomic JSON persistence and recoverable snapshots.
#![forbid(unsafe_code)]

use stagecue_project_model::{migrate_value, validate_project, MigrationError, SongProject};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("migration error: {0}")]
    Migration(#[from] MigrationError),
    #[error("project is invalid: {0}")]
    Invalid(String),
}

pub fn load(path: impl AsRef<Path>) -> Result<SongProject, StoreError> {
    let bytes = fs::read(path)?;
    let value = serde_json::from_slice(&bytes)?;
    Ok(migrate_value(value)?)
}

pub fn save_atomic(path: impl AsRef<Path>, project: &SongProject) -> Result<(), StoreError> {
    let path = path.as_ref();
    let report = validate_project(project);
    if !report.is_valid() {
        return Err(StoreError::Invalid(
            report
                .errors()
                .map(|i| i.message.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temp = temp_path(path);
    let data = serde_json::to_vec_pretty(project)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    file.write_all(&data)?;
    file.sync_all()?;
    if path.exists() {
        let backup = path.with_extension("stagecue.backup");
        fs::copy(path, backup)?;
    }
    fs::rename(&temp, path)?;
    if let Ok(directory) = fs::File::open(parent) {
        let _ = directory.sync_all();
    }
    Ok(())
}

pub fn write_recovery_snapshot(
    path: impl AsRef<Path>,
    project: &SongProject,
) -> Result<PathBuf, StoreError> {
    let recovery = path.as_ref().with_extension("stagecue-recovery");
    save_atomic(&recovery, project)?;
    Ok(recovery)
}

fn temp_path(path: &Path) -> PathBuf {
    let mut extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("stagecue")
        .to_owned();
    extension.push_str(".tmp");
    path.with_extension(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SongProject {
        serde_json::from_str(include_str!(
            "../../../fixtures/standard-rock.stagecue.json"
        ))
        .unwrap()
    }

    #[test]
    fn save_and_reopen_and_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.stagecue.json");
        let mut project = fixture();
        save_atomic(&path, &project).unwrap();
        assert_eq!(load(&path).unwrap(), project);
        project.title = "Changed".into();
        save_atomic(&path, &project).unwrap();
        assert_eq!(load(&path).unwrap().title, "Changed");
        assert!(path.with_extension("stagecue.backup").exists());
    }

    #[test]
    fn recovery_does_not_replace_primary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.stagecue.json");
        let project = fixture();
        save_atomic(&path, &project).unwrap();
        let mut changed = project.clone();
        changed.title = "Recovery".into();
        let recovery = write_recovery_snapshot(&path, &changed).unwrap();
        assert_eq!(load(&path).unwrap().title, project.title);
        assert_eq!(load(&recovery).unwrap().title, "Recovery");
    }
}
