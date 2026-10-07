//! Runs the architecture checks on the real Halberd workspace.
//! If this test fails, its message says which crate broke which rule.

use std::path::Path;

fn workspace_root() -> &'static Path {
    // tools/layer-check -> workspace root
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn workspace_follows_architecture_rules() {
    let crates = layer_check::load_workspace(workspace_root()).unwrap();
    let violations = layer_check::check(&crates);
    let report: Vec<String> = violations.iter().map(ToString::to_string).collect();
    assert!(
        violations.is_empty(),
        "architecture rules broken:\n{}",
        report.join("\n")
    );
}

#[test]
fn every_planned_crate_exists() {
    let crates = layer_check::load_workspace(workspace_root()).unwrap();
    let names: Vec<&str> = crates.iter().map(|c| c.name.as_str()).collect();
    let expected = [
        "halberd-app",
        "halberd-assets",
        "halberd-compile",
        "halberd-config",
        "halberd-doc",
        "halberd-fgd",
        "halberd-geom",
        "halberd-gma",
        "halberd-io",
        "halberd-kv",
        "halberd-mdl",
        "halberd-render",
        "halberd-tools",
        "halberd-ui",
        "halberd-vmf",
        "halberd-vpk",
        "halberd-vtf",
    ];
    assert_eq!(names, expected);
}

#[test]
fn only_the_app_is_in_the_top_layer() {
    let crates = layer_check::load_workspace(workspace_root()).unwrap();
    let top: Vec<&str> = crates
        .iter()
        .filter(|c| c.layer == Some(layer_check::MAX_LAYER))
        .map(|c| c.name.as_str())
        .collect();
    assert_eq!(top, ["halberd-app"]);
}

#[test]
fn file_format_crates_depend_on_formats_only() {
    let crates = layer_check::load_workspace(workspace_root()).unwrap();
    for c in crates.iter().filter(|c| c.layer == Some(0)) {
        for dep in &c.halberd_deps {
            let dep_layer = crates.iter().find(|d| &d.name == dep).and_then(|d| d.layer);
            assert_eq!(
                dep_layer,
                Some(0),
                "{} must only use other format crates",
                c.name
            );
        }
    }
}
