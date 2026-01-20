use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_yaml::{Mapping, Value};
use unicode_normalization::UnicodeNormalization;
use walkdir::WalkDir;

pub const MAX_SKILL_NAME_LENGTH: usize = 64;
pub const MAX_DESCRIPTION_LENGTH: usize = 1024;
pub const MAX_COMPATIBILITY_LENGTH: usize = 500;

const ALLOWED_FIELDS: [&str; 6] = [
    "name",
    "description",
    "license",
    "allowed-tools",
    "metadata",
    "compatibility",
];

const FIELD_ORDER: [&str; 6] = [
    "name",
    "description",
    "license",
    "compatibility",
    "allowed-tools",
    "metadata",
];

#[derive(Debug, Clone)]
pub struct SkillFile {
    pub dir_path: PathBuf,
    pub file_path: PathBuf,
    pub content: String,
}

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

pub fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|rel| rel.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

pub fn find_skill_md(skill_dir: &Path) -> Option<PathBuf> {
    for name in ["SKILL.md", "skill.md"] {
        let path = skill_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

pub fn parse_frontmatter(content: &str) -> Result<(BTreeMap<String, Value>, String), String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    if !content.starts_with("---") {
        return Err("SKILL.md must start with YAML frontmatter (---)".to_string());
    }

    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return Err("SKILL.md frontmatter not properly closed with ---".to_string());
    }

    let frontmatter_str = parts[1];
    let body = parts[2].to_string();

    let parsed: Value = serde_yaml::from_str(frontmatter_str)
        .map_err(|err| format!("Invalid YAML in frontmatter: {err}"))?;

    match parsed {
        Value::Mapping(map) => Ok((mapping_to_btreemap(map)?, body)),
        _ => Err("SKILL.md frontmatter must be a YAML mapping".to_string()),
    }
}

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

        if path.is_file() && path.file_name().map(|n| n.eq_ignore_ascii_case("skill.md")).unwrap_or(false) {
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

pub fn check_skill(skill: &SkillFile) -> Vec<String> {
    let mut errors = Vec::new();

    if !skill.dir_path.exists() {
        errors.push(format!("Path does not exist: {}", skill.dir_path.display()));
        return errors;
    }

    if !skill.dir_path.is_dir() {
        errors.push(format!("Not a directory: {}", skill.dir_path.display()));
        return errors;
    }

    if !skill.file_path.exists() {
        errors.push("Missing required file: SKILL.md".to_string());
        return errors;
    }

    if skill.file_path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md") {
        errors.push("SKILL.md should be uppercase".to_string());
    }

    match parse_frontmatter(&skill.content) {
        Ok((metadata, _body)) => {
            errors.extend(validate_metadata(&metadata, Some(&skill.dir_path)));
        }
        Err(err) => errors.push(err),
    }

    errors
}

pub fn fix_skill(skill: &SkillFile) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut changed = false;

    if !skill.dir_path.exists() || !skill.dir_path.is_dir() {
        return (false, vec![format!("Not a directory: {}", skill.dir_path.display())]);
    }

    let mut skill_path = find_skill_md(&skill.dir_path).unwrap_or_else(|| skill.file_path.clone());

    if skill_path.exists()
        && skill_path
            .file_name()
            .map(|n| n.eq_ignore_ascii_case("skill.md"))
            .unwrap_or(false)
        && skill_path.file_name().and_then(|n| n.to_str()) != Some("SKILL.md")
    {
        let new_path = skill_path.with_file_name("SKILL.md");
        if let Err(err) = fs::rename(&skill_path, &new_path) {
            return (
                false,
                vec![format!("Failed to rename {}: {err}", skill_path.display())],
            );
        }
        skill_path = new_path;
        changed = true;
    }

    if !skill_path.exists() {
        errors.push("Missing required file: SKILL.md".to_string());
        return (changed, errors);
    }

    let mut content = match fs::read_to_string(&skill_path) {
        Ok(data) => data,
        Err(err) => {
            errors.push(format!("Failed to read {}: {err}", skill_path.display()));
            return (changed, errors);
        }
    };

    if content.starts_with('\u{feff}') {
        content = content.trim_start_matches('\u{feff}').to_string();
        changed = true;
    }

    let mut metadata: BTreeMap<String, Value>;
    let mut body: String;

    if !content.starts_with("---") {
        metadata = BTreeMap::new();
        let dir_name = skill
            .dir_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        metadata.insert("name".to_string(), Value::String(dir_name));
        metadata.insert("description".to_string(), Value::String(derive_description(&content)));
        body = content.trim_matches('\n').to_string();
        changed = true;
    } else {
        match parse_frontmatter(&content) {
            Ok((parsed, parsed_body)) => {
                metadata = parsed;
                body = parsed_body.trim_matches('\n').to_string();
            }
            Err(err) => {
                errors.push(err);
                return (changed, errors);
            }
        }

        let dir_name = skill
            .dir_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let dir_name_norm: String = dir_name.nfkc().collect();

        match metadata.get("name") {
            Some(Value::String(name)) if !name.trim().is_empty() => {
                let name_norm: String = name.trim().nfkc().collect();
                if name_norm != dir_name_norm {
                    metadata.insert("name".to_string(), Value::String(dir_name.clone()));
                    changed = true;
                }
            }
            _ => {
                metadata.insert("name".to_string(), Value::String(dir_name.clone()));
                changed = true;
            }
        }

        match metadata.get("description") {
            Some(Value::String(desc)) if !desc.trim().is_empty() => {}
            _ => {
                metadata.insert("description".to_string(), Value::String(derive_description(&body)));
                changed = true;
            }
        }

        if let Some(Value::Mapping(map)) = metadata.get_mut("metadata") {
            if let Ok(normalized) = mapping_to_string_map(map) {
                let mut new_map = Mapping::new();
                for (key, value) in normalized {
                    new_map.insert(Value::String(key), Value::String(value));
                }
                *map = new_map;
                changed = true;
            }
        }
    }

    metadata.retain(|_, value| !matches!(value, Value::Null));

    let formatted = match format_frontmatter(&metadata) {
        Ok(result) => result,
        Err(err) => {
            errors.push(err);
            return (changed, errors);
        }
    };

    let mut new_content = format!("{}\n\n{}", formatted, body);
    new_content = new_content.trim_end().to_string();
    new_content.push('\n');

    if new_content != content {
        if let Err(err) = fs::write(&skill_path, new_content) {
            errors.push(format!("Failed to write {}: {err}", skill_path.display()));
            return (changed, errors);
        }
        changed = true;
    }

    (changed, errors)
}

