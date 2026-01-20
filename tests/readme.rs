use assert_cmd::prelude::*;
use std::process::Command;

#[test]
#[cfg_attr(windows, ignore)]
fn readme_is_executable() {
    let bin = assert_cmd::cargo::cargo_bin!("agent-skills-lint");
    Command::new("bash")
        .arg("README.md")
        .env("AGENT_SKILLS_LINT_BIN", bin)
        .assert()
        .success();
}
