//! Reading FGD files from disk, with includes.

use halberd_fgd::{ClassKind, KeyType, load};
use std::path::Path;

fn data(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

#[test]
fn a_file_and_its_includes_are_read() {
    let fgd = load(&data("game.fgd"));
    assert!(fgd.problems.is_empty(), "{:?}", fgd.problems);
    assert_eq!(fgd.files, ["base.fgd", "game.fgd"]);
    assert_eq!(fgd.map_size, Some((-16384, 16384)));
    let placeable: Vec<&str> = fgd.placeable().map(|c| c.name.as_str()).collect();
    assert_eq!(
        placeable,
        [
            "light",
            "info_player_start",
            "func_brush",
            "func_detail",
            "logic_relay"
        ]
    );
}

#[test]
fn a_light_gets_its_bases_keys_inputs_and_helpers() {
    let fgd = load(&data("game.fgd"));
    let light = fgd.resolved("light").unwrap();
    let keys: Vec<&str> = light.keys.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(
        keys,
        [
            "targetname",
            "angles",
            "_light",
            "style",
            "spawnflags",
            "_distance"
        ]
    );
    assert_eq!(light.key("_light").unwrap().kind, KeyType::Color255);
    let inputs: Vec<&str> = light.inputs.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(inputs, ["Kill", "TurnOn", "TurnOff"]);
    assert_eq!(light.color(), Some([255, 255, 0]));
    assert!(light.helper("iconsprite").is_some());
}

#[test]
fn joined_descriptions_models_and_overridden_outputs() {
    let fgd = load(&data("game.fgd"));
    let start = fgd.resolved("info_player_start").unwrap();
    assert!(start.class.description.ends_with("may be placed in a map."));
    assert_eq!(start.model(), Some("models/editor/playerstart.mdl"));
    assert_eq!(
        start.size(),
        Some(([-16.0, -16.0, 0.0], [16.0, 16.0, 72.0]))
    );
    let brush = fgd.resolved("func_brush").unwrap();
    assert_eq!(brush.class.kind, ClassKind::Solid);
    let on_user = brush.outputs.iter().find(|o| o.name == "OnUser1").unwrap();
    assert_eq!(on_user.param, "string", "the class's own output wins");
}

#[test]
fn missing_and_escaping_includes_are_reported() {
    let folder = tempfile::tempdir().unwrap();
    let main = folder.path().join("main.fgd");
    std::fs::write(
        &main,
        "@include \"missing.fgd\"\n@include \"../outside.fgd\"\n@include \"main.fgd\"\n@PointClass = e []",
    )
    .unwrap();
    let fgd = load(&main);
    assert!(fgd.class("e").is_some());
    assert_eq!(fgd.problems.len(), 2, "{:?}", fgd.problems);
    assert!(fgd.problems[0].message.contains("could not be opened"));
    assert!(fgd.problems[1].message.contains("outside the FGD folder"));
}
