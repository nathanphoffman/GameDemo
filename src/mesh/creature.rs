//! Meshes of the creatures: their bodies and floating health bars.

use glam::Vec3;

use super::{Vertex, FACES};
use crate::enemy::{Enemy, Kind};

// Texture layers (see textures.rs).
const BONE: f32 = 13.0;
const SKULL: f32 = 14.0;
const RIBS: f32 = 15.0;
const FUR: f32 = 18.0;
const DEER_FACE: f32 = 19.0;
const WOLF: f32 = 44.0;
const WOLF_FACE: f32 = 45.0;
const BOAR: f32 = 46.0;
const BOAR_FACE: f32 = 47.0;
const ZOMBIE: f32 = 48.0;
const ZOMBIE_FACE: f32 = 49.0;
const SCORPION: f32 = 50.0;
const SCORPION_FACE: f32 = 51.0;
const GOLEM: f32 = 52.0;
const GOLEM_FACE: f32 = 53.0;
const YETI: f32 = 54.0;
const YETI_FACE: f32 = 55.0;
const BAR_BACK: f32 = 12.0; // near-black border
const BAR_LOST: f32 = 16.0; // red
const BAR_HEALTH: f32 = 17.0; // green

/// Health bar size and how high it floats above the head, in blocks.
const BAR_WIDTH: f32 = 0.9;
const BAR_HEIGHT: f32 = 0.12;
const BAR_BORDER: f32 = 0.025;
const BAR_LIFT: f32 = 0.35;
/// Each bar layer sits this much closer to the viewer so they don't fight in the depth buffer.
const BAR_LAYER_GAP: f32 = 0.01;

/// How many enemies can be drawn at once, and the most vertices one of them can need.
pub const MAX_ENEMIES: usize = 8;
const MAX_PARTS: usize = 10;
pub const MAX_VERTICES_PER_ENEMY: usize = MAX_PARTS * 6 * 4 + 3 * 4;

/// One box of the body. `layers` is indexed like `FACES`: +Y, -Y, +X, -X, +Z (front), -Z.
struct Part {
    min: [f32; 3],
    max: [f32; 3],
    layers: [f32; 6],
}

fn part(min: [f32; 3], max: [f32; 3], layers: [f32; 6]) -> Part {
    Part { min, max, layers }
}

