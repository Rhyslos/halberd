//! Renders real frames on a GPU and checks the pixels.
//!
//! Machines without a GPU skip these tests with a note, unless the
//! environment variable `HALBERD_REQUIRE_GPU` is set, in which case a missing
//! GPU is a failure. CI sets it in the window smoke test job, which has a
//! software Vulkan driver, so these tests always run somewhere.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

use glam::{Vec2, Vec3};
use halberd_render::{
    BACKGROUND, FrameParams, ViewportRenderer, ViewportTarget, best_sample_count, read_pixels,
};
use halberd_tools::Camera;

struct Gpu {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

fn gpu() -> Option<Gpu> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
    let found =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()
            .and_then(|adapter| {
                let (device, queue) =
                    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                        .ok()?;
                Some(Gpu {
                    adapter,
                    device,
                    queue,
                })
            });
    if found.is_none() {
        assert!(
            std::env::var_os("HALBERD_REQUIRE_GPU").is_none(),
            "HALBERD_REQUIRE_GPU is set but no GPU adapter was found"
        );
        eprintln!("No GPU adapter available; skipping GPU rendering test.");
    }
    found
}

/// A rendered image and the camera that produced it.
struct Shot {
    pixels: Vec<u8>,
    size: [u32; 2],
    camera: Camera,
}

