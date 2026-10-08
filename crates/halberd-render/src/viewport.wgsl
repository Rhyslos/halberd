// Halberd viewport shaders: the ground grid, coloured lines (axes and
// outlines) and shaded brush faces.
//
// Colours are written in linear space; the render target is sRGB, so the
// GPU converts them when storing.

struct Frame {
    // World to clip space.
    view_proj: mat4x4<f32>,
    // Camera position (xyz); w unused.
    camera: vec4<f32>,
    // Grid spacings: minor, major, super; w = half the grid's width.
    grid: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// ---------------------------------------------------------------- grid ---

struct GridOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,
};

// One large square on the Z = 0 plane, drawn as two triangles. The grid
// lines themselves are computed per pixel in the fragment shader.
@vertex
fn grid_vs(@builtin(vertex_index) index: u32) -> GridOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let xy = corners[index] * frame.grid.w;
    var out: GridOut;
    out.clip = frame.view_proj * vec4<f32>(xy, 0.0, 1.0);
    out.world = xy;
    return out;
}

// How strongly a pixel lies on a grid line of the given spacing (0 to 1).
// Lines are one pixel wide whatever the distance, and a level fades out
// when its lines would be closer together than a few pixels, which avoids
// shimmering patterns in the distance.
fn grid_line(coord: vec2<f32>, spacing: f32) -> f32 {
    let cell = coord / spacing;
    let per_pixel = max(fwidth(cell), vec2<f32>(1e-6, 1e-6));
    let distance_to_line = abs(fract(cell - 0.5) - 0.5) / per_pixel;
    let on_line = 1.0 - min(min(distance_to_line.x, distance_to_line.y), 1.0);
    let density = max(per_pixel.x, per_pixel.y);
    let level_fade = 1.0 - smoothstep(0.08, 0.25, density);
    return on_line * level_fade;
}

@fragment
fn grid_fs(in: GridOut) -> @location(0) vec4<f32> {
    let minor = grid_line(in.world, frame.grid.x);
    let major = grid_line(in.world, frame.grid.y);
    let super_ = grid_line(in.world, frame.grid.z);

    // Fade the grid towards the horizon, farther when the camera is higher.
    let height = abs(frame.camera.z);
    let reach = max(8192.0, height * 40.0);
    let distance = length(in.world - frame.camera.xy);
    let distance_fade = 1.0 - smoothstep(reach * 0.4, reach, distance);

    let alpha = max(max(minor * 0.22, major * 0.45), super_ * 0.65) * distance_fade;
    if alpha <= 0.002 {
        discard;
    }
    return vec4<f32>(0.20, 0.22, 0.26, alpha);
}

// --------------------------------------------------------------- lines ---

struct LineIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn line_vs(in: LineIn) -> LineOut {
    var out: LineOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn line_fs(in: LineOut) -> @location(0) vec4<f32> {
    return in.color;
}

// ------------------------------------------------------------- brushes ---

struct BrushIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct BrushOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn brush_vs(in: BrushIn) -> BrushOut {
    var out: BrushOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    return out;
}

// Simple fixed lighting from above and to one side, so every face of a box
// has its own shade and shapes read clearly before real lighting exists.
@fragment
fn brush_fs(in: BrushOut) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(0.35, 0.55, 0.85));
    let diffuse = max(dot(normalize(in.normal), light), 0.0);
    let shade = 0.38 + 0.62 * diffuse;
    return vec4<f32>(in.color.rgb * shade, in.color.a);
}
