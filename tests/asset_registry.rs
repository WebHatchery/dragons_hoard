use macroquad_toolkit::data_loader::load_json_file_sync;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn asset_registry_matches_the_runtime_texture_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry: Value = load_json_file_sync(root.join("asset_registry.json"))
        .expect("asset_registry.json must be readable JSON");
    assert_eq!(registry["version"], 1);
    let registered: BTreeSet<&str> = registry["assets"]
        .as_array()
        .expect("asset registry needs an assets array")
        .iter()
        .map(|entry| entry.as_str().expect("asset paths must be strings"))
        .collect();

    let texture_manifest: Value =
        load_json_file_sync(root.join("assets/data/texture_manifest.json"))
            .expect("texture_manifest.json must be readable JSON");
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
