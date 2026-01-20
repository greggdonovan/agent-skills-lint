use std::path::PathBuf;

use clap::{Parser, Subcommand};

use agent_skills_lint::{check_skill, collect_skill_files, display_path, fix_skill, repo_root};

#[derive(Parser)]
#[command(
    name = "agent-skills-lint",
    version,
    about = "Lint and format Agent Skills"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check skill formatting and structure
    Check {
        /// Paths to check (directories or SKILL.md files)
        paths: Vec<PathBuf>,
    },
    /// Fix skill formatting and frontmatter
    Fix {
        /// Paths to fix (directories or SKILL.md files)
        paths: Vec<PathBuf>,

        /// Preview changes without writing to disk
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Check { paths } => run_check(paths),
        Command::Fix { paths, dry_run } => run_fix(paths, dry_run),
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
            eprintln!("Validation failed for {rel}:");
            for error in errors {
                eprintln!("  - {error}");
            }
            failed = true;
        }
    }

    i32::from(failed)
}

fn run_fix(paths: Vec<PathBuf>, dry_run: bool) -> i32 {
    let skill_files = collect_skill_files(&paths);
    if skill_files.is_empty() {
        eprintln!("No SKILL.md files found.");
        return 1;
    }

    let root = repo_root();
    let mut failed = false;

    for skill in skill_files {
        let result = fix_skill(&skill, dry_run);
        let rel = display_path(&skill.dir_path, &root);

        if result.changed {
            if dry_run {
                println!("Would fix {rel}");
                if let Some(content) = &result.new_content {
                    // Show a preview of the frontmatter
                    if let Some(end) = content.find("\n---\n") {
                        let preview = &content[..end + 4];
                        println!("{preview}");
                    }
                }
            } else {
                println!("Fixed {rel}");
            }
        }

        if !result.errors.is_empty() {
            eprintln!("Unable to fully fix {rel}:");
            for error in result.errors {
                eprintln!("  - {error}");
            }
            failed = true;
        }
    }

    i32::from(failed)
}
