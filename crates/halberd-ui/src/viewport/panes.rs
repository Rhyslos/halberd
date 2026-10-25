//! Drawing each view of the viewport panel, its view menu, and the layout
//! picker; saving and restoring the views.

use super::layout::{MAX_VIEWS, PaneView, RenderMode, ViewLayout, ViewSetup};
use super::{
    PIVOT_COLOR, PLAYER_LABEL_COLOR, ViewportPanel, ViewportRenderer, ViewportView, elements,
    gizmo, plan,
};
use egui::{Align, Align2, Color32, FontId, Layout, Rect, Stroke, Ui, UiBuilder, pos2, vec2};
use glam::Vec2;
use halberd_doc::Document;
use halberd_tools::Tool;

/// Accessible name of the layout picker in the viewport's top-right corner.
pub const LAYOUT_PICKER_LABEL: &str = "Views";

impl ViewportPanel {
    /// The current arrangement of views.
    pub fn layout(&self) -> ViewLayout {
        self.layout
    }

    /// Changes the arrangement of views. Views that disappear keep their
    /// cameras for when they come back.
    pub fn set_layout(&mut self, layout: ViewLayout) {
        self.layout = layout;
        if self.active >= layout.count() {
            self.active = 0;
        }
        if self.camera_view >= layout.count() {
            self.camera_view = 0;
        }
    }

    /// What view `index` shows.
    pub fn view(&self, index: usize) -> PaneView {
        self.panes
            .get(index)
            .map_or(PaneView::Perspective, |p| p.view)
    }

    /// How view `index` draws brushes.
    pub fn render_mode(&self, index: usize) -> RenderMode {
        self.panes
            .get(index)
            .map_or_else(RenderMode::default, |p| p.mode)
    }

    /// Makes view `index` draw brushes as `mode`.
    pub fn set_render_mode(&mut self, index: usize, mode: RenderMode) {
        if let Some(pane) = self.panes.get_mut(index) {
            pane.mode = mode;
        }
    }

    /// Makes view `index` show `view`.
    pub fn set_view(&mut self, index: usize, view: PaneView) {
        if let Some(pane) = self.panes.get_mut(index) {
            pane.set_view(view);
        }
    }

    /// The views as they are, to save between sessions.
    pub fn view_setup(&self) -> ViewSetup {
        ViewSetup {
            layout: self.layout,
            views: self.panes.iter().map(|p| p.view).collect(),
            sides: self.panes.iter().map(|p| p.side).collect(),
            modes: self.panes.iter().map(|p| p.mode).collect(),
        }
    }

    /// The same panel with saved views put back (whatever of them is usable).
    #[must_use]
    pub fn with_view_setup(mut self, setup: ViewSetup) -> Self {
        self.set_layout(setup.layout);
        for (pane, view) in self.panes.iter_mut().zip(setup.views) {
            pane.set_view(view);
            // Settings saved before render modes existed: start as the
            // restored view would.
            pane.mode = RenderMode::default_for(view);
        }
        for (pane, mode) in self.panes.iter_mut().zip(setup.modes) {
            pane.mode = mode;
        }
        for (pane, side) in self.panes.iter_mut().zip(setup.sides) {
            let keep = pane.side.side;
            pane.side = side.sanitized();
            // What the view shows decides which way its side camera looks.
            pane.side.side = match pane.view {
                PaneView::Side(s) => s,
                PaneView::Perspective => keep,
            };
        }
        self
    }

