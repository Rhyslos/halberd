//! Renders real frames on a GPU and checks the pixels.
//!
//! Machines without a GPU skip these tests with a note, unless the
//! environment variable `HALBERD_REQUIRE_GPU` is set, in which case a missing
//! GPU is a failure. CI sets it in the window smoke test job, which has a
//! software Vulkan driver, so these tests always run somewhere.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

use glam::{Vec2, Vec3};
use halberd_doc::{Command, Document};
use halberd_geom::{Aabb, Brush};
use halberd_render::{
    BACKGROUND, FrameParams, GridPlane, PlayerOutline, RenderMode, ViewportRenderer,
    ViewportTarget, best_sample_count, read_pixels,
};
use halberd_tools::{Camera, Side, SideCamera};

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
    render_with_preview(gpu, renderer, target, camera, None)
}

fn render_with_preview(
    gpu: &Gpu,
    renderer: &ViewportRenderer,
    target: &ViewportTarget,
    camera: Camera,
    preview: Option<Aabb>,
) -> Shot {
    render_full(gpu, renderer, target, camera, preview, None)
}

fn render_full(
    gpu: &Gpu,
    renderer: &ViewportRenderer,
    target: &ViewportTarget,
    camera: Camera,
    preview: Option<Aabb>,
    player: Option<PlayerOutline>,
) -> Shot {
    render_mode(
        gpu,
        renderer,
        target,
        camera,
        preview,
        player,
        RenderMode::Flat,
    )
}

fn render_mode(
    gpu: &Gpu,
    renderer: &ViewportRenderer,
    target: &ViewportTarget,
    camera: Camera,
    preview: Option<Aabb>,
    player: Option<PlayerOutline>,
    mode: RenderMode,
) -> Shot {
    let size = target.size();
    let params = FrameParams {
        view_projection: camera.view_projection(Vec2::new(size[0] as f32, size[1] as f32)),
        camera_position: camera.position,
        grid_size: 16.0,
        preview,
        player,
        grid: grid_for(&camera),
        mode,
    };
    renderer.render(&gpu.device, &gpu.queue, target, &params);
    Shot {
        pixels: read_pixels(&gpu.device, &gpu.queue, target).unwrap(),
        size,
        camera,
    }
}

/// The ground for a perspective camera; for a side view's camera, the
/// plane behind what it shows, as the editor draws it.
fn grid_for(camera: &Camera) -> GridPlane {
    if camera.ortho.is_none() {
        return GridPlane::default();
    }
    let side = Side::ALL
        .into_iter()
        .find(|s| SideCamera::new(*s).camera().forward().dot(camera.forward()) > 0.999)
        .unwrap();
    let mut view = SideCamera::new(side);
    view.center = camera.position + camera.forward() * 100_000.0;
    let (axis, offset) = view.grid_plane();
    GridPlane {
        axis,
        offset,
        fade: false,
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
fn the_memory_budget_limits_material_textures_until_the_first_arrives() {
    let Some(gpu) = gpu() else { return };
    let mut renderer = ViewportRenderer::new(&gpu.device, 1);
    let most = gpu
        .device
        .limits()
        .max_texture_array_layers
        .min(halberd_render::MAX_MATERIAL_TEXTURES);
    assert_eq!(
        renderer.limit_textures(u32::MAX),
        most,
        "never more than before"
    );
    assert_eq!(renderer.limit_textures(64), 64.min(most));
    assert_eq!(renderer.limit_textures(0), 1, "at least one");
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
            preview: None,
            player: None,
            grid: GridPlane::default(),
            mode: RenderMode::Flat,
        };
        renderer.render(&gpu.device, &gpu.queue, &target, &params);
    }
    let shot = render(&gpu, &renderer, &target, camera);
    assert_eq!(shot.pixels.len(), 200 * 150 * 4);
}

/// A map with one box standing on the grid around the Z axis, 128 wide and
/// 128 tall, seen from above and to the side.
fn box_scene(gpu: &Gpu, selected: bool) -> (ViewportRenderer, ViewportTarget, Camera) {
    let mut doc = Document::new();
    let brush = Brush::cuboid(Aabb::from_corners(
        Vec3::new(-64.0, -64.0, 0.0),
        Vec3::new(64.0, 64.0, 128.0),
    ))
    .unwrap();
    doc.execute(Command::AddBrushes(vec![brush])).unwrap();
    if !selected {
        doc.clear_selection();
    }
    let mut renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    renderer.update_scene(&gpu.device, &doc);
    let target = renderer.create_target(&gpu.device, [400, 300]);
    let camera = Camera::looking_at(Vec3::new(-420.0, -300.0, 360.0), Vec3::new(0.0, 0.0, 64.0));
    (renderer, target, camera)
}

