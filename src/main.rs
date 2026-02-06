use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

use agent_skills_lint::{
    check_skill, collect_skill_files, display_path, fix_skill, repo_root, FixError, ValidationError,
};

const OUTPUT_VERSION: &str = env!("CARGO_PKG_VERSION");

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

        /// Output in JSON format (machine-readable)
        #[arg(long)]
        json: bool,

        /// Suppress non-error output
        #[arg(long, short = 'q')]
        quiet: bool,
    },
    /// Fix skill formatting and frontmatter
    Fix {
        /// Paths to fix (directories or SKILL.md files)
        paths: Vec<PathBuf>,

        /// Preview changes without writing to disk
        #[arg(long, short = 'n')]
        dry_run: bool,

        /// Output in JSON format (machine-readable)
        #[arg(long)]
        json: bool,

        /// Suppress non-error output
        #[arg(long, short = 'q')]
        quiet: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let exit_code = match cli.command {
        Command::Check { paths, json, quiet } => run_check(paths, json, quiet),
        Command::Fix {
            paths,
            dry_run,
            json,
            quiet,
        } => run_fix(paths, dry_run, json, quiet),
    };

    std::process::exit(exit_code);
}

fn run_check(paths: Vec<PathBuf>, json: bool, quiet: bool) -> i32 {
    let root = repo_root();
    let path_issues = collect_check_path_issues(&paths, &root);
    let skill_files = collect_skill_files(&paths);

    if skill_files.is_empty() && path_issues.is_empty() {
        if json {
            println!(
                r#"{{"version":"{OUTPUT_VERSION}","skills":[],"error":"No SKILL.md files found"}}"#
            );
        } else {
            eprintln!("No SKILL.md files found.");
        }
        return 1;
    }

    let mut failed = !path_issues.is_empty();
    let mut json_results: Vec<String> = Vec::new();

    for issue in path_issues {
        if json {
            let error_str = format_validation_error(&issue.error);
            json_results.push(format!(
                r#"{{"path":"{}","status":"invalid","errors":[{}]}}"#,
                escape_json(&issue.path),
                error_str
            ));
        } else if !quiet {
            eprintln!("Validation failed for {}:", issue.path);
            eprintln!("  - {}", issue.error);
        }
    }

    for skill in skill_files {
        let errors = check_skill(&skill);
        let rel = display_path(&skill.dir_path, &root);

        if json {
            let error_strs: Vec<String> = errors.iter().map(format_validation_error).collect();
            let status = if errors.is_empty() {
                "valid"
            } else {
                "invalid"
            };
            json_results.push(format!(
                r#"{{"path":"{}","status":"{}","errors":{}}}"#,
                escape_json(&rel),
                status,
                format_json_array(&error_strs)
            ));
        }

        if !errors.is_empty() {
            if !json && !quiet {
                eprintln!("Validation failed for {rel}:");
                for error in &errors {
                    eprintln!("  - {error}");
                }
            }
            failed = true;
        }
    }

    if json {
        println!(
            r#"{{"version":"{}","skills":[{}]}}"#,
            OUTPUT_VERSION,
            json_results.join(",")
        );
    }

    i32::from(failed)
}

fn run_fix(paths: Vec<PathBuf>, dry_run: bool, json: bool, quiet: bool) -> i32 {
    let root = repo_root();
    let path_issues = collect_fix_path_issues(&paths, &root);
    let skill_files = collect_skill_files(&paths);

    if skill_files.is_empty() && path_issues.is_empty() {
        if json {
            println!(
                r#"{{"version":"{OUTPUT_VERSION}","skills":[],"error":"No SKILL.md files found"}}"#
            );
        } else {
            eprintln!("No SKILL.md files found.");
        }
        return 1;
    }

    let mut failed = !path_issues.is_empty();
    let mut json_results: Vec<String> = Vec::new();

    for issue in path_issues {
        if json {
            let error_str = format_fix_error(&issue.error);
            json_results.push(format!(
                r#"{{"path":"{}","status":"error","changed":false,"errors":[{}]}}"#,
                escape_json(&issue.path),
                error_str
            ));
        } else if !quiet {
            eprintln!("Unable to fully fix {}:", issue.path);
            eprintln!("  - {}", issue.error);
        }
    }

    for skill in skill_files {
        let result = fix_skill(&skill, dry_run);
        let rel = display_path(&skill.dir_path, &root);

        if json {
            let error_strs: Vec<String> = result.errors.iter().map(format_fix_error).collect();
            let status = if result.errors.is_empty() {
                if result.changed {
                    "fixed"
                } else {
                    "unchanged"
                }
            } else {
                "error"
            };
            json_results.push(format!(
                r#"{{"path":"{}","status":"{}","changed":{},"errors":{}}}"#,
                escape_json(&rel),
                status,
                result.changed,
                format_json_array(&error_strs)
            ));
        } else {
            if result.changed && !quiet {
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
                for error in &result.errors {
                    eprintln!("  - {error}");
                }
            }
        }

        if !result.errors.is_empty() {
            failed = true;
        }
    }

    if json {
        println!(
            r#"{{"version":"{}","skills":[{}]}}"#,
            OUTPUT_VERSION,
            json_results.join(",")
        );
    }

    i32::from(failed)
}

