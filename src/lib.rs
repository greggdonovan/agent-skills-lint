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
    let body = parts[2].trim().to_string();

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
    let body: String;

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

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use serde_yaml::Value;
    use tempfile::TempDir;

    fn temp_skill_dir(name: &str) -> TempDir {
        let dir = TempDir::new().expect("temp dir");
        let skill_dir = dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        dir
    }

    fn write_skill(dir: &Path, filename: &str, content: &str) -> PathBuf {
        let path = dir.join(filename);
        fs::write(&path, content).expect("write skill file");
        path
    }

    fn base_metadata() -> BTreeMap<String, Value> {
        let mut metadata = BTreeMap::new();
        metadata.insert("name".to_string(), Value::String("my-skill".to_string()));
        metadata.insert(
            "description".to_string(),
            Value::String("A test skill".to_string()),
        );
        metadata
    }

    #[test]
    fn parse_frontmatter_valid() {
        let content = "---\nname: my-skill\ndescription: A test skill\n---\n# Title\n\nBody\n";
        let (metadata, body) = parse_frontmatter(content).expect("frontmatter parsed");
        assert_eq!(
            metadata.get("name"),
            Some(&Value::String("my-skill".to_string()))
        );
        assert_eq!(body, "# Title\n\nBody");
    }

    #[test]
    fn parse_frontmatter_bom() {
        let content = "\u{feff}---\nname: my-skill\ndescription: A test skill\n---\nBody";
        let (metadata, body) = parse_frontmatter(content).expect("frontmatter parsed");
        assert_eq!(
            metadata.get("name"),
            Some(&Value::String("my-skill".to_string()))
        );
        assert_eq!(body, "Body");
    }

    #[test]
    fn parse_frontmatter_errors() {
        let missing = "No frontmatter here";
        assert!(parse_frontmatter(missing)
            .unwrap_err()
            .contains("must start with YAML frontmatter"));

        let unclosed = "---\nname: my-skill\ndescription: A test skill\n";
        assert!(parse_frontmatter(unclosed)
            .unwrap_err()
            .contains("not properly closed"));

        let invalid = "---\nname: [invalid\ndescription: broken\n---\nBody";
        assert!(parse_frontmatter(invalid)
            .unwrap_err()
            .contains("Invalid YAML"));

        let non_mapping = "---\n- just\n- a\n- list\n---\nBody";
        assert!(parse_frontmatter(non_mapping)
            .unwrap_err()
            .contains("YAML mapping"));
    }

    #[test]
    fn parse_frontmatter_non_string_key() {
        let content = "---\n1: a\nname: my-skill\ndescription: A test skill\n---\nBody";
        let err = parse_frontmatter(content).unwrap_err();
        assert!(err.contains("Frontmatter keys must be strings"));
    }

    #[test]
    fn find_skill_md_prefers_uppercase() {
        let dir = TempDir::new().expect("temp dir");
        let skill_dir = dir.path().join("skill");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        write_skill(&skill_dir, "skill.md", "lowercase");
        write_skill(&skill_dir, "SKILL.md", "uppercase");

        let found = find_skill_md(&skill_dir).expect("find skill");
        assert_eq!(found.file_name().unwrap().to_string_lossy(), "SKILL.md");
    }

    #[test]
    fn check_skill_missing_file() {
        let dir = temp_skill_dir("missing-skill");
        let skill_dir = dir.path().join("missing-skill");
        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_dir.join("SKILL.md"),
            content: String::new(),
        };
        let errors = check_skill(&skill);
        assert!(errors.iter().any(|err| err.contains("Missing required file")));
    }

    #[test]
    fn validate_metadata_missing_fields() {
        let mut metadata = base_metadata();
        metadata.remove("name");
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("Missing required field")));

        let mut metadata = base_metadata();
        metadata.remove("description");
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("Missing required field")));
    }

    #[test]
    fn validate_name_rules() {
        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("MySkill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("must be lowercase")));

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("-my-skill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("cannot start or end")));

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("my--skill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("consecutive hyphens")));

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("my_skill".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("invalid characters")));

        let long_name = "a".repeat(MAX_SKILL_NAME_LENGTH + 1);
        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String(long_name));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("character limit")));
    }

    #[test]
    fn validate_name_directory_match_and_i18n() {
        let dir = TempDir::new().expect("temp dir");
        let name = "caf\u{00e9}";
        let skill_dir = dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create dir");

        let mut metadata = base_metadata();
        metadata.insert(
            "name".to_string(),
            Value::String("cafe\u{0301}".to_string()),
        );
        let errors = validate_metadata(&metadata, Some(&skill_dir));
        assert!(errors.is_empty());

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String("wrong-name".to_string()));
        let errors = validate_metadata(&metadata, Some(&skill_dir));
        assert!(errors.iter().any(|err| err.contains("must match skill name")));
    }

    #[test]
    fn validate_i18n_case() {
        let dir = TempDir::new().expect("temp dir");
        let name = "\u{6280}\u{80fd}";
        let skill_dir = dir.path().join(name);
        fs::create_dir_all(&skill_dir).expect("create dir");

        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String(name.to_string()));
        let errors = validate_metadata(&metadata, Some(&skill_dir));
        assert!(errors.is_empty());

        let upper = "\u{041d}\u{0410}\u{0412}\u{042b}\u{041a}";
        let mut metadata = base_metadata();
        metadata.insert("name".to_string(), Value::String(upper.to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("must be lowercase")));
    }

    #[test]
    fn validate_description_and_compatibility_limits() {
        let mut metadata = base_metadata();
        metadata.insert(
            "description".to_string(),
            Value::String("x".repeat(MAX_DESCRIPTION_LENGTH + 1)),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("Description exceeds")));

        let mut metadata = base_metadata();
        metadata.insert(
            "compatibility".to_string(),
            Value::String("x".repeat(MAX_COMPATIBILITY_LENGTH + 1)),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("Compatibility exceeds")));

        let mut metadata = base_metadata();
        metadata.insert("compatibility".to_string(), Value::Number(1.into()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors
            .iter()
            .any(|err| err.contains("compatibility") && err.contains("string")));
    }

    #[test]
    fn validate_allowed_fields() {
        let mut metadata = base_metadata();
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(jq:*)".to_string()),
        );
        let errors = validate_metadata(&metadata, None);
        assert!(errors.is_empty());

        let mut metadata = base_metadata();
        metadata.insert("owner".to_string(), Value::String("me".to_string()));
        let errors = validate_metadata(&metadata, None);
        assert!(errors.iter().any(|err| err.contains("Unexpected fields")));
    }

    #[test]
    fn derive_description_skips_code_blocks() {
        let body = "\n# Title\n\n```bash\necho hi\n```\n\nUse this skill to do X.\n";
        assert_eq!(derive_description(body), "Use this skill to do X.");

        let fallback = "# Title\n\n```bash\necho hi\n```\n";
        assert_eq!(derive_description(fallback), "Title");
    }

    #[test]
    fn format_frontmatter_orders_and_quotes() {
        let mut metadata = BTreeMap::new();
        metadata.insert("description".to_string(), Value::String("Use: this # now".to_string()));
        metadata.insert("name".to_string(), Value::String("my-skill".to_string()));
        metadata.insert("license".to_string(), Value::String("MIT".to_string()));
        metadata.insert(
            "compatibility".to_string(),
            Value::String("Rust 1.75+".to_string()),
        );
        metadata.insert(
            "allowed-tools".to_string(),
            Value::String("Bash(git:*)".to_string()),
        );
        let mut meta_map = Mapping::new();
        meta_map.insert(Value::String("z".to_string()), Value::String("2".to_string()));
        meta_map.insert(Value::String("a".to_string()), Value::String("1".to_string()));
        metadata.insert("metadata".to_string(), Value::Mapping(meta_map));
        metadata.insert("owner".to_string(), Value::String("me".to_string()));

        let formatted = format_frontmatter(&metadata).expect("format frontmatter");
        let expected = [
            "---",
            "name: my-skill",
            "description: \"Use: this # now\"",
            "license: MIT",
            "compatibility: Rust 1.75+",
            "allowed-tools: \"Bash(git:*)\"",
            "metadata:",
            "  a: 1",
            "  z: 2",
            "owner: me",
            "---",
        ]
        .join("\n");
        assert_eq!(formatted, expected);
    }

    #[test]
    fn check_and_fix_skill_files() {
        let dir = temp_skill_dir("my-skill");
        let skill_dir = dir.path().join("my-skill");
        let skill_path = write_skill(
            &skill_dir,
            "skill.md",
            "# Title\n\nUse this skill to do X.\n",
        );

        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_path,
            content: fs::read_to_string(skill_dir.join("skill.md")).unwrap(),
        };

        let errors = check_skill(&skill);
        assert!(errors.iter().any(|err| err.contains("uppercase")));

        let (changed, errors) = fix_skill(&skill);
        assert!(changed);
        assert!(errors.is_empty());

        let fixed = fs::read_to_string(skill_dir.join("SKILL.md")).expect("read fixed");
        assert!(fixed.contains("name: my-skill"));
        assert!(fixed.contains("description: Use this skill to do X."));
        assert!(fixed.ends_with('\n'));

        let fixed_skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_dir.join("SKILL.md"),
            content: fixed,
        };
        let errors = check_skill(&fixed_skill);
        assert!(errors.is_empty());
    }

    #[test]
    fn fix_skill_preserves_unknown_fields_and_metadata() {
        let dir = temp_skill_dir("my-skill");
        let skill_dir = dir.path().join("my-skill");
        let content = r#"---
description: A test skill
metadata:
  version: 1.2
  count: 5
owner: team
---
# Title
"#;
        let skill_path = write_skill(&skill_dir, "SKILL.md", content);
        let skill = SkillFile {
            dir_path: skill_dir.clone(),
            file_path: skill_path,
            content: fs::read_to_string(skill_dir.join("SKILL.md")).unwrap(),
        };

        let (changed, errors) = fix_skill(&skill);
        assert!(changed);
        assert!(errors.is_empty());

        let fixed = fs::read_to_string(skill_dir.join("SKILL.md")).expect("read fixed");
        assert!(fixed.contains("name: my-skill"));
        assert!(fixed.contains("description: A test skill"));
        assert!(fixed.contains("metadata:"), "fixed content:\n{fixed}");
        assert!(fixed.contains("  count: 5"));
        assert!(fixed.contains("  version: 1.2"));
        assert!(fixed.contains("owner: team"));
    }

    #[test]
    fn collect_skill_files_discovers_nested() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        let skill_a = root.join("a").join("SKILL.md");
        let skill_b = root.join("b").join("skill.md");
        fs::create_dir_all(skill_a.parent().unwrap()).expect("mkdir");
        fs::create_dir_all(skill_b.parent().unwrap()).expect("mkdir");
        fs::write(
            &skill_a,
            "---\nname: a\ndescription: A\n---\nBody\n",
        )
        .expect("write");
        fs::write(
            &skill_b,
            "---\nname: b\ndescription: B\n---\nBody\n",
        )
        .expect("write");

        let paths = vec![root.to_path_buf()];
        let skills = collect_skill_files(&paths);
        assert_eq!(skills.len(), 2);
    }

    proptest! {
        #[test]
        fn prop_valid_names_are_accepted(name in "[a-z0-9]{1,8}(-[a-z0-9]{1,8}){0,6}") {
            let mut metadata = base_metadata();
            metadata.insert("name".to_string(), Value::String(name));
            let errors = validate_metadata(&metadata, None);
            prop_assert!(errors.iter().all(|err| !err.contains("Skill name")));
        }

        #[test]
        fn prop_invalid_names_with_uppercase_fail(name in "[A-Z]{1,10}") {
            let mut metadata = base_metadata();
            metadata.insert("name".to_string(), Value::String(name));
            let errors = validate_metadata(&metadata, None);
            prop_assert!(errors.iter().any(|err| err.contains("lowercase")));
        }
    }
}
