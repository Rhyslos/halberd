//! Tests for the material browser.

use super::*;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    browser: MaterialBrowser,
    can_apply: bool,
    actions: Vec<BrowserAction>,
}

fn harness(names: &[&str]) -> Harness<'static, State> {
    let mut browser = MaterialBrowser::default();
    browser.set_names(names.iter().map(|n| n.to_string()).collect());
    Harness::builder().with_size([300.0, 500.0]).build_ui_state(
        |ui, s: &mut State| {
            if let Some(action) = s.browser.show(ui, s.can_apply) {
                s.actions.push(action);
            }
        },
        State {
            browser,
            can_apply: true,
            actions: Vec::new(),
        },
    )
}

const NAMES: [&str; 4] = [
    "BRICK/BRICKWALL001A",
    "CONCRETE/CONCRETEFLOOR001A",
    "DEV/DEV_MEASUREGENERIC01B",
    "DEV/DEV_MEASUREWALL01A",
];

#[test]
fn searching_narrows_the_list_by_every_word() {
    let mut h = harness(&NAMES);
    h.run();
    assert!(h.query_by_label("4 of 4 materials").is_some());
    h.get_by_label(MATERIAL_SEARCH_NAME).click();
    h.run();
    h.get_by_label(MATERIAL_SEARCH_NAME).type_text("dev wall");
    h.run();
    assert!(h.query_by_label("1 of 4 materials").is_some());
    assert!(h.query_by_label("DEV/DEV_MEASUREWALL01A").is_some());
    assert!(h.query_by_label("BRICK/BRICKWALL001A").is_none());
}

#[test]
fn a_click_picks_and_a_double_click_or_the_button_applies() {
    let mut h = harness(&NAMES);
    h.run();
    h.get_by_label("BRICK/BRICKWALL001A").click();
    h.run();
    assert_eq!(h.state().browser.picked(), Some("BRICK/BRICKWALL001A"));
    assert_eq!(
        h.state().actions,
        [BrowserAction::Pick("BRICK/BRICKWALL001A".into())]
    );
    h.get_by_label(APPLY_MATERIAL_LABEL).click();
    h.run();
    assert_eq!(
        h.state().actions.last(),
        Some(&BrowserAction::Apply("BRICK/BRICKWALL001A".into()))
    );
    // Nothing selected: the button can't be used.
    h.state_mut().can_apply = false;
    h.run();
    let before = h.state().actions.len();
    h.get_by_label(APPLY_MATERIAL_LABEL).click();
    h.run();
    assert_eq!(h.state().actions.len(), before);
}

#[test]
fn pictures_are_asked_for_once_and_shown_when_they_arrive() {
    let mut h = harness(&NAMES);
    h.run();
    let wanted = h.state_mut().browser.take_wanted();
    assert_eq!(wanted.len(), 4, "every tile on screen wants its picture");
    h.run();
    assert!(
        h.state_mut().browser.take_wanted().is_empty(),
        "asked only once"
    );
    h.state_mut()
        .browser
        .add_picture("brick/brickwall001a", Ok((2, 2, vec![255; 16])));
    h.state_mut().browser.add_picture(
        "DEV/DEV_MEASUREWALL01A",
        Err("its texture is missing".into()),
    );
    // A picture whose pixels don't match its size is refused, not shown.
    h.state_mut()
        .browser
        .add_picture("DEV/DEV_MEASUREGENERIC01B", Ok((4, 4, vec![0; 3])));
    h.run();
    assert!(matches!(
        h.state().browser.pictures.get("BRICK/BRICKWALL001A"),
        Some(Picture::Shown(_))
    ));
    assert!(matches!(
        h.state().browser.pictures.get("DEV/DEV_MEASUREGENERIC01B"),
        Some(Picture::None(_))
    ));
}

#[test]
fn only_so_many_pictures_are_kept() {
    let mut browser = MaterialBrowser::default();
    for i in 0..MAX_PICTURES + 50 {
        browser.add_picture(&format!("M/{i}"), Err("none".into()));
    }
    assert_eq!(browser.pictures.len(), MAX_PICTURES);
    assert!(
        !browser.pictures.contains_key("M/0"),
        "the oldest went first"
    );
    assert_eq!(truncate("TOOLSNODRAW", 12), "TOOLSNODRAW");
    assert_eq!(truncate("CONCRETEFLOOR001A", 12), "CONCRETEFLO…");
}

#[test]
fn pictures_on_screen_are_kept_and_huge_ones_refused() {
    let mut h = harness(&NAMES);
    h.run();
    h.state_mut()
        .browser
        .add_picture("BRICK/BRICKWALL001A", Ok((2, 2, vec![255; 16])));
    // Many pictures arrive later, but the brick is still drawn each frame.
    for i in 0..MAX_PICTURES {
        h.state_mut()
            .browser
            .add_picture(&format!("M/{i}"), Err("none".into()));
        if i % 100 == 0 {
            h.run();
        }
    }
    assert!(
        h.state()
            .browser
            .pictures
            .contains_key("BRICK/BRICKWALL001A")
    );
    h.state_mut()
        .browser
        .add_picture("DEV/DEV_MEASUREWALL01A", Ok((4096, 1, vec![0; 4096 * 4])));
    assert!(matches!(
        h.state().browser.pictures.get("DEV/DEV_MEASUREWALL01A"),
        Some(Picture::None(_))
    ));
}
