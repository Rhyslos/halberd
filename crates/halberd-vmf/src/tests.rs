//! Tests for the VMF layer.

use super::*;

const SAMPLE: &str = include_str!("../tests/data/sample.vmf");

#[test]
fn the_sample_map_reads_and_writes_back_byte_for_byte() {
    let vmf = Vmf::parse(SAMPLE).unwrap();
    assert_eq!(vmf.line_ending, LineEnding::Windows);
    assert_eq!(vmf.to_text(), SAMPLE);
}

#[test]
fn world_entities_solids_and_sides_are_found() {
    let vmf = Vmf::parse(SAMPLE).unwrap();
    let world = vmf.world().unwrap();
    assert_eq!(world.get("classname"), Some("worldspawn"));
    assert_eq!(world.get("skyname"), Some("sky_day01_01"));
    let solids: Vec<&Block> = world.blocks("solid").collect();
    assert_eq!(solids.len(), 2);
    assert_eq!(id_of(solids[0]), Some(2));
    assert_eq!(solids[0].blocks("side").count(), 6);
    let classes: Vec<&str> = vmf.entities().filter_map(|e| e.get("classname")).collect();
    assert_eq!(
        classes,
        ["info_player_start", "light", "func_detail", "logic_relay"]
    );
    let detail = vmf.entities().nth(2).unwrap();
    assert_eq!(detail.blocks("solid").count(), 1);
    assert_eq!(vmf.top_blocks("cameras").count(), 1);
}

#[test]
fn planes_and_origins_read_as_numbers() {
    let vmf = Vmf::parse(SAMPLE).unwrap();
    let side = vmf
        .world()
        .unwrap()
        .blocks("solid")
        .next()
        .unwrap()
        .blocks("side")
        .next()
        .unwrap();
    let points = parse_plane(side.get("plane").unwrap()).unwrap();
    assert_eq!(points[0], [-256.0, 256.0, 0.0]);
    let player = vmf.entities().next().unwrap();
    assert_eq!(
        parse_vec3(player.get("origin").unwrap()),
        Some([-128.0, 128.0, 0.0])
    );
}

#[test]
fn bad_numbers_are_reported_not_a_crash() {
    for bad in [
        "",
        "(1 2 3) (4 5 6)",
        "(1 2 3) (4 5 6) (7 8 x)",
        "(nan 0 0) (1 1 1) (2 2 2)",
        "(1 2 3 4) (5 6) (7 8 9)",
    ] {
        let err = parse_plane(bad).unwrap_err();
        assert!(err.to_string().contains("plane"), "{bad}: {err}");
    }
    assert_eq!(parse_vec3("1 2"), None);
    assert_eq!(parse_vec3("1 2 3 4"), None);
    assert_eq!(parse_vec3("1 inf 3"), None);
    assert_eq!(parse_vec3(" 1.5 -2 3 "), Some([1.5, -2.0, 3.0]));
}

#[test]
fn numbers_are_written_as_hammer_writes_them() {
    assert_eq!(format_number(64.0), "64");
    assert_eq!(format_number(-0.0), "0");
    assert_eq!(format_number(-1e-9), "0");
    assert_eq!(format_number(12.5), "12.5");
    assert_eq!(format_number(1.0 / 3.0), "0.333333");
    assert_eq!(
        format_plane([[0.0, 64.0, 64.0], [64.0, 64.0, 64.0], [64.0, 0.5, 64.0]]),
        "(0 64 64) (64 64 64) (64 0.5 64)"
    );
    // Formatting then reading gives back the same plane.
    let points = [[-37.25, 12.0, 0.0], [1e5, -3.125, 7.0], [0.0, 0.0, -16.0]];
    assert_eq!(parse_plane(&format_plane(points)).unwrap(), points);
}

#[test]
fn files_that_are_not_maps_are_refused_with_a_reason() {
    assert_eq!(Vmf::parse("versioninfo { a b }"), Err(VmfError::NoWorld));
    let damaged = Vmf::parse("world {").unwrap_err();
    assert!(damaged.to_string().contains("damaged"), "{damaged}");
    assert!(VmfError::NoWorld.to_string().contains("not a Hammer map"));
}

#[test]
fn world_can_be_changed_in_place() {
    let mut vmf = Vmf::parse(SAMPLE).unwrap();
    vmf.world_mut().unwrap().push_pair("comment", "edited");
    assert_eq!(vmf.world().unwrap().get("comment"), Some("edited"));
}