fn format_validation_error(error: &ValidationError) -> String {
    format!(
        r#"{{"code":"{}","message":"{}"}}"#,
        error_code(error),
        escape_json(&error.to_string())
    )
}

fn format_fix_error(error: &FixError) -> String {
    format!(
        r#"{{"code":"{}","message":"{}"}}"#,
        fix_error_code(error),
        escape_json(&error.to_string())
    )
}

const fn error_code(error: &ValidationError) -> &'static str {
    match error {
        ValidationError::MissingFile(_) => "missing-file",
        ValidationError::PathNotFound(_) => "path-not-found",
        ValidationError::NotADirectory(_) => "not-a-directory",
        ValidationError::NotUppercase => "not-uppercase",
        ValidationError::Parse(_) => "parse-error",
        ValidationError::MissingField(_) => "missing-field",
        ValidationError::EmptyField(_) => "empty-field",
        ValidationError::InvalidType(_) => "invalid-type",
        ValidationError::MetadataNotMapping => "metadata-not-mapping",
        ValidationError::MetadataNonStringKey => "metadata-non-string-key",
        ValidationError::MetadataNonStringValue { .. } => "metadata-non-string-value",
        ValidationError::NameTooLong { .. } => "name-too-long",
        ValidationError::NameNotLowercase(_) => "name-not-lowercase",
        ValidationError::NameInvalidHyphen => "name-invalid-hyphen",
        ValidationError::NameConsecutiveHyphens => "name-consecutive-hyphens",
        ValidationError::NameInvalidChars(_) => "name-invalid-chars",
        ValidationError::NameMismatch { .. } => "name-mismatch",
        ValidationError::DescriptionTooLong { .. } => "description-too-long",
        ValidationError::CompatibilityTooLong { .. } => "compatibility-too-long",
        ValidationError::EmptyLicense => "empty-license",
        ValidationError::InvalidToolSpec { .. } => "invalid-tool-spec",
        ValidationError::InvalidToolArrayItem { .. } => "invalid-tool-array-item",
        ValidationError::InvalidToolsType => "invalid-tools-type",
        ValidationError::UnexpectedFields { .. } => "unexpected-fields",
    }
}

const fn fix_error_code(error: &FixError) -> &'static str {
    match error {
        FixError::PathNotFound(_) => "path-not-found",
        FixError::NotADirectory(_) => "not-a-directory",
        FixError::RenameFailed { .. } => "rename-failed",
        FixError::MissingFile => "missing-file",
        FixError::WriteFailed { .. } => "write-failed",
        FixError::Parse(_) => "parse-error",
        FixError::UnsupportedValueType => "unsupported-value-type",
    }
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(ch),
        }
    }
    out
}

fn format_json_array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

struct CheckPathIssue {
    path: String,
    error: ValidationError,
}

struct FixPathIssue {
    path: String,
    error: FixError,
}

fn collect_check_path_issues(paths: &[PathBuf], root: &Path) -> Vec<CheckPathIssue> {
    let mut issues = Vec::new();
    for target in paths {
        let resolved = resolve_target_path(target, root);
        let display = display_path(&resolved, root);

        if !resolved.exists() {
            issues.push(CheckPathIssue {
                path: display,
                error: ValidationError::PathNotFound(resolved.display().to_string()),
            });
            continue;
        }

        if resolved.is_file() && !is_skill_markdown_file(&resolved) {
            issues.push(CheckPathIssue {
                path: display,
                error: ValidationError::NotADirectory(resolved.display().to_string()),
            });
        }
    }
    issues
}

fn collect_fix_path_issues(paths: &[PathBuf], root: &Path) -> Vec<FixPathIssue> {
    let mut issues = Vec::new();
    for target in paths {
        let resolved = resolve_target_path(target, root);
        let display = display_path(&resolved, root);

        if !resolved.exists() {
            issues.push(FixPathIssue {
                path: display,
                error: FixError::PathNotFound(resolved),
            });
            continue;
        }

        if resolved.is_file() && !is_skill_markdown_file(&resolved) {
            issues.push(FixPathIssue {
                path: display,
                error: FixError::NotADirectory(resolved),
            });
        }
    }
    issues
}

fn resolve_target_path(target: &Path, root: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    }
}

fn is_skill_markdown_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("skill.md"))
}