fn brightness(p: [u8; 4]) -> i32 {
    i32::from(p[0]) + i32::from(p[1]) + i32::from(p[2])
}

#[test]
fn brushes_are_drawn_solid_and_hide_what_is_behind_them() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, camera) = box_scene(&gpu, false);
    let shot = render(&gpu, &renderer, &target, camera);
    let background = background_srgb();
    let background = i32::from(background[0]) + i32::from(background[1]) + i32::from(background[2]);
    let top = Vec3::new(0.0, 0.0, 128.0);
    assert!(
        shot.best_near(top, brightness) > background + 120,
        "the top face should be lit"
    );
    assert!(shot.best_near(top, red) < 25, "an unselected box is grey");
    // The blue Z axis runs up through the middle of the box: hidden inside.
    assert!(
        shot.best_near(Vec3::new(0.0, 0.0, 64.0), blue) < 20,
        "the box hides the axis inside it"
    );
    // The axis is visible again above the box.
    assert!(shot.best_near(Vec3::new(0.0, 0.0, 220.0), blue) > 60);
}

#[test]
fn a_selected_brush_is_red() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, camera) = box_scene(&gpu, true);
    let shot = render(&gpu, &renderer, &target, camera);
    assert!(shot.best_near(Vec3::new(0.0, 0.0, 128.0), red) > 60);
}

#[test]
fn faces_of_a_box_are_shaded_differently() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, camera) = box_scene(&gpu, false);
    let shot = render(&gpu, &renderer, &target, camera);
    // The camera sees the top, the -X side and the -Y side.
    let top = shot.best_near(Vec3::new(0.0, 0.0, 128.0), brightness);
    let side_x = shot.best_near(Vec3::new(-64.0, 0.0, 64.0), brightness);
    let side_y = shot.best_near(Vec3::new(0.0, -64.0, 64.0), brightness);
    assert!(top > side_x && top > side_y, "{top} {side_x} {side_y}");
}

#[test]
fn the_box_being_drawn_is_outlined_in_yellow() {
    let Some(gpu) = gpu() else { return };
    let renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    let target = renderer.create_target(&gpu.device, [400, 300]);
    let camera = Camera::looking_at(Vec3::new(-420.0, -300.0, 360.0), Vec3::new(0.0, 0.0, 64.0));
    let preview = Aabb::from_corners(Vec3::new(-64.0, -64.0, 0.0), Vec3::new(64.0, 64.0, 128.0));
    let yellow = |p: [u8; 4]| i32::from(p[0].min(p[1])) - i32::from(p[2]);
    let edge = Vec3::new(0.0, -64.0, 128.0);
    let with = render_with_preview(&gpu, &renderer, &target, camera, Some(preview));
    assert!(with.best_near(edge, yellow) > 60);
    let without = render(&gpu, &renderer, &target, camera);
    assert!(
        without.best_near(edge, yellow) < 40,
        "no preview, no outline"
    );
}

#[test]
fn a_replaced_map_is_redrawn_even_at_the_same_revision() {
    // Regression: the drawn map was only rebuilt when the revision numbers
    // changed, so a new map that happened to reach the same numbers kept
    // showing the old one.
    let Some(gpu) = gpu() else { return };
    let (mut renderer, target, camera) = box_scene(&gpu, false);
    let before = render(&gpu, &renderer, &target, camera).pixels;
    let mut other = Document::new();
    let far_away = Brush::cuboid(Aabb::from_corners(
        Vec3::new(5000.0, 5000.0, 0.0),
        Vec3::new(5064.0, 5064.0, 64.0),
    ))
    .unwrap();
    other.execute(Command::AddBrushes(vec![far_away])).unwrap();
    other.clear_selection();
    renderer.update_scene(&gpu.device, &other);
    let after = render(&gpu, &renderer, &target, camera).pixels;
    let changed = before
        .chunks(4)
        .zip(after.chunks(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 2000,
        "the old box must be gone ({changed} pixels changed)"
    );
}

#[test]
fn the_player_figure_is_drawn_and_hidden_behind_brushes() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, camera) = box_scene(&gpu, false);
    let light_blue = |p: [u8; 4]| i32::from(p[2]) - i32::from(p[0]);
    let figure = |x: f32, y: f32| PlayerOutline {
        bounds: Aabb::from_corners(
            Vec3::new(x - 16.0, y - 16.0, 0.0),
            Vec3::new(x + 16.0, y + 16.0, 72.0),
        ),
        eye_height: 64.0,
        color: halberd_render::PLAYER_COLOR,
    };
    // In the open, beside the box: its top edge shows.
    let beside = figure(-160.0, 0.0);
    let shot = render_full(&gpu, &renderer, &target, camera, None, Some(beside));
    let top_edge = Vec3::new(-160.0, -16.0, 72.0);
    assert!(
        shot.best_near(top_edge, light_blue) > 80,
        "the figure shows"
    );
    // Behind the box (from the camera's side), it is hidden.
    let behind = figure(160.0, 160.0);
    let shot = render_full(&gpu, &renderer, &target, camera, None, Some(behind));
    let hidden_edge = Vec3::new(160.0, 144.0, 36.0);
    let on_screen = camera
        .project(hidden_edge, Vec2::new(400.0, 300.0))
        .unwrap();
    assert!(on_screen.x > 0.0 && on_screen.x < 400.0 && on_screen.y > 0.0 && on_screen.y < 300.0);
    assert!(
        shot.best_near(hidden_edge, light_blue) < 40,
        "the box hides it"
    );
}

