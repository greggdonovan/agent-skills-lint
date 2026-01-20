//! Skill file discovery utilities.
//!
//! This module provides functions for finding SKILL.md files in a repository,
//! either by explicit paths or through automatic discovery using git or filesystem
//! traversal.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use walkdir::WalkDir;

use crate::skill::SkillFile;

/// Find the repository root by looking for a git directory.
///
/// Falls back to the current working directory if not in a git repository.
pub fn repo_root() -> PathBuf {
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return PathBuf::from(path);
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Convert a path to a display-friendly relative path.
///
/// Makes paths relative to the repository root for cleaner output.
pub fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| rel.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

/// Find the SKILL.md file in a directory.
///
/// Prefers uppercase `SKILL.md` over lowercase `skill.md`.
pub fn find_skill_md(skill_dir: &Path) -> Option<PathBuf> {
    for name in ["SKILL.md", "skill.md"] {
        let path = skill_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Collect skill files from the given paths.
///
/// If paths is empty, discovers all skills in the repository.
/// If a path is a directory containing a SKILL.md, uses that.
/// If a path is a directory without a SKILL.md, searches recursively.
/// If a path is a SKILL.md file directly, uses that.
pub fn collect_skill_files(paths: &[PathBuf]) -> Vec<SkillFile> {
    let root = repo_root();

    if paths.is_empty() {
        return discover_skills(&root);
    }

    let mut skill_files = Vec::new();

    for target in paths {
        let mut path = target.clone();
        if !path.is_absolute() {
            path = root.join(path);
        }

        if path.is_dir() {
            if let Some(skill_md) = find_skill_md(&path) {
                if let Ok(content) = fs::read_to_string(&skill_md) {
                    skill_files.push(SkillFile {
                        dir_path: path,
                        file_path: skill_md,
                        content,
                    });
                }
            } else {
                skill_files.extend(discover_skills_in_dir(&path));
            }
            continue;
        }

        if path.is_file()
            && path
                .file_name()
                .map(|n| n.eq_ignore_ascii_case("skill.md"))
                .unwrap_or(false)
        {
            if let Ok(content) = fs::read_to_string(&path) {
                skill_files.push(SkillFile {
                    dir_path: path.parent().unwrap_or(&path).to_path_buf(),
                    file_path: path,
                    content,
                });
            }
        }
    }

    skill_files
}

/// Discover all skill files in a repository.
///
/// Uses git ls-files for efficiency, falling back to `WalkDir` for non-git repos.
pub fn discover_skills(root: &Path) -> Vec<SkillFile> {
    let mut map: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();

    if let Ok(paths) = git_ls_files(root, false) {
        add_skill_paths(&mut map, root, &paths);
    }
    if let Ok(paths) = git_ls_files(root, true) {
        add_skill_paths(&mut map, root, &paths);
    }

    if map.is_empty() {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy();
            if file_name == "SKILL.md" || file_name == "skill.md" {
                let path = entry.into_path();
                map.entry(path.parent().unwrap_or(root).to_path_buf())
                    .or_insert(path);
            }
        }
    }

    map.into_iter()
        .filter_map(|(dir, file)| {
            fs::read_to_string(&file).ok().map(|content| SkillFile {
                dir_path: dir,
                file_path: file,
                content,
            })
        })
        .collect()
}

/// Discover skill files in a specific directory (non-git).
pub fn discover_skills_in_dir(root: &Path) -> Vec<SkillFile> {
    let mut map: BTreeMap<PathBuf, PathBuf> = BTreeMap::new();

    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy();
        if file_name == "SKILL.md" || file_name == "skill.md" {
            let path = entry.into_path();
            map.entry(path.parent().unwrap_or(root).to_path_buf())
                .or_insert(path);
        }
    }

    map.into_iter()
        .filter_map(|(dir, file)| {
            fs::read_to_string(&file).ok().map(|content| SkillFile {
                dir_path: dir,
                file_path: file,
                content,
            })
        })
        .collect()
}

fn git_ls_files(root: &Path, untracked: bool) -> Result<Vec<PathBuf>, String> {
    let root_str = root.to_string_lossy();
    let mut args = vec!["-C", root_str.as_ref(), "ls-files", "-z"];
    if untracked {
        args.push("--others");
        args.push("--exclude-standard");
    }

    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|err| format!("Failed to run git ls-files: {err}"))?;

    if !output.status.success() {
        return Err("git ls-files failed".to_string());
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    Ok(raw
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(PathBuf::from)
        .collect())
}

fn add_skill_paths(map: &mut BTreeMap<PathBuf, PathBuf>, root: &Path, paths: &[PathBuf]) {
    for rel in paths {
        let file_name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "SKILL.md" {
            let full = root.join(rel);
            map.insert(full.parent().unwrap_or(root).to_path_buf(), full);
        }
    }
    for rel in paths {
        let file_name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == "skill.md" {
            let full = root.join(rel);
            map.entry(full.parent().unwrap_or(root).to_path_buf())
                .or_insert(full);
        }
    }
}

/// Extract the directory name from a path.
///
/// Helper function to reduce code duplication.
pub fn get_dir_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default()
}