fn parts(kind: Kind) -> Vec<Part> {
    match kind {
        Kind::Skeleton => vec![
            // Head, with the face on the front.
            part(
                [-0.25, 1.5, -0.25],
                [0.25, 2.0, 0.25],
                [BONE, BONE, BONE, BONE, SKULL, BONE],
            ),
            // Torso, with ribs on the front.
            part(
                [-0.25, 0.75, -0.125],
                [0.25, 1.5, 0.125],
                [BONE, BONE, BONE, BONE, RIBS, BONE],
            ),
            // Arms.
            part([0.25, 0.75, -0.0625], [0.375, 1.5, 0.0625], [BONE; 6]),
            part([-0.375, 0.75, -0.0625], [-0.25, 1.5, 0.0625], [BONE; 6]),
            // Legs.
            part([0.03, 0.0, -0.0625], [0.16, 0.75, 0.0625], [BONE; 6]),
            part([-0.16, 0.0, -0.0625], [-0.03, 0.75, 0.0625], [BONE; 6]),
        ],
        Kind::Deer => vec![
            // Body, longer front to back; the head end is +Z.
            part([-0.25, 0.65, -0.5], [0.25, 1.15, 0.5], [FUR; 6]),
            // Legs.
            part([0.1, 0.0, 0.3], [0.22, 0.65, 0.42], [FUR; 6]),
            part([-0.22, 0.0, 0.3], [-0.1, 0.65, 0.42], [FUR; 6]),
            part([0.1, 0.0, -0.42], [0.22, 0.65, -0.3], [FUR; 6]),
            part([-0.22, 0.0, -0.42], [-0.1, 0.65, -0.3], [FUR; 6]),
            // Neck and head, with the face on the front.
            part([-0.1, 1.0, 0.3], [0.1, 1.55, 0.5], [FUR; 6]),
            part(
                [-0.13, 1.4, 0.38],
                [0.13, 1.68, 0.74],
                [FUR, FUR, FUR, FUR, DEER_FACE, FUR],
            ),
            // Antlers.
            part([0.07, 1.68, 0.42], [0.12, 1.98, 0.5], [BONE; 6]),
            part([-0.12, 1.68, 0.42], [-0.07, 1.98, 0.5], [BONE; 6]),
        ],
        Kind::Wolf => vec![
            // Body, with the head end at +Z.
            part([-0.2, 0.45, -0.5], [0.2, 0.85, 0.5], [WOLF; 6]),
            // Legs.
            part([0.06, 0.0, 0.3], [0.18, 0.45, 0.42], [WOLF; 6]),
            part([-0.18, 0.0, 0.3], [-0.06, 0.45, 0.42], [WOLF; 6]),
            part([0.06, 0.0, -0.42], [0.18, 0.45, -0.3], [WOLF; 6]),
            part([-0.18, 0.0, -0.42], [-0.06, 0.45, -0.3], [WOLF; 6]),
            // Head, with the face on the front, ears and a tail.
            part(
                [-0.17, 0.62, 0.45],
                [0.17, 1.0, 0.85],
                [WOLF, WOLF, WOLF, WOLF, WOLF_FACE, WOLF],
            ),
            part([0.06, 1.0, 0.5], [0.15, 1.12, 0.6], [WOLF; 6]),
            part([-0.15, 1.0, 0.5], [-0.06, 1.12, 0.6], [WOLF; 6]),
            part([-0.05, 0.7, -0.8], [0.05, 0.82, -0.5], [WOLF; 6]),
        ],
        Kind::Boar => vec![
            part([-0.35, 0.35, -0.55], [0.35, 0.95, 0.55], [BOAR; 6]),
            // Legs.
            part([0.12, 0.0, 0.3], [0.3, 0.35, 0.48], [BOAR; 6]),
            part([-0.3, 0.0, 0.3], [-0.12, 0.35, 0.48], [BOAR; 6]),
            part([0.12, 0.0, -0.48], [0.3, 0.35, -0.3], [BOAR; 6]),
            part([-0.3, 0.0, -0.48], [-0.12, 0.35, -0.3], [BOAR; 6]),
            // Head with a snout on the front, and two tusks.
            part(
                [-0.28, 0.45, 0.5],
                [0.28, 0.95, 0.95],
                [BOAR, BOAR, BOAR, BOAR, BOAR_FACE, BOAR],
            ),
            part([0.18, 0.4, 0.88], [0.24, 0.55, 0.96], [BONE; 6]),
            part([-0.24, 0.4, 0.88], [-0.18, 0.55, 0.96], [BONE; 6]),
        ],
        Kind::BogZombie => vec![
            part(
                [-0.25, 1.5, -0.25],
                [0.25, 2.0, 0.25],
                [ZOMBIE, ZOMBIE, ZOMBIE, ZOMBIE, ZOMBIE_FACE, ZOMBIE],
            ),
            part([-0.25, 0.75, -0.15], [0.25, 1.5, 0.15], [ZOMBIE; 6]),
            // Arms held out in front.
            part([0.25, 1.15, 0.0], [0.4, 1.4, 0.8], [ZOMBIE; 6]),
            part([-0.4, 1.15, 0.0], [-0.25, 1.4, 0.8], [ZOMBIE; 6]),
            // Legs.
            part([0.02, 0.0, -0.1], [0.24, 0.75, 0.1], [ZOMBIE; 6]),
            part([-0.24, 0.0, -0.1], [-0.02, 0.75, 0.1], [ZOMBIE; 6]),
        ],
        Kind::Scorpion => vec![
            // Body with the eyes on the front.
            part(
                [-0.4, 0.2, -0.5],
                [0.4, 0.6, 0.5],
                [SCORPION, SCORPION, SCORPION, SCORPION, SCORPION_FACE, SCORPION],
            ),
            // Legs.
            part([0.4, 0.0, 0.2], [0.6, 0.2, 0.3], [SCORPION; 6]),
            part([-0.6, 0.0, 0.2], [-0.4, 0.2, 0.3], [SCORPION; 6]),
            part([0.4, 0.0, -0.3], [0.6, 0.2, -0.2], [SCORPION; 6]),
            part([-0.6, 0.0, -0.3], [-0.4, 0.2, -0.2], [SCORPION; 6]),
            // Claws.
            part([0.25, 0.25, 0.5], [0.55, 0.5, 0.95], [SCORPION; 6]),
            part([-0.55, 0.25, 0.5], [-0.25, 0.5, 0.95], [SCORPION; 6]),
            // Tail curling up over the back.
            part([-0.1, 0.4, -0.9], [0.1, 0.6, -0.5], [SCORPION; 6]),
            part([-0.1, 0.6, -1.0], [0.1, 0.95, -0.8], [SCORPION; 6]),
            part([-0.07, 0.95, -0.9], [0.07, 1.15, -0.6], [BONE; 6]),
        ],
        Kind::Golem => vec![
            part([-0.5, 0.9, -0.3], [0.5, 1.9, 0.3], [GOLEM; 6]),
            part(
                [-0.25, 1.9, -0.25],
                [0.25, 2.3, 0.25],
                [GOLEM, GOLEM, GOLEM, GOLEM, GOLEM_FACE, GOLEM],
            ),
            // Heavy arms and legs.
            part([0.5, 0.7, -0.2], [0.8, 1.9, 0.2], [GOLEM; 6]),
            part([-0.8, 0.7, -0.2], [-0.5, 1.9, 0.2], [GOLEM; 6]),
            part([0.05, 0.0, -0.2], [0.4, 0.9, 0.2], [GOLEM; 6]),
            part([-0.4, 0.0, -0.2], [-0.05, 0.9, 0.2], [GOLEM; 6]),
        ],
        Kind::Yeti => vec![
            part([-0.45, 0.8, -0.3], [0.45, 1.7, 0.3], [YETI; 6]),
            part(
                [-0.3, 1.7, -0.3],
                [0.3, 2.25, 0.3],
                [YETI, YETI, YETI, YETI, YETI_FACE, YETI],
            ),
            // Long arms and thick legs.
            part([0.45, 0.6, -0.15], [0.7, 1.6, 0.15], [YETI; 6]),
            part([-0.7, 0.6, -0.15], [-0.45, 1.6, 0.15], [YETI; 6]),
            part([0.05, 0.0, -0.2], [0.4, 0.8, 0.2], [YETI; 6]),
            part([-0.4, 0.0, -0.2], [-0.05, 0.8, 0.2], [YETI; 6]),
        ],
    }
}

