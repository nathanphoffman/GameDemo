struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

const HORIZON: vec3<f32> = vec3<f32>(0.62, 0.80, 0.96);
const ZENITH: vec3<f32> = vec3<f32>(0.10, 0.36, 0.85);

// ---- sky: fullscreen triangle, colour from view direction ----

struct SkyOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn sky_vs(@builtin(vertex_index) i: u32) -> SkyOut {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: SkyOut;
    o.pos = vec4(p[i], 0.0, 1.0);
    o.ndc = p[i];
    return o;
}

@fragment
fn sky_fs(in: SkyOut) -> @location(0) vec4<f32> {
    let far = g.inv_view_proj * vec4(in.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - g.cam.xyz);
    let t = pow(clamp(dir.y, 0.0, 1.0), 0.5);
    return vec4(mix(HORIZON, ZENITH, t), 1.0);
}

// ---- blocks ----

@group(0) @binding(1) var blocks: texture_2d_array<f32>;
@group(0) @binding(2) var block_sampler: sampler;

struct BlockOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) shade: f32,
    @location(3) @interpolate(flat) layer: i32,
};

@vertex
fn block_vs(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) layer: f32,
    @location(3) shade: f32,
) -> BlockOut {
    var o: BlockOut;
    o.pos = g.view_proj * vec4(pos, 1.0);
    o.uv = uv;
    o.world = pos;
    o.shade = shade;
    o.layer = i32(round(layer));
    return o;
}

@fragment
fn block_fs(in: BlockOut) -> @location(0) vec4<f32> {
    let c = textureSample(blocks, block_sampler, in.uv, in.layer).rgb * in.shade;
    let d = length(in.world - g.cam.xyz);
    let fog = smoothstep(70.0, 230.0, d);
    return vec4(mix(c, HORIZON, fog), 1.0);
}

// Crack overlay: same vertices as blocks, but transparent where there is no crack.
@fragment
fn crack_fs(in: BlockOut) -> @location(0) vec4<f32> {
    let c = textureSample(blocks, block_sampler, in.uv, in.layer);
    if (c.a < 0.5) {
        discard;
    }
    return vec4(c.rgb, 1.0);
}

// ---- HUD (crosshair, counter, mode label) ----

struct HudOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn hud_vs(@location(0) pos: vec2<f32>, @location(1) color: vec3<f32>) -> HudOut {
    var o: HudOut;
    o.pos = vec4(pos, 0.0, 1.0);
    o.color = color;
    return o;
}

@fragment
fn hud_fs(in: HudOut) -> @location(0) vec4<f32> {
    return vec4(in.color, 1.0);
}
