use assert_cmd::prelude::*;
use predicates::str::contains;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn write_skill(dir: &Path, filename: &str, content: &str) {
    fs::write(dir.join(filename), content).expect("write skill file");
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

    Command::new(assert_cmd::cargo::cargo_bin!("agent-skills-lint"))
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

    Command::new(assert_cmd::cargo::cargo_bin!("agent-skills-lint"))
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

    Command::new(assert_cmd::cargo::cargo_bin!("agent-skills-lint"))
        .args(["fix", skill_dir.to_str().unwrap()])
        .assert()
        .success();

    assert!(skill_dir.join("SKILL.md").exists());

    Command::new(assert_cmd::cargo::cargo_bin!("agent-skills-lint"))
        .args(["check", skill_dir.to_str().unwrap()])
        .assert()
        .success();
}
