#![no_main]

use agent_skills_lint::parse_frontmatter;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
        let _ = parse_frontmatter(input);
        let wrapped = format!("---\n{}\n---\n", input);
        let _ = parse_frontmatter(&wrapped);
    }
});