#[test]
fn side_views_show_the_grid_behind_the_map_in_every_direction() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, _) = box_scene(&gpu, false);
    let background = background_srgb();
    let background = i32::from(background[0]) + i32::from(background[1]) + i32::from(background[2]);
    for side in Side::ALL {
        let mut view = SideCamera::new(side);
        view.center = Vec3::new(0.0, 0.0, 64.0);
        view.height = 600.0;
        let shot = render(&gpu, &renderer, &target, view.camera());
        // The box fills the middle, lit.
        assert!(
            brightness(shot.pixel(200, 150).unwrap()) > background + 60,
            "{side:?}: the box is drawn"
        );
        // Grid lines show around it, all the way to the edges (no fading).
        let lines = (0..400)
            .filter(|&x| brightness(shot.pixel(x, 10).unwrap()) > background + 6)
            .count();
        assert!(lines >= 4, "{side:?}: grid lines across the top: {lines}");
    }
}

#[test]
fn wireframe_shows_light_outlines_and_no_faces() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, camera) = box_scene(&gpu, false);
    let shot = render_mode(
        &gpu,
        &renderer,
        &target,
        camera,
        None,
        None,
        RenderMode::Wireframe,
    );
    let background = background_srgb();
    let background = i32::from(background[0]) + i32::from(background[1]) + i32::from(background[2]);
    // The middle of the top face is see-through now: the Z axis shows.
    assert!(
        shot.best_near(Vec3::new(0.0, 0.0, 64.0), blue) > 60,
        "the axis inside the box shows through"
    );
    // A top corner of the box is outlined in a light colour.
    assert!(shot.best_near(Vec3::new(64.0, -64.0, 128.0), brightness) > background + 150);
}

#[test]
fn textured_mode_shows_a_pattern_on_the_faces() {
    let Some(gpu) = gpu() else { return };
    let (renderer, target, camera) = box_scene(&gpu, false);
    let flat = render_mode(
        &gpu,
        &renderer,
        &target,
        camera,
        None,
        None,
        RenderMode::Flat,
    );
    let textured = render_mode(
        &gpu,
        &renderer,
        &target,
        camera,
        None,
        None,
        RenderMode::Textured,
    );
    // The top face is one colour when flat; textured, its pixels vary.
    // (Sampled away from y = 0, where a texture's edge line runs.)
    let spread = |shot: &Shot| {
        let values: Vec<i32> = (-40..=40)
            .step_by(4)
            .filter_map(|d| {
                let at = shot
                    .camera
                    .project(Vec3::new(d as f32, -30.0, 128.0), Vec2::new(400.0, 300.0))?;
                shot.pixel(at.x as i64, at.y as i64).map(brightness)
            })
            .collect();
        values.iter().max().unwrap() - values.iter().min().unwrap()
    };
    assert!(spread(&flat) < 6, "flat is even: {}", spread(&flat));
    assert!(
        spread(&textured) > 12,
        "textured has a pattern: {}",
        spread(&textured)
    );
}

#[test]
fn textured_mode_shows_a_materials_real_texture_once_it_arrives() {
    let Some(gpu) = gpu() else { return };
    let (mut renderer, target, camera) = box_scene(&gpu, false);
    // The box's material is asked for once.
    assert_eq!(renderer.wanted_textures(), ["DEV/DEV_MEASUREGENERIC01B"]);
    assert!(renderer.wanted_textures().is_empty());
    let green = [20u8, 220, 30, 255].repeat(4 * 4);
    renderer.upload_texture(
        &gpu.device,
        &gpu.queue,
        "dev/dev_measuregeneric01b",
        (4, 4, &green),
        (512, 512),
    );
    let shot = render_mode(
        &gpu,
        &renderer,
        &target,
        camera,
        None,
        None,
        RenderMode::Textured,
    );
    let top = Vec3::new(0.0, -30.0, 128.0);
    assert!(
        shot.best_near(top, green_minus_red) > 80,
        "the top face is the texture's green"
    );
    // Flat mode is unchanged.
    let flat = render_mode(
        &gpu,
        &renderer,
        &target,
        camera,
        None,
        None,
        RenderMode::Flat,
    );
    assert!(flat.best_near(top, green_minus_red) < 20);
}

