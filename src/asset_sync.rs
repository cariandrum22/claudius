#![allow(missing_docs)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST_FILE_NAME: &str = ".claudius-managed-files.json";
const MANIFEST_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileMapping {
    pub source_path: PathBuf,
    pub relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncBehavior {
    pub dry_run: bool,
    pub prune: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedTreeSyncReport {
    pub target_dir: PathBuf,
    pub synced_files: Vec<String>,
    pub pruned_files: Vec<String>,
}

impl ManagedTreeSyncReport {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.synced_files.is_empty() && self.pruned_files.is_empty()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ManagedTreeManifest {
    #[serde(default = "manifest_version")]
    version: u8,
    #[serde(default)]
    managed_files: BTreeSet<String>,
}

const fn manifest_version() -> u8 {
    MANIFEST_VERSION
}

/// Collect source-to-target mappings for a managed directory tree.
///
/// # Errors
///
/// Returns an error if the source tree cannot be read or relative paths cannot
/// be computed.
pub fn collect_directory_tree_mappings(source_dir: &Path) -> Result<Vec<SourceFileMapping>> {
    if !source_dir.exists() {
        return Ok(Vec::new());
    }

    let mut mappings = Vec::new();
    collect_directory_tree_mappings_recursive(source_dir, source_dir, &mut mappings)?;
    mappings.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(mappings)
}

/// Synchronize a Claudius-managed target tree and optionally prune stale files.
///
/// # Errors
///
/// Returns an error if files cannot be copied, deleted, or if the manifest
/// cannot be read or written.
pub fn sync_managed_tree(
    target_dir: &Path,
    mappings: &[SourceFileMapping],
    behavior: SyncBehavior,
) -> Result<ManagedTreeSyncReport> {
    let manifest_path = manifest_path(target_dir);
    let previous_manifest = read_manifest(&manifest_path)?;

    let synced_files =
        mappings.iter().map(|mapping| mapping.relative_path.clone()).collect::<Vec<_>>();

    let current_files = synced_files.iter().cloned().collect::<BTreeSet<_>>();
    let pruned_files = if behavior.prune {
        previous_manifest
            .managed_files
            .difference(&current_files)
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if !behavior.dry_run {
        let should_prepare_target =
            !mappings.is_empty() || !previous_manifest.managed_files.is_empty();
        if should_prepare_target {
            fs::create_dir_all(target_dir).with_context(|| {
                format!("Failed to create target directory: {}", target_dir.display())
            })?;
        }

        for mapping in mappings {
            copy_mapping(target_dir, mapping)?;
        }

        if behavior.prune {
            for relative_path in &pruned_files {
                delete_managed_file(target_dir, relative_path)?;
            }
        }

        let next_managed_files = if behavior.prune {
            current_files
        } else {
            previous_manifest.managed_files.union(&current_files).cloned().collect()
        };
        write_manifest(&manifest_path, &next_managed_files)?;
    }

    Ok(ManagedTreeSyncReport { target_dir: target_dir.to_path_buf(), synced_files, pruned_files })
}

fn collect_directory_tree_mappings_recursive(
    root_dir: &Path,
    current_dir: &Path,
    mappings: &mut Vec<SourceFileMapping>,
) -> Result<()> {
    let mut entries = fs::read_dir(current_dir)
        .with_context(|| format!("Failed to read directory: {}", current_dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_directory_tree_mappings_recursive(root_dir, &path, mappings)?;
            continue;
        }

        if !path.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root_dir)
            .with_context(|| format!("Failed to compute relative path for {}", path.display()))?;
        let relative_path = normalize_relative_path(relative);
        mappings.push(SourceFileMapping { source_path: path, relative_path });
    }

    Ok(())
}

fn copy_mapping(target_dir: &Path, mapping: &SourceFileMapping) -> Result<()> {
    let destination = target_dir.join(&mapping.relative_path);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    fs::copy(&mapping.source_path, &destination).with_context(|| {
        format!(
            "Failed to copy file from {} to {}",
            mapping.source_path.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn delete_managed_file(target_dir: &Path, relative_path: &str) -> Result<()> {
    let file_path = target_dir.join(relative_path);
    match fs::remove_file(&file_path) {
        Ok(()) => remove_empty_parent_directories(target_dir, &file_path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to remove {}", file_path.display()));
        },
    }

    Ok(())
}

fn remove_empty_parent_directories(root_dir: &Path, file_path: &Path) -> Result<()> {
    let mut current = file_path.parent();
    while let Some(dir) = current {
        if dir == root_dir {
            break;
        }

        match fs::remove_dir(dir) {
            Ok(()) => current = dir.parent(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => current = dir.parent(),
            Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to remove directory {}", dir.display()));
            },
        }
    }

    Ok(())
}

fn read_manifest(manifest_path: &Path) -> Result<ManagedTreeManifest> {
    if !manifest_path.exists() {
        return Ok(ManagedTreeManifest::default());
    }

    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;
    let manifest: ManagedTreeManifest = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;

    Ok(manifest)
}

fn write_manifest(manifest_path: &Path, managed_files: &BTreeSet<String>) -> Result<()> {
    if managed_files.is_empty() {
        match fs::remove_file(manifest_path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to remove {}", manifest_path.display()));
            },
        }
        return Ok(());
    }

    if let Some(parent) = manifest_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let manifest =
        ManagedTreeManifest { version: MANIFEST_VERSION, managed_files: managed_files.clone() };
    let content =
        serde_json::to_string_pretty(&manifest).context("Failed to serialize sync manifest")?;
    fs::write(manifest_path, format!("{content}\n"))
        .with_context(|| format!("Failed to write {}", manifest_path.display()))?;

    Ok(())
}

fn manifest_path(target_dir: &Path) -> PathBuf {
    target_dir.join(MANIFEST_FILE_NAME)
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn sync_managed_tree_records_manifest_without_pruning() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).expect("create source");
        fs::write(source_dir.join("a.txt"), "alpha").expect("write source file");

        let mappings = collect_directory_tree_mappings(&source_dir).expect("collect mappings");
        let report = sync_managed_tree(
            &target_dir,
            &mappings,
            SyncBehavior { dry_run: false, prune: false },
        )
        .expect("sync should succeed");

        assert_eq!(report.synced_files, vec!["a.txt".to_string()]);
        assert!(report.pruned_files.is_empty());
        assert!(target_dir.join("a.txt").exists());
        assert!(target_dir.join(MANIFEST_FILE_NAME).exists());
    }

    #[test]
    fn sync_managed_tree_prunes_only_manifested_files() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).expect("create source");
        fs::create_dir_all(target_dir.join("manual")).expect("create manual dir");
        fs::write(source_dir.join("keep.txt"), "keep").expect("write source file");

        let mappings = collect_directory_tree_mappings(&source_dir).expect("collect mappings");
        sync_managed_tree(&target_dir, &mappings, SyncBehavior { dry_run: false, prune: false })
            .expect("initial sync should succeed");

        fs::write(target_dir.join("old.txt"), "old").expect("write old file");
        fs::write(target_dir.join("manual").join("note.txt"), "manual").expect("write note");
        write_manifest(
            &manifest_path(&target_dir),
            &BTreeSet::from(["keep.txt".to_string(), "old.txt".to_string()]),
        )
        .expect("write manifest");

        let report =
            sync_managed_tree(&target_dir, &mappings, SyncBehavior { dry_run: false, prune: true })
                .expect("prune sync should succeed");

        assert_eq!(report.pruned_files, vec!["old.txt".to_string()]);
        assert!(target_dir.join("keep.txt").exists());
        assert!(!target_dir.join("old.txt").exists());
        assert!(target_dir.join("manual").join("note.txt").exists());
    }

    #[test]
    fn sync_managed_tree_dry_run_does_not_modify_target() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");
        fs::create_dir_all(&source_dir).expect("create source");
        fs::write(source_dir.join("new.txt"), "new").expect("write source file");
        fs::create_dir_all(&target_dir).expect("create target");
        fs::write(target_dir.join("old.txt"), "old").expect("write target file");
        write_manifest(&manifest_path(&target_dir), &BTreeSet::from(["old.txt".to_string()]))
            .expect("write manifest");

        let mappings = collect_directory_tree_mappings(&source_dir).expect("collect mappings");
        let report =
            sync_managed_tree(&target_dir, &mappings, SyncBehavior { dry_run: true, prune: true })
                .expect("dry-run sync should succeed");

        assert_eq!(report.synced_files, vec!["new.txt".to_string()]);
        assert_eq!(report.pruned_files, vec!["old.txt".to_string()]);
        assert!(target_dir.join("old.txt").exists());
        assert!(!target_dir.join("new.txt").exists());
    }
}
