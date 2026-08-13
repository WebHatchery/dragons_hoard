use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn read_json(relative: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json = fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("{relative} must be readable: {error}"));
    serde_json::from_str(&json).unwrap_or_else(|error| panic!("{relative} must be JSON: {error}"))
}

#[test]
fn asset_registry_matches_the_runtime_texture_manifest() {
    let registry = read_json("asset_registry.json");
    assert_eq!(registry["version"], 1);
    let registered: BTreeSet<&str> = registry["assets"]
        .as_array()
        .expect("asset registry needs an assets array")
        .iter()
        .map(|entry| entry.as_str().expect("asset paths must be strings"))
        .collect();

    let texture_manifest = read_json("assets/data/texture_manifest.json");
    let runtime_textures: BTreeSet<&str> = texture_manifest
        .as_array()
        .expect("texture manifest must be an array")
        .iter()
        .map(|entry| {
            entry["path"]
                .as_str()
                .expect("each runtime texture needs a path")
        })
        .collect();

    assert_eq!(registered, runtime_textures);
    assert!(
        registered.is_empty(),
        "Dragon's Hoard currently renders art procedurally"
    );
}