fn green_minus_red(p: [u8; 4]) -> i32 {
    i32::from(p[1]) - i32::from(p[0])
}

#[test]
fn a_prop_shows_its_model_once_it_arrives_with_its_texture() {
    use halberd_doc::EntityObject;
    use halberd_mdl::example::{ExampleModel, box_mesh, model_files};
    use halberd_render::PropModel;
    let Some(gpu) = gpu() else { return };
    let mut doc = Document::new();
    doc.execute(Command::AddEntity(EntityObject::point(
        "prop_static",
        Vec3::new(0.0, 0.0, 0.0),
        &[("model", "models/crate.mdl"), ("angles", "0 45 0")],
    )))
    .unwrap();
    doc.clear_selection();
    let mut renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    renderer.update_scene(&gpu.device, &doc);
    assert_eq!(renderer.wanted_models(), ["models/crate.mdl"]);
    let target = renderer.create_target(&gpu.device, [400, 300]);
    // Level with the model, so the sky (not the grid) is behind it.
    let camera = Camera::looking_at(Vec3::new(-420.0, -300.0, 60.0), Vec3::new(0.0, 0.0, 64.0));
    // Until the model arrives, the prop is a small marker box.
    // A side of the model, off the Z axis, above the horizon.
    let top = Vec3::new(-40.0, -40.0, 110.0);
    let before = render(&gpu, &renderer, &target, camera);
    let background = background_srgb();
    let background = i32::from(background[0]) + i32::from(background[1]) + i32::from(background[2]);
    let before = before.best_near(top, brightness);
    assert!(before < background + 40, "{before}");
    let files = model_files(&ExampleModel {
        materials: &["crate"],
        meshes: &[box_mesh([-64.0, -64.0, 0.0], [64.0, 64.0, 128.0], 0)],
        ..ExampleModel::default()
    });
    let model = halberd_mdl::Model::parse(&files.mdl, &files.vvd, &files.vtx).unwrap();
    renderer.add_model(
        "models/crate.mdl",
        Some(PropModel::new(&model, &["MODELS/CRATE".into()])),
    );
    renderer.update_scene(&gpu.device, &doc);
    let after = render(&gpu, &renderer, &target, camera);
    assert!(
        after.best_near(top, brightness) > background + 120,
        "the model's top is drawn"
    );
    // Its texture, once loaded, is stretched across each side (0 to 1).
    assert_eq!(renderer.wanted_textures(), ["MODELS/CRATE"]);
    let green = [20u8, 220, 30, 255].repeat(4 * 4);
    renderer.upload_texture(
        &gpu.device,
        &gpu.queue,
        "models/crate",
        (4, 4, &green),
        (1024, 1024),
    );
    let textured = render_mode(
        &gpu,
        &renderer,
        &target,
        camera,
        None,
        None,
        RenderMode::Textured,
    );
    assert!(
        textured.best_near(top, green_minus_red) > 80,
        "the texture shows"
    );
}

#[test]
fn a_thumbnail_shows_the_model_filling_the_picture() {
    use halberd_mdl::example::{ExampleModel, box_mesh, model_files};
    use halberd_render::PropModel;
    let Some(gpu) = gpu() else { return };
    let renderer = ViewportRenderer::new(&gpu.device, best_sample_count(&gpu.adapter));
    let target = renderer.create_target(&gpu.device, [64, 64]);
    let files = model_files(&ExampleModel {
        materials: &["crate"],
        meshes: &[box_mesh([0.0; 3], [8.0, 8.0, 40.0], 0)],
        ..ExampleModel::default()
    });
    let model = halberd_mdl::Model::parse(&files.mdl, &files.vvd, &files.vtx).unwrap();
    let pixels = renderer
        .render_thumbnail(
            &gpu.device,
            &gpu.queue,
            &target,
            &PropModel::new(&model, &[]),
        )
        .unwrap();
    assert_eq!(pixels.len(), 64 * 64 * 4);
    let background = background_srgb();
    let centre = &pixels[(32 * 64 + 32) * 4..][..3];
    assert_ne!(centre, background.as_slice(), "the model is in the middle");
    let corner = &pixels[..3];
    assert_eq!(
        corner,
        background.as_slice(),
        "the background shows around it"
    );
    assert!(
        renderer
            .render_thumbnail(&gpu.device, &gpu.queue, &target, &PropModel::default())
            .is_none()
    );
}