impl Shot {
    fn pixel(&self, x: i64, y: i64) -> Option<[u8; 4]> {
        let [w, h] = self.size;
        if x < 0 || y < 0 || x >= i64::from(w) || y >= i64::from(h) {
            return None;
        }
        let i = ((y as usize) * (w as usize) + x as usize) * 4;
        Some([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }

    /// The best score of `score` within a few pixels of where `point` appears.
    fn best_near(&self, point: Vec3, score: impl Fn([u8; 4]) -> i32) -> i32 {
        let size = Vec2::new(self.size[0] as f32, self.size[1] as f32);
        let at = self
            .camera
            .project(point, size)
            .expect("point is in front of the camera");
        let (cx, cy) = (at.x as i64, at.y as i64);
        let mut best = i32::MIN;
        for dy in -3..=3 {
            for dx in -3..=3 {
                if let Some(p) = self.pixel(cx + dx, cy + dy) {
                    best = best.max(score(p));
                }
            }
        }
        best
    }
}

fn render(gpu: &Gpu, renderer: &ViewportRenderer, target: &ViewportTarget, camera: Camera) -> Shot {
    let size = target.size();
    let params = FrameParams {
        view_projection: camera.view_projection(Vec2::new(size[0] as f32, size[1] as f32)),
        camera_position: camera.position,
        grid_size: 16.0,
    };
    renderer.render(&gpu.device, &gpu.queue, target, &params);
    Shot {
        pixels: read_pixels(&gpu.device, &gpu.queue, target).unwrap(),
        size,
        camera,
    }
}

/// The background colour as stored in the sRGB image.
fn background_srgb() -> [u8; 3] {
    let encode = |linear: f64| {
        let s = if linear <= 0.003_130_8 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    };
    [
        encode(BACKGROUND.r),
        encode(BACKGROUND.g),
        encode(BACKGROUND.b),
    ]
}

fn red(p: [u8; 4]) -> i32 {
    i32::from(p[0]) - i32::from(p[1].max(p[2]))
}
fn green(p: [u8; 4]) -> i32 {
    i32::from(p[1]) - i32::from(p[0].max(p[2]))
}
fn blue(p: [u8; 4]) -> i32 {
    i32::from(p[2]) - i32::from(p[0].max(p[1]))
}

fn check_standard_view(gpu: &Gpu, sample_count: u32) {
    let renderer = ViewportRenderer::new(&gpu.device, sample_count);
    let target = renderer.create_target(&gpu.device, [480, 320]);
    let camera = Camera::looking_at(Vec3::new(-600.0, -600.0, 400.0), Vec3::ZERO);
    let shot = render(gpu, &renderer, &target, camera);
    let label = format!("{sample_count}x multisampling");

    // The top edge looks above the horizon: plain background.
    let bg = background_srgb();
    let corner = shot.pixel(2, 2).unwrap();
    for c in 0..3 {
        assert!(
            corner[c].abs_diff(bg[c]) <= 2,
            "{label}: sky is {corner:?}, expected {bg:?}"
        );
    }
    assert_eq!(corner[3], 255, "{label}: image is opaque");

    // Axes in their colours.
    assert!(
        shot.best_near(Vec3::new(300.0, 0.0, 0.0), red) > 60,
        "{label}: X axis not red"
    );
    assert!(
        shot.best_near(Vec3::new(0.0, 300.0, 0.0), green) > 40,
        "{label}: Y axis not green"
    );
    assert!(
        shot.best_near(Vec3::new(0.0, 0.0, 300.0), blue) > 40,
        "{label}: Z axis not blue"
    );

    // A major grid line away from the axes is visible against the background.
    let brightness = |p: [u8; 4]| i32::from(p[0]) + i32::from(p[1]) + i32::from(p[2]);
    let background = i32::from(bg[0]) + i32::from(bg[1]) + i32::from(bg[2]);
    let line = shot.best_near(Vec3::new(128.0, 200.0, 0.0), brightness);
    assert!(
        line > background + 15,
        "{label}: grid line too faint ({line} vs {background})"
    );
}

#[test]
fn draws_background_axes_and_grid() {
    let Some(gpu) = gpu() else { return };
    check_standard_view(&gpu, best_sample_count(&gpu.adapter));
}

#[test]
fn draws_correctly_without_multisampling() {
    let Some(gpu) = gpu() else { return };
    check_standard_view(&gpu, 1);
}

#[test]
fn looking_straight_down_shows_both_axes() {
    let Some(gpu) = gpu() else { return };
    let renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    let target = renderer.create_target(&gpu.device, [256, 256]);
    let camera = Camera::looking_at(Vec3::new(0.0, 0.0, 1000.0), Vec3::ZERO);
    let shot = render(&gpu, &renderer, &target, camera);
    assert!(shot.best_near(Vec3::new(150.0, 0.0, 0.0), red) > 60);
    assert!(shot.best_near(Vec3::new(0.0, 150.0, 0.0), green) > 40);
}

#[test]
fn targets_of_any_size_render_and_read_back() {
    let Some(gpu) = gpu() else { return };
    let renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    for (asked, expected) in [([0, 0], [1, 1]), ([1, 1], [1, 1]), ([333, 77], [333, 77])] {
        let target = renderer.create_target(&gpu.device, asked);
        assert_eq!(target.size(), expected);
        let shot = render(&gpu, &renderer, &target, Camera::default());
        assert_eq!(shot.pixels.len(), (expected[0] * expected[1] * 4) as usize);
    }
}

#[test]
fn oversized_targets_are_clamped_to_the_gpu_limit() {
    let Some(gpu) = gpu() else { return };
    let renderer = ViewportRenderer::new(&gpu.device, 1);
    let max = gpu.device.limits().max_texture_dimension_2d;
    let target = renderer.create_target(&gpu.device, [u32::MAX, 8]);
    assert_eq!(target.size(), [max, 8]);
}

#[test]
fn rendering_many_frames_is_stable() {
    let Some(gpu) = gpu() else { return };
    let renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    let target = renderer.create_target(&gpu.device, [200, 150]);
    let mut camera = Camera::default();
    for _ in 0..120 {
        camera.orbit(Vec3::ZERO, 0.05, 0.01);
        let params = FrameParams {
            view_projection: camera.view_projection(Vec2::new(200.0, 150.0)),
            camera_position: camera.position,
            grid_size: 16.0,
        };
        renderer.render(&gpu.device, &gpu.queue, &target, &params);
    }
    let shot = render(&gpu, &renderer, &target, camera);
    assert_eq!(shot.pixels.len(), 200 * 150 * 4);
}