/// Body and health bar vertices, with the front (and the bar) turned toward `look_at`.
pub fn enemy_mesh(e: &Enemy, look_at: Vec3) -> Vec<Vertex> {
    let pos = e.pos();
    let front = Vec3::new(look_at.x - pos.x, 0.0, look_at.z - pos.z)
        .try_normalize()
        .unwrap_or(Vec3::Z);
    let right = Vec3::new(front.z, 0.0, -front.x);

    let mut out = Vec::with_capacity(MAX_VERTICES_PER_ENEMY);
    for part in parts(e.kind) {
        for (f, face) in FACES.iter().enumerate() {
            for i in 0..4 {
                let c = face.corners[i];
                let mut local = [0.0f32; 3];
                for k in 0..3 {
                    let size = part.max[k] - part.min[k];
                    local[k] = part.min[k] + c[k] * size;
                }
                let p = pos + right * local[0] + Vec3::Y * local[1] + front * local[2];
                out.push(Vertex {
                    pos: p.to_array(),
                    uv: face.uv[i],
                    layer: part.layers[f],
                    shade: face.shade,
                });
            }
        }
    }
    push_health_bar(e, &mut out, pos, front, right);
    out
}

/// Three flat quads facing the viewer: border, red (missing health), green (health left).
/// The green one is anchored to one end so it shrinks as he takes damage.
fn push_health_bar(e: &Enemy, out: &mut Vec<Vertex>, pos: Vec3, front: Vec3, right: Vec3) {
    let max = e.kind.max_health() as f32;
    let fraction = (e.health.max(0) as f32 / max).clamp(0.0, 1.0);
    let center = pos + Vec3::Y * (e.kind.height() + BAR_LIFT);
    let (half_w, half_h) = (BAR_WIDTH / 2.0, BAR_HEIGHT / 2.0);

    // (left edge, right edge, half height, layer, how far toward the viewer)
    let quads = [
        (-half_w - BAR_BORDER, half_w + BAR_BORDER, half_h + BAR_BORDER, BAR_BACK, 0.0),
        (-half_w, half_w, half_h, BAR_LOST, BAR_LAYER_GAP),
        (
            -half_w,
            -half_w + BAR_WIDTH * fraction,
            half_h,
            BAR_HEALTH,
            2.0 * BAR_LAYER_GAP,
        ),
    ];
    for (left, right_edge, height, layer, lift) in quads {
        let origin = center + front * (0.1 + lift);
        for (x, y) in [(left, -height), (left, height), (right_edge, height), (right_edge, -height)] {
            out.push(Vertex {
                pos: (origin + right * x + Vec3::Y * y).to_array(),
                uv: [0.0, 0.0],
                layer,
                shade: 1.0,
            });
        }
    }
}

/// Index list for any number of enemy quads, up to `vertex_count` vertices. Every mesh is a
/// run of 4-vertex quads, so a prefix of this list fits whatever is currently uploaded.
pub fn indices(vertex_count: usize) -> Vec<u32> {
    let mut out = Vec::with_capacity(vertex_count / 4 * 6);
    for quad in 0..(vertex_count / 4) as u32 {
        let b = quad * 4;
        out.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

#[test]
fn every_kind_fits_in_the_buffers() {
    for kind in Kind::ALL {
        let e = Enemy::new(1, kind, Vec3::new(10.0, 5.0, 10.0));
        let v = enemy_mesh(&e, Vec3::ZERO);
        assert!(v.len() <= MAX_VERTICES_PER_ENEMY, "{kind:?}");
        assert_eq!(v.len() % 4, 0);
        assert!(parts(kind).len() <= MAX_PARTS);
    }
}

#[test]
fn health_bar_shrinks_with_health() {
    for kind in Kind::ALL {
        let mut e = Enemy::new(1, kind, Vec3::new(10.0, 5.0, 10.0));
        let width = |e: &Enemy| {
            // The green quad is the last four vertices.
            let v = enemy_mesh(e, Vec3::ZERO);
            let g = &v[v.len() - 4..];
            assert_eq!(g[0].layer, BAR_HEALTH);
            (Vec3::from(g[3].pos) - Vec3::from(g[0].pos)).length()
        };
        assert!((width(&e) - BAR_WIDTH).abs() < 1e-4);
        e.health = kind.max_health() / 2;
        let fraction = e.health as f32 / kind.max_health() as f32;
        assert!((width(&e) - BAR_WIDTH * fraction).abs() < 1e-4, "{kind:?}");
    }
}
}
