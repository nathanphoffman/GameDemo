struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xyz = camera position, w = seconds of play (animates rain and wind).
    cam: vec4<f32>,
    // x = fog start, y = fog end, z = overcast (0 clear .. 1 grey), w = dimming.
    fog: vec4<f32>,
    // xy = how far the clouds have drifted (the rain and wind are drawn as streaks in the
    // world instead), z = 1 when the camera is under water.
    sky: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

const CLEAR_HORIZON: vec3<f32> = vec3<f32>(0.62, 0.80, 0.96);
const CLEAR_ZENITH: vec3<f32> = vec3<f32>(0.10, 0.36, 0.85);
const OVERCAST_HORIZON: vec3<f32> = vec3<f32>(0.66, 0.69, 0.73);
const OVERCAST_ZENITH: vec3<f32> = vec3<f32>(0.42, 0.46, 0.53);

// The sky colours, pulled toward grey as the weather closes in and darkened by `dim`.
fn horizon_color() -> vec3<f32> {
    return mix(CLEAR_HORIZON, OVERCAST_HORIZON, g.fog.z) * (1.0 - g.fog.w * 0.5);
}

fn zenith_color() -> vec3<f32> {
    return mix(CLEAR_ZENITH, OVERCAST_ZENITH, g.fog.z) * (1.0 - g.fog.w * 0.5);
}

// ---- sky: fullscreen triangle, colour from view direction ----

// Clouds are a flat layer of noise high above the world. `g.sky.xy` is how far they have
// drifted, which the game works out from the wind.
const CLOUD_HEIGHT: f32 = 100.0;
const CLOUD_SCALE: f32 = 70.0;   // blocks per noise cell

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn cloud_density(p: vec2<f32>) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 4; i++) {
        sum += value_noise(q) * amp;
        q = q * 2.03 + vec2<f32>(17.1, 9.2);
        amp *= 0.5;
    }
    // More overcast means more of the layer counts as cloud.
    let cover = g.fog.z;
    return smoothstep(0.48 - 0.26 * cover, 0.72 - 0.2 * cover, sum);
}

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
    var color = mix(horizon_color(), zenith_color(), t);

    // Clouds: where the view ray crosses the cloud layer. They fade out toward the horizon,
    // where the layer is very far away and would just shimmer.
    if (dir.y > 0.01) {
        let dist = (CLOUD_HEIGHT - g.cam.y) / dir.y;
        let spot = (g.cam.xz + dir.xz * dist) / CLOUD_SCALE + g.sky.xy;
        let density = cloud_density(spot);
        let fade = smoothstep(0.01, 0.2, dir.y);
        // Thicker parts are a little greyer underneath.
        let white = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.62, 0.65, 0.70), g.fog.z);
        let cloud_color = mix(white, white * vec3<f32>(0.78, 0.82, 0.88), density * density) * (1.0 - g.fog.w * 0.4);
        color = mix(color, cloud_color, density * fade * 0.92);
    }
    return vec4(color, 1.0);
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
    let c = textureSample(blocks, block_sampler, in.uv, in.layer).rgb * in.shade * (1.0 - g.fog.w * 0.6);
    let d = length(in.world - g.cam.xyz);
    if (g.sky.z > 0.5) {
        // Under water: everything goes blue and fades out within a few blocks.
        let tinted = mix(c * vec3<f32>(0.55, 0.8, 1.0), vec3<f32>(0.05, 0.22, 0.45), 0.35);
        let murk = smoothstep(2.0, 32.0, d);
        return vec4(mix(tinted, vec3<f32>(0.04, 0.17, 0.38), murk), 1.0);
    }
    let fog = smoothstep(g.fog.x, g.fog.y, d);
    return vec4(mix(c, horizon_color(), fog), 1.0);
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
