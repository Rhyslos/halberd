//! Tests for reading FGD text.

use super::*;

#[test]
fn crate_name_matches_package() {
    assert_eq!(CRATE_NAME, env!("CARGO_PKG_NAME"));
}

const SAMPLE: &str = r#"
@BaseClass = Targetname [ targetname(target_source) : "Name" : : "Its name." ]
@PointClass base(Targetname) size(-4 -4 -4, 4 4 4) color(200 0 0) = env_spark : "Sparks"
[
    MaxDelay(string) : "Max delay" : "0"
    Magnitude(choices) : "Magnitude" : 1 =
    [
        1 : "Small"
        2 : "Medium"
    ]
    spawnflags(flags) = [ 64 : "Start on" : 1  128 : "Glow" : 0 ]
    input SparkOnce(void) : "Spark once."
    output OnSpark(void) : "Sparked."
]
"#;

#[test]
fn a_class_is_read_with_its_keys_choices_flags_and_io() {
    let fgd = parse(SAMPLE, "sample.fgd");
    assert!(fgd.problems.is_empty(), "{:?}", fgd.problems);
    let spark = fgd.class("ENV_SPARK").expect("found in any case");
    assert_eq!(spark.kind, ClassKind::Point);
    assert_eq!(spark.description, "Sparks");
    assert_eq!(spark.bases, ["Targetname"]);
    assert_eq!(spark.size(), Some(([-4.0; 3], [4.0; 3])));
    assert_eq!(spark.color(), Some([200, 0, 0]));
    let magnitude = &spark.keys[1];
    assert_eq!(magnitude.kind, KeyType::Choices);
    assert_eq!(magnitude.default.as_deref(), Some("1"));
    assert_eq!(magnitude.choices.len(), 2);
    assert_eq!(magnitude.choices[1].label, "Medium");
    let flags = &spark.keys[2];
    assert_eq!(flags.kind, KeyType::Flags);
    assert_eq!(flags.label, "spawnflags");
    assert!(flags.choices[0].on_by_default && !flags.choices[1].on_by_default);
    assert_eq!(spark.inputs[0].name, "SparkOnce");
    assert_eq!(spark.outputs[0].param, "void");
}

#[test]
fn bases_are_included_once_and_own_keys_win() {
    let fgd = parse(SAMPLE, "sample.fgd");
    let resolved = fgd.resolved("env_spark").unwrap();
    let names: Vec<&str> = resolved.keys.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["targetname", "MaxDelay", "Magnitude", "spawnflags"]);
    assert_eq!(resolved.key("TARGETNAME").unwrap().description, "Its name.");
    assert_eq!(fgd.placeable().count(), 1, "base classes aren't placed");
}

#[test]
fn bases_that_loop_or_are_missing_are_left_out() {
    let fgd = parse(
        r#"@BaseClass base(B) = A [ a(string) ]
           @BaseClass base(A, Missing) = B [ b(string) ]
           @PointClass base(B, C) = c_entity [ c(string) ]
           @BaseClass base(C) = C [ cc(string) ]"#,
        "loop.fgd",
    );
    let resolved = fgd.resolved("c_entity").unwrap();
    let names: Vec<&str> = resolved.keys.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "cc", "c"]);
}

#[test]
fn a_broken_class_is_reported_and_the_rest_still_read() {
    let fgd = parse(
        r#"@PointClass = broken : "No body"
           @PointClass = fine : "Fine" [ health(integer) : "Health" : 10 ]
           stray words
           @SolidClass = never_ends [ x(string) "#,
        "broken.fgd",
    );
    assert!(fgd.class("fine").is_some());
    assert!(fgd.class("broken").is_none());
    assert_eq!(fgd.problems.len(), 3, "{:?}", fgd.problems);
    assert!(
        fgd.problems
            .iter()
            .all(|p| p.file == "broken.fgd" && p.line > 0)
    );
    assert!(
        fgd.problems[0]
            .to_string()
            .starts_with("broken.fgd line 2:")
    );
}

#[test]
fn keys_may_leave_out_any_part_after_the_type() {
    let fgd = parse(
        r#"@PointClass = e [
            a(string)
            b(integer) : "B"
            c(float) : "C" : 0.5
            d(string) readonly : "D" : : "Only a description."
            e(sound) report : "E" : "ambient.wav" : "Text" + " joined."
        ]"#,
        "parts.fgd",
    );
    assert!(fgd.problems.is_empty(), "{:?}", fgd.problems);
    let keys = &fgd.class("e").unwrap().keys;
    assert_eq!(keys[0].label, "a");
    assert_eq!(keys[1].label, "B");
    assert_eq!(keys[2].default.as_deref(), Some("0.5"));
    assert!(keys[3].read_only);
    assert_eq!(keys[3].default, None);
    assert_eq!(keys[3].description, "Only a description.");
    assert_eq!(keys[4].kind, KeyType::Sound);
    assert_eq!(keys[4].description, "Text joined.");
}

#[test]
fn a_later_definition_replaces_an_earlier_one() {
    let fgd = parse(
        r#"@PointClass = e : "First" []
           @PointClass = E : "Second" []"#,
        "twice.fgd",
    );
    assert_eq!(fgd.len(), 1);
    assert_eq!(fgd.class("e").unwrap().description, "Second");
}

#[test]
fn a_size_with_one_value_is_centred() {
    let fgd = parse("@PointClass size(16 16 32) = e []", "size.fgd");
    assert_eq!(
        fgd.class("e").unwrap().size(),
        Some(([-8.0, -8.0, -16.0], [8.0, 8.0, 16.0]))
    );
}

#[test]
fn fuzz_odd_fgd_text_never_crashes() {
    let pieces = [
        "@PointClass",
        "@BaseClass",
        "@include",
        "=",
        ":",
        "[",
        "]",
        "(",
        ")",
        ",",
        "+",
        "\"text\"",
        "name",
        "base(",
        "size(",
        "-16",
        "choices",
        "flags",
        "input",
        "output",
        "\n",
        "//",
        "@",
    ];
    let mut seed: u64 = 0xF6D;
    for _ in 0..3000 {
        let text: String = (0..40)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                pieces[(seed % pieces.len() as u64) as usize]
            })
            .collect::<Vec<_>>()
            .join(" ");
        let fgd = parse(&text, "fuzz.fgd");
        for class in fgd.placeable() {
            let _ = fgd.resolved(&class.name);
        }
    }
}

#[test]
fn a_base_class_and_a_point_class_may_share_a_name() {
    // Regression: Valve's files have a base class "Light" and a point
    // class "light"; one replaced the other.
    let fgd = parse(
        r#"@BaseClass = Light [ _light(color255) : "Brightness" ]
           @PointClass base(Light) = light [ _distance(integer) ]"#,
        "light.fgd",
    );
    let light = fgd.resolved("light").unwrap();
    let names: Vec<&str> = light.keys.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["_light", "_distance"]);
    assert_eq!(fgd.len(), 2);
}