pub fn validate_metadata(metadata: &BTreeMap<String, Value>, skill_dir: Option<&Path>) -> Vec<String> {
    let mut errors = Vec::new();

    let extra_fields: Vec<String> = metadata
        .keys()
        .filter(|key| !ALLOWED_FIELDS.contains(&key.as_str()))
        .cloned()
        .collect();
    if !extra_fields.is_empty() {
        errors.push(format!(
            "Unexpected fields in frontmatter: {}. Only {:?} are allowed.",
            extra_fields.join(", "),
            ALLOWED_FIELDS
        ));
    }

    match metadata.get("name") {
        Some(Value::String(name)) if !name.trim().is_empty() => {
            errors.extend(validate_name(name, skill_dir));
        }
        Some(_) => errors.push("Field 'name' must be a non-empty string".to_string()),
        None => errors.push("Missing required field in frontmatter: name".to_string()),
    }

    match metadata.get("description") {
        Some(Value::String(description)) if !description.trim().is_empty() => {
            errors.extend(validate_description(description));
        }
        Some(_) => errors.push("Field 'description' must be a non-empty string".to_string()),
        None => errors.push("Missing required field in frontmatter: description".to_string()),
    }

    if let Some(value) = metadata.get("compatibility") {
        match value {
            Value::String(text) => errors.extend(validate_compatibility(text)),
            _ => errors.push("Field 'compatibility' must be a string".to_string()),
        }
    }

    errors
}

fn validate_name(name: &str, skill_dir: Option<&Path>) -> Vec<String> {
    let mut errors = Vec::new();

    if name.trim().is_empty() {
        errors.push("Field 'name' must be a non-empty string".to_string());
        return errors;
    }

    let normalized: String = name.trim().nfkc().collect();

    if normalized.len() > MAX_SKILL_NAME_LENGTH {
        errors.push(format!(
            "Skill name '{}' exceeds {} character limit ({} chars)",
            normalized,
            MAX_SKILL_NAME_LENGTH,
            normalized.len()
        ));
    }

    if normalized != normalized.to_lowercase() {
        errors.push(format!("Skill name '{}' must be lowercase", normalized));
    }

    if normalized.starts_with('-') || normalized.ends_with('-') {
        errors.push("Skill name cannot start or end with a hyphen".to_string());
    }

    if normalized.contains("--") {
        errors.push("Skill name cannot contain consecutive hyphens".to_string());
    }

    if !normalized
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-')
    {
        errors.push(format!(
            "Skill name '{}' contains invalid characters. Only letters, digits, and hyphens are allowed.",
            normalized
        ));
    }

    if let Some(dir) = skill_dir {
        let dir_name = dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let dir_norm: String = dir_name.nfkc().collect();
        if dir_norm != normalized {
            errors.push(format!(
                "Directory name '{}' must match skill name '{}'",
                dir_name, normalized
            ));
        }
    }

    errors
}

