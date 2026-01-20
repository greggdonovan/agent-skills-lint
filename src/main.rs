use std::path::PathBuf;

use clap::{Parser, Subcommand};

use agent_skills_lint::{
    check_skill, collect_skill_files, display_path, fix_skill, repo_root,
};

#[derive(Parser)]
#[command(name = "agent-skills-lint", version, about = "Lint and format Agent Skills")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check skill formatting and structure
    Check { paths: Vec<PathBuf> },
    /// Fix skill formatting and frontmatter
    Fix { paths: Vec<PathBuf> },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Check { paths } => run_check(paths),
        Command::Fix { paths } => run_fix(paths),
    };

    std::process::exit(exit_code);
}

fn run_check(paths: Vec<PathBuf>) -> i32 {
    let skill_files = collect_skill_files(&paths);
    if skill_files.is_empty() {
        eprintln!("No SKILL.md files found.");
        return 1;
    }

    let root = repo_root();
    let mut failed = false;

    for skill in skill_files {
        let errors = check_skill(&skill);
        if !errors.is_empty() {
            let rel = display_path(&skill.dir_path, &root);
            eprintln!("Validation failed for {}:", rel);
            for error in errors {
                eprintln!("  - {}", error);
            }
            failed = true;
        }
    }

    if failed { 1 } else { 0 }
}

fn run_fix(paths: Vec<PathBuf>) -> i32 {
    let skill_files = collect_skill_files(&paths);
    if skill_files.is_empty() {
        eprintln!("No SKILL.md files found.");
        return 1;
    }

    let root = repo_root();
    let mut failed = false;

    for skill in skill_files {
        let (changed, errors) = fix_skill(&skill);
        if changed {
            let rel = display_path(&skill.dir_path, &root);
            println!("Fixed {}", rel);
        }
        if !errors.is_empty() {
            let rel = display_path(&skill.dir_path, &root);
            eprintln!("Unable to fully fix {}:", rel);
            for error in errors {
                eprintln!("  - {}", error);
            }
            failed = true;
        }
    }

    if failed { 1 } else { 0 }
}
