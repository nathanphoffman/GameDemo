struct Globals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam: vec4<f32>,
};

@group(0) @binding(0) var<uniform> g: Globals;

const HORIZON: vec3<f32> = vec3<f32>(0.62, 0.80, 0.96);
const ZENITH: vec3<f32> = vec3<f32>(0.10, 0.36, 0.85);

// ---- sky: fullscreen triangle, colour from view direction ----

// Clouds are a flat layer of drifting noise high above the world. `g.cam.w` is the time in
// seconds.
const CLOUD_HEIGHT: f32 = 100.0;
const CLOUD_SCALE: f32 = 70.0;   // blocks per noise cell
const CLOUD_DRIFT: vec2<f32> = vec2<f32>(0.018, 0.007); // noise cells per second

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
    return smoothstep(0.48, 0.72, sum);
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
    var color = mix(HORIZON, ZENITH, t);

    // Clouds: where the view ray crosses the cloud layer. They fade out toward the horizon,
    // where the layer is very far away and would just shimmer.
    if (dir.y > 0.01) {
        let dist = (CLOUD_HEIGHT - g.cam.y) / dir.y;
        let spot = (g.cam.xz + dir.xz * dist) / CLOUD_SCALE + CLOUD_DRIFT * g.cam.w;
        let density = cloud_density(spot);
        let fade = smoothstep(0.01, 0.2, dir.y);
        // Thicker parts are a little greyer underneath.
        let cloud_color = mix(vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(0.80, 0.84, 0.90), density * density);
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