    /// Draws view `index` into `rect`: its image, what the tools show over
    /// it, its name (a menu to change what it shows) and, for the 3D view,
    /// the camera mode switcher.
    pub(super) fn draw_pane(
        &mut self,
        ui: &mut Ui,
        renderer: &mut dyn ViewportRenderer,
        doc: &Document,
        index: usize,
        rect: Rect,
    ) {
        let pane = &self.panes[index];
        let camera = pane.camera();
        let size = Vec2::new(rect.width().max(1.0), rect.height().max(1.0));
        let ppp = ui.ctx().pixels_per_point();
        let (grid_axis, grid_offset) = match pane.view {
            PaneView::Perspective => (2, 0.0),
            PaneView::Side(_) => pane.side.grid_plane(),
        };
        let view = ViewportView {
            size_px: [
                (rect.width() * ppp).round().max(1.0) as u32,
                (rect.height() * ppp).round().max(1.0) as u32,
            ],
            view_projection: camera.view_projection(size),
            camera_position: camera.position,
            grid_size: self.grid_size,
            preview: self.tools.preview(),
            preview_shape: self.tools.preview_brushes(),
            player: self.player_at,
            player_fit: self.player_clearance().map(|c| c.fit),
            id: index,
            grid_axis,
            grid_offset,
            grid_fade: pane.view == PaneView::Perspective,
            render_mode: pane.mode,
        };

        let painter = ui.painter_at(rect);
        match renderer.render(&view, doc) {
            Ok(texture) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(texture, rect, uv, Color32::WHITE);
            }
            Err(reason) => {
                painter.rect_filled(rect, 0.0, Color32::from_rgb(18, 20, 24));
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    format!("3D view unavailable: {reason}"),
                    FontId::proportional(14.0),
                    Color32::from_gray(150),
                );
            }
        }

        if pane.view == PaneView::Perspective
            && let Some(pivot) = pane.perspective.active_pivot()
            && let Some(at) = camera.project(pivot, size)
        {
            let centre = rect.min + vec2(at.x, at.y);
            painter.circle_stroke(centre, 6.0, Stroke::new(1.5, PIVOT_COLOR));
            painter.circle_filled(centre, 2.0, PIVOT_COLOR);
        }

        if let Some(player) = view.player
            && let Some(at) = camera.project(player.center().with_z(player.max.z + 8.0), size)
            && rect.contains(rect.min + vec2(at.x, at.y))
        {
            painter.text(
                rect.min + vec2(at.x, at.y),
                Align2::CENTER_BOTTOM,
                super::player::label(self.length_unit, self.player_clearance()),
                FontId::proportional(11.0),
                match view.player_fit {
                    Some(halberd_tools::Fit::CrouchOnly) => super::PLAYER_CROUCH_LABEL_COLOR,
                    Some(halberd_tools::Fit::Blocked) => super::PLAYER_BLOCKED_LABEL_COLOR,
                    _ => PLAYER_LABEL_COLOR,
                },
            );
        }
        super::readout::draw(
            &painter,
            rect,
            &camera,
            size,
            self.tools
                .preview()
                .filter(|_| self.tools.tool() == Tool::Box),
            (index == 0).then(|| doc.selection_bounds()).flatten(),
            self.length_unit,
        );

        self.draw_links(&painter, rect, &camera, size);
        self.draw_sculpt_brush(&painter, rect, &camera, size);
        self.draw_leak(&painter, rect, &camera, size);

        let first = index == 0;
        if let Some(overlay) = self.tools.element_overlay(&camera, size, doc) {
            elements::draw(&painter, rect, &overlay);
            if index == self.active
                && let Some((a, b)) = self.tools.marquee()
            {
                elements::draw_box(&painter, rect, a, b);
            }
            if first && doc.selection().is_empty() {
                elements::hint(&painter, rect, self.tools.select_mode());
            }
        }
        let gizmo_shapes = self.tools.gizmo_shapes(&camera, size, doc);
        gizmo::draw(&painter, rect, &gizmo_shapes);
        if self.tools.tool() == Tool::Box && self.tools.shape() == halberd_geom::Shape::FloorPlan {
            let preview = self.tools.plan_preview();
            if let Some(plan) = &preview {
                plan::draw(&painter, rect, &camera, plan);
            }
            if first {
                plan::hint(&painter, rect, preview.as_ref());
            }
        }

        self.view_menu(ui, rect, index);
        if self.panes[index].view == PaneView::Perspective {
            self.mode_switcher(ui, rect, index);
        }
    }

    /// The view's name in its top-left corner: a menu of what it can show.
    fn view_menu(&mut self, ui: &mut Ui, rect: Rect, index: usize) {
        let area = Rect::from_min_size(rect.left_top() + vec2(4.0, 2.0), vec2(160.0, 22.0));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::left_to_right(Align::Center)),
        );
        let current = self.panes[index].view;
        let button = egui::Button::new(
            egui::RichText::new(format!("{} ⏷", current.label()))
                .size(12.0)
                .color(Color32::from_gray(170)),
        )
        .frame(false);
        egui::containers::menu::MenuButton::from_button(button)
            .ui(&mut child, |ui| {
                for view in PaneView::ALL {
                    if ui.selectable_label(view == current, view.label()).clicked() {
                        self.panes[index].set_view(view);
                        ui.close();
                    }
                }
                ui.separator();
                ui.label("Draw as");
                let mode = self.panes[index].mode;
                for choice in RenderMode::ALL {
                    if ui
                        .radio(mode == choice, choice.label())
                        .on_hover_text(choice.description())
                        .clicked()
                    {
                        self.panes[index].mode = choice;
                        ui.close();
                    }
                }
            })
            .0
            .on_hover_text("What this view shows");
    }

    /// The layout picker in the viewport's top-right corner.
    pub(super) fn layout_picker(&mut self, ui: &mut Ui, full: Rect) {
        let area = Rect::from_min_size(full.right_top() + vec2(-150.0, 2.0), vec2(146.0, 22.0));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::right_to_left(Align::Center)),
        );
        let current = self.layout;
        let button = egui::Button::new(
            egui::RichText::new(format!("{LAYOUT_PICKER_LABEL} ⏷"))
                .size(12.0)
                .color(Color32::from_gray(170)),
        )
        .frame(false);
        egui::containers::menu::MenuButton::from_button(button)
            .ui(&mut child, |ui| {
                for layout in ViewLayout::ALL {
                    if ui
                        .selectable_label(layout == current, layout.label())
                        .clicked()
                    {
                        self.set_layout(layout);
                        ui.close();
                    }
                }
            })
            .0
            .on_hover_text(format!(
                "How many views to show (up to {MAX_VIEWS}), and how they are arranged"
            ));
    }
}
