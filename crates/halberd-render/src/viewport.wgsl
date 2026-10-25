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
    // Where the grid lies: x = the axis it is flat across (0 X, 1 Y, 2 Z),
    // y = its position along that axis, z = 1 to fade into the distance;
    // w = how brushes are drawn: 0 flat, 1 textured, 2 wireframe.
    plane: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;

// ---------------------------------------------------------------- grid ---

struct GridOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec2<f32>,
};

// One large square, flat across the axis `frame.plane` names (the ground,
// Z = 0, in the 3D view), drawn as two triangles. The grid lines themselves
// are computed per pixel in the fragment shader.
@vertex
fn grid_vs(@builtin(vertex_index) index: u32) -> GridOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let xy = corners[index] * frame.grid.w;
    var world = vec3<f32>(xy, frame.plane.y);
    if frame.plane.x < 0.5 {
        world = vec3<f32>(frame.plane.y, xy.x, xy.y);
    } else if frame.plane.x < 1.5 {
        world = vec3<f32>(xy.x, frame.plane.y, xy.y);
    }
    var out: GridOut;
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
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
    var distance_fade = 1.0 - smoothstep(reach * 0.4, reach, distance);
    if frame.plane.z < 0.5 {
        distance_fade = 1.0;
    }

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
    // Without faces, the dark outlines that frame solid faces would vanish
    // into the background: draw them light instead, keeping the hint of
    // colour entity outlines carry (teal for brush entities, purple for
    // point entities).
    let dark = max(max(in.color.r, in.color.g), in.color.b) < 0.08;
    if frame.plane.w > 1.5 && dark {
        let tint = (in.color.rgb - vec3<f32>(0.02, 0.02, 0.025)) * 5.0;
        out.color = vec4<f32>(vec3<f32>(0.55, 0.57, 0.62) + tint, 1.0);
    }
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
    // Texture coordinates, material hue, and 1 if the face has a material
    // (2 if it is also selected).
    @location(3) texture: vec4<f32>,
};

struct BrushOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) texture: vec4<f32>,
};

@vertex
fn brush_vs(in: BrushIn) -> BrushOut {
    var out: BrushOut;
    out.clip = frame.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    out.texture = in.texture;
    return out;
}

// A colour from a hue (0 to 1), muted, in linear space.
fn hue_color(hue: f32) -> vec3<f32> {
    let k = vec3<f32>(0.0, 2.0 / 3.0, 1.0 / 3.0);
    let rgb = clamp(abs(fract(vec3<f32>(hue) + k) * 6.0 - 3.0) - 1.0, vec3<f32>(0.0), vec3<f32>(1.0));
    return mix(vec3<f32>(0.32), rgb * 0.55, 0.55);
}

// The Textured mode's stand-in for a texture, until real ones are read: the
// material's own colour, a faint checker every quarter texture and a line
// at each texture's edge, so alignment, scale and rotation show.
fn material_pattern(texture: vec4<f32>) -> vec3<f32> {
    let base = hue_color(texture.z);
    let uv = texture.xy;
    let quarter = floor(uv * 4.0);
    // fract instead of integers: no overflow however far the coordinates go.
    let checker = fract((quarter.x + quarter.y) * 0.5) > 0.25;
    var color = base * select(1.0, 0.86, checker);
    let per_pixel = max(fwidth(uv), vec2<f32>(1e-6));
    let edge = abs(fract(uv - 0.5) - 0.5) / per_pixel;
    let on_edge = 1.0 - min(min(edge.x, edge.y), 1.0);
    color = mix(color, base * 1.6 + 0.05, on_edge * 0.7);
    return color;
}

// Simple fixed lighting from above and to one side, so every face of a box
// has its own shade and shapes read clearly before real lighting exists.
@fragment
fn brush_fs(in: BrushOut) -> @location(0) vec4<f32> {
    let light = normalize(vec3<f32>(0.35, 0.55, 0.85));
    let diffuse = max(dot(normalize(in.normal), light), 0.0);
    let shade = 0.38 + 0.62 * diffuse;
    var color = in.color.rgb;
    if frame.plane.w > 0.5 && frame.plane.w < 1.5 && in.texture.w > 0.5 {
        let pattern = material_pattern(in.texture);
        // Selected faces keep a red tint so the selection still shows.
        let selected = in.texture.w > 1.5;
        color = select(pattern, mix(pattern, in.color.rgb, 0.55), selected);
    }
    return vec4<f32>(color * shade, in.color.a);
}
