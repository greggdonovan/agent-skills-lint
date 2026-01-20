use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn readme_is_executable() {
    let bin = assert_cmd::cargo::cargo_bin!("agent-skills-lint");
    Command::new("sh")
        .arg("README.md")
        .env("AGENT_SKILLS_LINT_BIN", bin)
        .assert()
        .success();
}
