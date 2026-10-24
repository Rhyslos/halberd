//! Properties with nothing selected: the map itself. Its size against the
//! ±16384-unit edge, and meters for the engine's limits.

use egui::{Color32, ProgressBar, RichText, Ui};
use halberd_config::LengthUnit;
use halberd_doc::{Document, MAP_HALF_SIZE, MapStats};

/// Accessible name of the button selecting objects past the map's edge.
pub(crate) const SELECT_OUTSIDE_LABEL: &str = "Select them";
/// Heading of the map summary.
pub(crate) const MAP_HEADING: &str = "Map";

/// The last stats worked out, kept while the map doesn't change (counting
/// a big map every frame would be wasted work).
#[derive(Debug, Default)]
pub(crate) struct StatsCache {
    shown: Option<((u64, u64), MapStats)>,
}

impl StatsCache {
    /// The stats for `doc` as it is now.
    pub(crate) fn get(&mut self, doc: &Document) -> &MapStats {
        let key = (doc.instance(), doc.revision());
        if self.shown.as_ref().is_some_and(|(k, _)| *k != key) {
            self.shown = None;
        }
        &self.shown.get_or_insert_with(|| (key, doc.stats())).1
    }
}

/// Colour of a meter: green, yellow past three quarters, red when full.
fn meter_color(fraction: f32) -> Color32 {
    if fraction >= 1.0 {
        Color32::from_rgb(210, 60, 50)
    } else if fraction >= 0.75 {
        Color32::from_rgb(220, 170, 40)
    } else {
        Color32::from_rgb(60, 150, 80)
    }
}

/// Draws the map summary.
pub(crate) fn show(ui: &mut Ui, doc: &mut Document, unit: LengthUnit, cache: &mut StatsCache) {
    let stats = cache.get(doc).clone();
    ui.add_space(8.0);
    ui.label(RichText::new(MAP_HEADING).strong().size(15.0));
    match stats.bounds {
        Some(b) => {
            let size = b.size();
            let n = |v: f32| {
                let text = unit.format(f64::from(v));
                text.trim_end_matches(unit.suffix()).to_string()
            };
            ui.label(format!(
                "Size: {} × {} × {}{}",
                n(size.x),
                n(size.y),
                n(size.z),
                unit.suffix()
            ))
            .on_hover_text("Width, depth and height of everything in the map");
            let largest = size.max_element() / (MAP_HALF_SIZE * 2.0);
            ui.add(
                ProgressBar::new(largest.clamp(0.0, 1.0))
                    .text(format!(
                        "{:.0}% of the largest map ({} across)",
                        largest * 100.0,
                        unit.format(f64::from(MAP_HALF_SIZE * 2.0))
                    ))
                    .fill(meter_color(largest)),
            );
        }
        None => {
            ui.label(RichText::new("Empty").weak());
        }
    }
    if !stats.outside.is_empty() {
        ui.horizontal_wrapped(|ui| {
            let count = stats.outside.len();
            ui.colored_label(
                meter_color(1.0),
                format!(
                    "{count} object{} reach{} past the map's edge ({} from the middle), \
                     which won't compile.",
                    if count == 1 { "" } else { "s" },
                    if count == 1 { "es" } else { "" },
                    unit.format(f64::from(MAP_HALF_SIZE))
                ),
            );
            if ui.button(SELECT_OUTSIDE_LABEL).clicked() {
                doc.set_selection(stats.outside.iter().copied());
            }
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Engine limits").strong());
    ui.label(
        RichText::new("Estimates; the compiler's counts can differ a little.")
            .weak()
            .small(),
    );
    egui::Grid::new("halberd_engine_limits")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            for meter in &stats.meters {
                ui.label(meter.name).on_hover_text(meter.help);
                let fraction = meter.fraction();
                let bar = ProgressBar::new(fraction.clamp(0.0, 1.0))
                    .desired_width(ui.available_width().min(220.0))
                    .text(format!("{} of {}", group(meter.count), group(meter.limit)))
                    .fill(meter_color(fraction));
                let response = ui.add(bar).on_hover_text(meter.help);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::ProgressIndicator,
                        true,
                        format!("{}: {} of {}", meter.name, meter.count, meter.limit),
                    )
                });
                ui.end_row();
            }
        });
}

/// 8192 as "8,192".
fn group(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_colours_read_at_a_glance() {
        assert_eq!(group(65_536), "65,536");
        assert_eq!(group(12), "12");
        assert_eq!(meter_color(0.2), Color32::from_rgb(60, 150, 80));
        assert_eq!(meter_color(0.8), Color32::from_rgb(220, 170, 40));
        assert_eq!(meter_color(1.2), Color32::from_rgb(210, 60, 50));
    }

    #[test]
    fn the_cache_counts_again_only_when_the_map_changes() {
        let mut doc = Document::new();
        let mut cache = StatsCache::default();
        assert_eq!(cache.get(&doc).meters[0].count, 0);
        let cube = halberd_geom::Brush::cuboid(halberd_geom::Aabb::from_corners(
            glam::Vec3::ZERO,
            glam::Vec3::splat(64.0),
        ))
        .unwrap();
        doc.execute(halberd_doc::Command::AddBrushes(vec![cube]))
            .unwrap();
        assert_eq!(cache.get(&doc).meters[0].count, 1);
    }
}
