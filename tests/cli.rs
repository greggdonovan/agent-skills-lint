use assert_cmd::prelude::*;
use predicates::str::{contains, is_empty};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn write_skill(dir: &Path, filename: &str, content: &str) {
    fs::write(dir.join(filename), content).expect("write skill file");
}

fn bin() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("agent-skills-lint"))
}

#[test]
fn cli_check_success() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("good-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(
        &skill_dir,
        "SKILL.md",
        "---\nname: good-skill\ndescription: A test skill\n---\nBody\n",
    );

    bin()
        .args(["check", skill_dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn cli_check_failure() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("bad-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(&skill_dir, "SKILL.md", "# Missing frontmatter\n");

    bin()
        .args(["check", skill_dir.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("must start with YAML frontmatter"));
}

#[test]
fn cli_fix_then_check() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("fix-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(
        &skill_dir,
        "skill.md",
        "# Title\n\nUse this skill to do X.\n",
    );

    bin()
        .args(["fix", skill_dir.to_str().unwrap()])
        .assert()
        .success();

    assert!(skill_dir.join("SKILL.md").exists());

    bin()
        .args(["check", skill_dir.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn cli_fix_dry_run() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("dry-run-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(
        &skill_dir,
        "skill.md",
        "# Title\n\nUse this skill for testing.\n",
    );

    // Dry run should report would fix but not actually rename
    bin()
        .args(["fix", "--dry-run", skill_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains("Would fix"));

    // File should still be lowercase
    let entries: Vec<_> = fs::read_dir(&skill_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert!(
        entries.iter().any(|name| name == "skill.md"),
        "Expected skill.md to still exist (not renamed), found: {entries:?}"
    );
}

#[test]
fn cli_check_no_skills_found() {
    let dir = TempDir::new().expect("temp dir");
    let empty_dir = dir.path().join("empty");
    fs::create_dir_all(&empty_dir).expect("mkdir");

    bin()
        .args(["check", empty_dir.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("No SKILL.md files found"));
}

#[test]
fn cli_check_quiet() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("quiet-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(&skill_dir, "SKILL.md", "# Missing frontmatter\n");

    // With --quiet, stdout should be empty (errors still go to stderr)
    bin()
        .args(["check", "--quiet", skill_dir.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(is_empty());
}

#[test]
fn cli_check_json() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("json-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(&skill_dir, "SKILL.md", "# Missing frontmatter\n");

    bin()
        .args(["check", "--json", skill_dir.to_str().unwrap()])
        .assert()
        .failure()
        .stdout(contains(format!(r#""version":"{VERSION}""#)))
        .stdout(contains(r#""status":"invalid""#))
        .stdout(contains(r#""code":"parse-error""#));
}

#[test]
fn cli_fix_json() {
    let dir = TempDir::new().expect("temp dir");
    let skill_dir = dir.path().join("json-fix-skill");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    // Use already-normalized format: blank line between frontmatter and body, trailing newline
    write_skill(
        &skill_dir,
        "SKILL.md",
        "---\nname: \"json-fix-skill\"\ndescription: \"A test\"\n---\n\nBody\n",
    );

    bin()
        .args(["fix", "--json", skill_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(contains(format!(r#""version":"{VERSION}""#)))
        .stdout(contains(r#""status":"unchanged""#));
}

#[test]
fn cli_check_nonexistent_path_reports_path_error() {
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("missing");

    bin()
        .args(["check", missing.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("Path does not exist:"));
}

#[test]
fn cli_fix_nonexistent_path_reports_path_error() {
    let dir = TempDir::new().expect("temp dir");
    let missing = dir.path().join("missing");

    bin()
        .args(["fix", missing.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(contains("Path does not exist:"));
}

#[test]
fn cli_check_discovers_mixed_case_skill_file_names_in_git_repos() {
    let dir = TempDir::new().expect("temp dir");
    Command::new("git")
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();

    let skill_dir = dir.path().join("mixed-case");
    fs::create_dir_all(&skill_dir).expect("mkdir");
    write_skill(
        &skill_dir,
        "Skill.md",
        "---\nname: mixed-case\ndescription: A test skill\n---\nBody\n",
    );

    bin()
        .current_dir(dir.path())
        .arg("check")
        .assert()
        .failure()
        .stderr(contains("SKILL.md should be uppercase"));
}