fn validate_description(description: &str) -> Vec<String> {
    let mut errors = Vec::new();

    if description.trim().is_empty() {
        errors.push("Field 'description' must be a non-empty string".to_string());
        return errors;
    }

    if description.len() > MAX_DESCRIPTION_LENGTH {
        errors.push(format!(
            "Description exceeds {} character limit ({} chars)",
            MAX_DESCRIPTION_LENGTH,
            description.len()
        ));
    }

    errors
}

fn validate_compatibility(compatibility: &str) -> Vec<String> {
    let mut errors = Vec::new();

    if compatibility.len() > MAX_COMPATIBILITY_LENGTH {
        errors.push(format!(
            "Compatibility exceeds {} character limit ({} chars)",
            MAX_COMPATIBILITY_LENGTH,
            compatibility.len()
        ));
    }

    errors
}

fn derive_description(body: &str) -> String {
    let mut in_code_block = false;
    for raw in body.lines() {
        let line = raw.trim();
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block || line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        return line.to_string();
    }

    for raw in body.lines() {
        let line = raw.trim();
        if line.starts_with('#') {
            return line.trim_start_matches('#').trim().to_string();
        }
    }

    "Describe what this skill does and when to use it".to_string()
}

fn format_frontmatter(metadata: &BTreeMap<String, Value>) -> Result<String, String> {
    let mut lines: Vec<String> = vec!["---".to_string()];

    for field in FIELD_ORDER {
        let Some(value) = metadata.get(field) else {
            continue;
        };

        if field == "metadata" {
            match value {
                Value::Mapping(map) => {
                    if map.is_empty() {
                        continue;
                    }
                    let normalized = mapping_to_string_map(map)?;
                    lines.push("metadata:".to_string());
                    for (key, val) in normalized {
                        lines.push(format!(
                            "  {}: {}",
                            format_key(&key),
                            format_scalar(&val)
                        ));
                    }
                }
                _ => {
                    let scalar = value_to_string(value)?;
                    lines.push(format!("metadata: {}", format_scalar(&scalar)));
                }
            }
            continue;
        }

        let scalar = value_to_string(value)?;
        lines.push(format!("{}: {}", field, format_scalar(&scalar)));
    }

    let unknown_fields: Vec<String> = metadata
        .keys()
        .filter(|key| !ALLOWED_FIELDS.contains(&key.as_str()))
        .cloned()
        .collect();

    for key in unknown_fields {
        let value = metadata.get(&key).expect("key exists");
        match value {
            Value::Mapping(map) => {
                let normalized = mapping_to_string_map(map)?;
                lines.push(format!("{}:", format_key(&key)));
                for (sub_key, sub_val) in normalized {
                    lines.push(format!(
                        "  {}: {}",
                        format_key(&sub_key),
                        format_scalar(&sub_val)
                    ));
                }
            }
            _ => {
                let scalar = value_to_string(value)?;
                lines.push(format!("{}: {}", format_key(&key), format_scalar(&scalar)));
            }
        }
    }

    lines.push("---".to_string());
    Ok(lines.join("\n"))
}

fn value_to_string(value: &Value) -> Result<String, String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(num) => Ok(num.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Ok("null".to_string()),
        _ => Err("Unsupported YAML value type for formatting".to_string()),
    }
}

fn mapping_to_btreemap(map: Mapping) -> Result<BTreeMap<String, Value>, String> {
    let mut result = BTreeMap::new();
    for (key, value) in map {
        let key_str = match key {
            Value::String(text) => text,
            _ => return Err("Frontmatter keys must be strings".to_string()),
        };
        result.insert(key_str, value);
    }
    Ok(result)
}

fn mapping_to_string_map(map: &Mapping) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for (key, value) in map {
        let key_str = value_to_string(key)?;
        let value_str = value_to_string(value)?;
        result.insert(key_str, value_str);
    }
    Ok(result)
}

fn format_scalar(value: &str) -> String {
    let mut needs_quotes = value.is_empty()
        || value.starts_with(' ')
        || value.starts_with('\t')
        || value.ends_with(' ')
        || value.ends_with('\t')
        || value.contains('\n')
        || value.contains('\r')
        || value.contains('\t')
        || value.contains(':')
        || value.contains('#');

    if let Some(first) = value.chars().next() {
        if matches!(
            first,
            '-' | '?' | '!' | '@' | '&' | '*' | '>' | '|' | '{' | '}' | '[' | ']' | ','
        ) {
            needs_quotes = true;
        }
    }

    if needs_quotes {
        json_quote(value)
    } else {
        value.to_string()
    }
}

fn format_key(value: &str) -> String {
    if is_simple_key(value) {
        value.to_string()
    } else {
        json_quote(value)
    }
}

fn is_simple_key(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
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
    out.push('"');
    out
}

fn discover_skills(root: &Path) -> Vec<SkillFile> {
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

fn discover_skills_in_dir(root: &Path) -> Vec<SkillFile> {
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
    let mut args = vec!["-C", root.to_string_lossy().as_ref(), "ls-files", "-z"];
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
