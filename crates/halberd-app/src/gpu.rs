//! Connects the GPU viewport renderer (`halberd-render`) to the window's
//! wgpu device and to egui, so the panel can show the rendered image.

use eframe::egui;
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use halberd_doc::Document;
use halberd_render::{
    FrameParams, GridPlane, PlayerOutline, PropModel, RenderMode, ViewportRenderer as GpuRenderer,
    ViewportTarget,
};
use halberd_tools::PLAYER_EYE_HEIGHT;
use halberd_ui::{ViewportRenderer, ViewportView};

/// Draws the viewport's views with the same GPU device the window uses.
pub(crate) struct GpuViewport {
    state: RenderState,
    renderer: GpuRenderer,
    /// An image for each view, made when the view is first drawn.
    views: Vec<Option<ViewImage>>,
    /// Where prop browser pictures are drawn, made when first needed.
    thumbnail_target: Option<ViewportTarget>,
}

/// One view's image and the egui texture showing it.
struct ViewImage {
    target: ViewportTarget,
    /// The size last asked for (the target may be clamped smaller).
    requested_size: [u32; 2],
    texture: egui::TextureId,
}

impl GpuViewport {
    /// Sets up viewport rendering on the window's GPU device, keeping at
    /// most `textures` material textures (the memory budget).
    pub(crate) fn new(state: &RenderState, textures: u32) -> Self {
        let samples = halberd_render::best_sample_count(&state.adapter);
        let mut renderer = GpuRenderer::new(&state.device, samples);
        renderer.limit_textures(textures);
        Self {
            renderer,
            state: state.clone(),
            views: Vec::new(),
            thumbnail_target: None,
        }
    }

    /// Materials whose textures the Textured mode wants (each asked for
    /// once).
    pub(crate) fn wanted_textures(&mut self) -> Vec<String> {
        self.renderer.wanted_textures()
    }

    /// A material texture read for the Textured mode.
    pub(crate) fn texture_arrived(&mut self, picture: &halberd_assets::MaterialImage) {
        match &picture.image {
            Some(image) => self.renderer.upload_texture(
                &self.state.device,
                &self.state.queue,
                &picture.name,
                (image.width, image.height, &image.pixels),
                picture.texture_size,
            ),
            None => self.renderer.texture_missing(&picture.name),
        }
    }

    /// Models the map's props show that should be read (each asked for
    /// once).
    pub(crate) fn wanted_models(&mut self) -> Vec<String> {
        self.renderer.wanted_models()
    }

    /// A model read for the map's props (`None`: it can't be shown).
    pub(crate) fn model_arrived(&mut self, name: &str, model: Option<PropModel>) {
        self.renderer.add_model(name, model);
    }

    /// A picture of `model` for the prop browser: width, height and RGBA
    /// pixels. `None` if it has nothing to draw.
    pub(crate) fn thumbnail(&mut self, model: &PropModel) -> Option<(u32, u32, Vec<u8>)> {
        let size = halberd_ui::THUMBNAIL_SIZE;
        let target = self.thumbnail_target.get_or_insert_with(|| {
            self.renderer
                .create_target(&self.state.device, [size, size])
        });
        let [w, h] = target.size();
        let pixels =
            self.renderer
                .render_thumbnail(&self.state.device, &self.state.queue, target, model)?;
        Some((w, h, pixels))
    }

    /// A short description of the GPU, for the Console.
    pub(crate) fn describe(&self) -> String {
        let info = self.state.adapter.get_info();
        format!(
            "Graphics: {} ({:?}, {}x multisampling)",
            info.name,
            info.backend,
            self.renderer.sample_count()
        )
    }

    /// Makes sure view `id` has an image of the requested size, registered
    /// with egui. Returns where it is kept.
    fn prepare_target(&mut self, id: usize, size: [u32; 2]) -> usize {
        let id = id.min(halberd_ui::MAX_VIEWS - 1);
        if self.views.len() <= id {
            self.views.resize_with(id + 1, || None);
        }
        let device = &self.state.device;
        let slot = &mut self.views[id];
        let stale = slot.as_ref().is_none_or(|v| v.requested_size != size);
        if stale {
            let target = self.renderer.create_target(device, size);
            let mut egui_renderer = self.state.renderer.write();
            let texture = match slot.as_ref() {
                Some(old) => {
                    egui_renderer.update_egui_texture_from_wgpu_texture(
                        device,
                        target.display_view(),
                        wgpu::FilterMode::Nearest,
                        old.texture,
                    );
                    old.texture
                }
                None => egui_renderer.register_native_texture(
                    device,
                    target.display_view(),
                    wgpu::FilterMode::Nearest,
                ),
            };
            *slot = Some(ViewImage {
                target,
                requested_size: size,
                texture,
            });
        }
        id
    }
}

impl ViewportRenderer for GpuViewport {
    fn render(&mut self, view: &ViewportView, doc: &Document) -> Result<egui::TextureId, String> {
        self.renderer.update_scene(&self.state.device, doc);
        self.renderer.set_preview_shape(&view.preview_shape);
        let params = FrameParams {
            view_projection: view.view_projection,
            camera_position: view.camera_position,
            grid_size: view.grid_size,
            preview: view.preview,
            player: view.player.map(|bounds| PlayerOutline {
                bounds,
                eye_height: PLAYER_EYE_HEIGHT,
                color: match view.player_fit {
                    Some(halberd_tools::Fit::CrouchOnly) => halberd_render::PLAYER_CROUCH_COLOR,
                    Some(halberd_tools::Fit::Blocked) => halberd_render::PLAYER_BLOCKED_COLOR,
                    _ => halberd_render::PLAYER_COLOR,
                },
            }),
            grid: GridPlane {
                axis: view.grid_axis,
                offset: view.grid_offset,
                fade: view.grid_fade,
            },
            mode: match view.render_mode {
                halberd_ui::RenderMode::Wireframe => RenderMode::Wireframe,
                halberd_ui::RenderMode::Flat => RenderMode::Flat,
                halberd_ui::RenderMode::Textured => RenderMode::Textured,
            },
        };
        let slot = self.prepare_target(view.id, view.size_px);
        let image = self
            .views
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or("the viewport image could not be created")?;
        self.renderer.render(
            &self.state.device,
            &self.state.queue,
            &image.target,
            &params,
        );
        Ok(image.texture)
    }

    fn release_unused(&mut self, shown: usize) {
        if self.views.len() <= shown {
            return;
        }
        let mut egui_renderer = self.state.renderer.write();
        for image in self.views.drain(shown..).flatten() {
            egui_renderer.free_texture(&image.texture);
        }
    }
}
