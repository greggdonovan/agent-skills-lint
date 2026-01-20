#![no_main]

use std::collections::BTreeMap;

use agent_skills_lint::validate_metadata;
use libfuzzer_sys::fuzz_target;
use serde_yaml::Value;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_yaml::from_slice::<Value>(data) else {
        return;
    };
    let Value::Mapping(map) = value else {
        return;
    };

    let mut metadata = BTreeMap::new();
    for (key, value) in map {
        if let Value::String(key) = key {
            metadata.insert(key, value);
        }
    }

    let _ = validate_metadata(&metadata, None);
});
