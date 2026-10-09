//! Connects the GPU viewport renderer (`halberd-render`) to the window's
//! wgpu device and to egui, so the panel can show the rendered image.

use eframe::egui;
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use halberd_doc::Document;
use halberd_render::{FrameParams, PlayerOutline, ViewportRenderer as GpuRenderer, ViewportTarget};
use halberd_tools::PLAYER_EYE_HEIGHT;
use halberd_ui::{ViewportRenderer, ViewportView};

/// Draws the viewport with the same GPU device the window uses.
pub(crate) struct GpuViewport {
    state: RenderState,
    renderer: GpuRenderer,
    target: Option<ViewportTarget>,
    /// The size last asked for (the target may be clamped smaller).
    requested_size: [u32; 2],
    texture: Option<egui::TextureId>,
}

impl GpuViewport {
    /// Sets up viewport rendering on the window's GPU device.
    pub(crate) fn new(state: &RenderState) -> Self {
        let samples = halberd_render::best_sample_count(&state.adapter);
        Self {
            renderer: GpuRenderer::new(&state.device, samples),
            state: state.clone(),
            target: None,
            requested_size: [0, 0],
            texture: None,
        }
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

    /// Makes sure a target of the requested size exists and is registered
    /// with egui. Returns the egui texture showing it.
    fn prepare_target(&mut self, size: [u32; 2]) -> egui::TextureId {
        let device = &self.state.device;
        if self.target.is_none() || self.requested_size != size {
            let target = self.renderer.create_target(device, size);
            let mut egui_renderer = self.state.renderer.write();
            let id = match self.texture {
                Some(id) => {
                    egui_renderer.update_egui_texture_from_wgpu_texture(
                        device,
                        target.display_view(),
                        wgpu::FilterMode::Nearest,
                        id,
                    );
                    id
                }
                None => egui_renderer.register_native_texture(
                    device,
                    target.display_view(),
                    wgpu::FilterMode::Nearest,
                ),
            };
            self.texture = Some(id);
            self.target = Some(target);
            self.requested_size = size;
            return id;
        }
        // Both are always set together above.
        self.texture.unwrap_or_default()
    }
}

impl ViewportRenderer for GpuViewport {
    fn render(&mut self, view: &ViewportView, doc: &Document) -> Result<egui::TextureId, String> {
        let texture = self.prepare_target(view.size_px);
        self.renderer.update_scene(&self.state.device, doc);
        self.renderer.set_preview_shape(&view.preview_shape);
        let target = self
            .target
            .as_ref()
            .ok_or("the viewport image could not be created")?;
        let params = FrameParams {
            view_projection: view.view_projection,
            camera_position: view.camera_position,
            grid_size: view.grid_size,
            preview: view.preview,
            player: view.player.map(|bounds| PlayerOutline {
                bounds,
                eye_height: PLAYER_EYE_HEIGHT,
            }),
        };
        self.renderer
            .render(&self.state.device, &self.state.queue, target, &params);
        Ok(texture)
    }
}
