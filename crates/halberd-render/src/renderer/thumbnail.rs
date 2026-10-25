//! Pictures of single models for the prop browser: the model alone,
//! framed from the front and a little above, in the Flat look.

use super::{BACKGROUND, GpuList, ViewportRenderer, ViewportTarget};
use crate::frame::{FrameParams, FrameUniforms, GridPlane, RenderMode};
use crate::models::PropModel;
use crate::readback::read_pixels;
use crate::scene::SceneGeometry;
use glam::{Mat4, Vec3, Vec4};
use halberd_doc::{Command, Document, EntityObject};

impl ViewportRenderer {
    /// Draws `model` alone into `target`, framed to fill it, and reads the
    /// picture back (RGBA, the target's size). The map being shown is not
    /// touched. `None` if the picture could not be read back.
    pub fn render_thumbnail(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &ViewportTarget,
        model: &PropModel,
    ) -> Option<Vec<u8>> {
        let (min, max) = model.bounds()?;
        // The model as a prop at the origin, through the scene builder, so
        // it looks as it does in the map.
        let mut doc = Document::new();
        doc.execute(Command::AddEntity(EntityObject::point(
            "prop_static",
            Vec3::ZERO,
            &[("model", "thumbnail")],
        )))
        .ok()?;
        doc.clear_selection();
        let geometry = SceneGeometry::from_document_with(&doc, &mut |_| None, &|_| Some(model));
        let faces = GpuList::upload(device, "halberd thumbnail", &geometry.faces, 3);

        let centre = (min + max) * 0.5;
        let radius = ((max - min).length() * 0.5).max(1.0);
        // From the model's front (+X), turned a little and from above.
        let direction = Vec3::new(1.0, -0.6, 0.55).normalize();
        let fov = 35f32.to_radians();
        let distance = radius / (fov * 0.5).sin();
        let eye = centre + direction * distance;
        let [w, h] = target.size();
        let aspect = w.max(1) as f32 / h.max(1) as f32;
        let view_projection =
            projection(fov, aspect, distance * 0.01, distance * 4.0) * look_at(eye, centre);
        let params = FrameParams {
            view_projection,
            camera_position: eye,
            grid_size: 16.0,
            preview: None,
            player: None,
            grid: GridPlane::default(),
            mode: RenderMode::Flat,
        };
        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&FrameUniforms::new(&params)),
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("halberd thumbnail"),
        });
        {
            let (view, resolve_target, store) = match &target.msaa_view {
                Some(msaa) => (msaa, Some(&target.resolved_view), wgpu::StoreOp::Discard),
                None => (&target.resolved_view, None, wgpu::StoreOp::Store),
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("halberd thumbnail pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(BACKGROUND),
                        store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &target.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.brush_pipeline);
            faces.draw(&mut pass);
        }
        queue.submit([encoder.finish()]);
        read_pixels(device, queue, target).ok()
    }
}

/// World-to-camera transform for a camera at `eye` looking at `target`,
/// Z up (as the editor's camera builds it).
fn look_at(eye: Vec3, target: Vec3) -> Mat4 {
    let f = (target - eye).normalize_or(Vec3::X);
    let r = f.cross(Vec3::Z).normalize_or(Vec3::Y);
    let u = r.cross(f);
    Mat4::from_cols(
        Vec4::new(r.x, u.x, -f.x, 0.0),
        Vec4::new(r.y, u.y, -f.y, 0.0),
        Vec4::new(r.z, u.z, -f.z, 0.0),
        Vec4::new(-r.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
    )
}

/// Perspective with depth 0 (near) to 1 (far), as wgpu expects.
fn projection(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let y_scale = 1.0 / (fov_y * 0.5).tan();
    let depth = far / (near - far);
    Mat4::from_cols(
        Vec4::new(y_scale / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, y_scale, 0.0, 0.0),
        Vec4::new(0.0, 0.0, depth, -1.0),
        Vec4::new(0.0, 0.0, depth * near, 0.0),
    )
}
