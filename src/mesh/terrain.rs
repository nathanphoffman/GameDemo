//! Meshes of the blocks, and the outline and overlay shapes drawn on them.

use glam::Vec3;

use super::{Vertex, FACES};
use crate::biome::Biome;
#[cfg(test)]
use crate::world::{CHUNKS_X, CHUNKS_Z};
use crate::world::{Block, World, CHUNK, SIZE_Y};

// Texture layers used by the house plant model.
const PLANT_POT: f32 = 28.0;
const PLANT_SOIL: f32 = 30.0;
const PLANT_LEAF: f32 = 31.0;

/// The parts of the potted plant, in cell coordinates (0 to 1): pot, stem, a leafy bush, a
/// tuft on top, and two leaves spread out in a cross. Face layers are in `FACES` order: +Y,
/// -Y, +X, -X, +Z, -Z.
const PLANT_PARTS: [([f32; 3], [f32; 3], [f32; 6]); 6] = [
    (
        [0.26, 0.0, 0.26],
        [0.74, 0.38, 0.74],
        [PLANT_SOIL, PLANT_POT, PLANT_POT, PLANT_POT, PLANT_POT, PLANT_POT],
    ),
    ([0.46, 0.38, 0.46], [0.54, 0.54, 0.54], [PLANT_LEAF; 6]),
    ([0.30, 0.50, 0.30], [0.70, 0.78, 0.70], [PLANT_LEAF; 6]),
    ([0.38, 0.78, 0.38], [0.62, 0.95, 0.62], [PLANT_LEAF; 6]),
    ([0.10, 0.58, 0.42], [0.90, 0.70, 0.58], [PLANT_LEAF; 6]),
    ([0.42, 0.58, 0.10], [0.58, 0.70, 0.90], [PLANT_LEAF; 6]),
];

/// Adds the potted plant model to a cell.
fn push_plant(vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>, x: i32, y: i32, z: i32) {
    for (min, max, layers) in PLANT_PARTS {
        for (f, face) in FACES.iter().enumerate() {
            let base = vertices.len() as u32;
            for i in 0..4 {
                let c = face.corners[i];
                let mut pos = [0.0f32; 3];
                for (k, origin) in [x, y, z].into_iter().enumerate() {
                    pos[k] = origin as f32 + min[k] + c[k] * (max[k] - min[k]);
                }
                vertices.push(Vertex {
                    pos,
                    uv: face.uv[i],
                    layer: layers[f],
                    shade: face.shade,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
}

/// The six faces of one block, pushed out a hair so they sit on top of the real faces.
pub fn block_overlay(x: i32, y: i32, z: i32, layer: f32) -> ([Vertex; 24], [u32; 36]) {
    const GROW: f32 = 0.003;
    let mut vertices = [Vertex {
        pos: [0.0; 3],
        uv: [0.0; 2],
        layer,
        shade: 1.0,
    }; 24];
    let mut indices = [0u32; 36];
    for (f, face) in FACES.iter().enumerate() {
        for i in 0..4 {
            let c = face.corners[i];
            let at = |offset: i32, c: f32| offset as f32 - GROW + c * (1.0 + 2.0 * GROW);
            vertices[f * 4 + i].pos = [at(x, c[0]), at(y, c[1]), at(z, c[2])];
            vertices[f * 4 + i].uv = face.uv[i];
        }
        let base = (f * 4) as u32;
        indices[f * 6..f * 6 + 6]
            .copy_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

/// A wireframe box around one block, built from 12 thin bars so it has a visible
/// thickness at any distance (GPU lines are always one pixel wide).
pub fn block_outline(x: i32, y: i32, z: i32, layer: f32) -> (Vec<Vertex>, Vec<u32>) {
    let min = Vec3::new(x as f32, y as f32, z as f32);
    box_outline(min, min + Vec3::ONE, layer)
}

/// Same wireframe, around any axis-aligned box.
pub fn box_outline(min_corner: Vec3, max_corner: Vec3, layer: f32) -> (Vec<Vertex>, Vec<u32>) {
    const HALF: f32 = 0.012; // half the bar thickness
    // Corner of the unit cube from a 3-bit index (bit 0 = x, 1 = y, 2 = z).
    const QUADS: [[u32; 4]; 6] = [
        [0, 2, 6, 4],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
        [0, 1, 3, 2],
        [4, 5, 7, 6],
    ];
    let (lo, hi) = (min_corner.to_array(), max_corner.to_array());
    let mut vertices = Vec::with_capacity(96);
    let mut indices = Vec::with_capacity(432);
    for axis in 0..3 {
        for corner in 0..4 {
            // Which two of the other axes this bar sits on (each at 0 or 1).
            let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut min = [0.0f32; 3];
            let mut max = [0.0f32; 3];
            min[axis] = lo[axis] - HALF;
            max[axis] = hi[axis] + HALF;
            for (k, bit) in [(a, corner & 1), (b, corner >> 1)] {
                let edge = if bit == 1 { hi[k] } else { lo[k] };
                min[k] = edge - HALF;
                max[k] = edge + HALF;
            }
            let base = vertices.len() as u32;
            for i in 0..8u32 {
                let pick = |k: usize, bit: u32| if bit == 1 { max[k] } else { min[k] };
                vertices.push(Vertex {
                    pos: [
                        pick(0, i & 1),
                        pick(1, (i >> 1) & 1),
                        pick(2, (i >> 2) & 1),
                    ],
                    uv: [0.0, 0.0],
                    layer,
                    shade: 1.0,
                });
            }
            for q in QUADS {
                let [p, r, s, t] = q.map(|i| base + i);
                indices.extend_from_slice(&[p, r, s, p, s, t]);
            }
        }
    }
    (vertices, indices)
}

/// Every chunk's mesh in one (for tests).
#[cfg(test)]
pub fn build_mesh(world: &World) -> (Vec<Vertex>, Vec<u32>) {
    let (mut vertices, mut indices) = (Vec::new(), Vec::new());
    for cz in 0..CHUNKS_Z {
        for cx in 0..CHUNKS_X {
            let (v, i) = build_chunk(world, cx, cz);
            let base = vertices.len() as u32;
            vertices.extend(v);
            indices.extend(i.into_iter().map(|n| n + base));
        }
    }
    (vertices, indices)
}

/// Builds the mesh of one chunk of columns, containing only the block faces that touch air.
pub fn build_chunk(world: &World, cx: usize, cz: usize) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let (x0, z0) = ((cx * CHUNK) as i32, (cz * CHUNK) as i32);
    for y in 0..world.top_y().min(SIZE_Y) as i32 {
        for z in z0..z0 + CHUNK as i32 {
            for x in x0..x0 + CHUNK as i32 {
                let block = world.get(x, y, z);
                if block == Block::Air {
                    continue;
                }
                // The house plant is a little model, not a cube.
                if block == Block::HousePlant {
                    push_plant(&mut vertices, &mut indices, x, y, z);
                    continue;
                }
                for face in &FACES {
                    let [nx, ny, nz] = face.normal;
                    // Cells holding the plant don't hide the faces next to them. Neither
                    // does liquid water (the lake bed has to be there to see), but water
                    // does hide its own sides, and ice hides everything under it.
                    let beside = world.get(x + nx, y + ny, z + nz);
                    let hidden = match beside {
                        Block::Air | Block::HousePlant => false,
                        Block::Water => {
                            block == Block::Water
                                || world.is_frozen_column(x + nx, z + nz)
                        }
                        _ => true,
                    };
                    if hidden {
                        continue;
                    }
                    if face.kind == 2 && y == 0 {
                        continue; // never visible
                    }
                    let biome = world.biome(x, z);
                    let layer = match (block, face.kind) {
                        (Block::Grass, 0) if biome == Biome::Meadow => 41.0,
                        (Block::Grass, 0) if biome == Biome::Swamp => 42.0,
                        (Block::Dirt, _) if biome == Biome::Swamp => 43.0,
                        (Block::Grass, 0) => 0.0,
                        (Block::Grass, 1) => 1.0,
                        (Block::Snow, 0) => 3.0,
                        (Block::Snow, 1) => 4.0,
                        (Block::Stone, _) => 5.0,
                        (Block::Log, 1) => 6.0,
                        (Block::Log, _) => 7.0,
                        (Block::PineLog, 1) => 20.0,
                        (Block::PineLog, _) => 21.0,
                        (Block::Leaves, _) => 8.0,
                        (Block::PineLeaves, _) => 9.0,
                        (Block::Burning, _) => 22.0,
                        (Block::Coal, _) => 23.0,
                        (Block::Water, _) if world.is_frozen_column(x, z) => 34.0,
                        (Block::Water, _) => 24.0,
                        (Block::WoodFloor, _) => 25.0,
                        (Block::StoneFloor, _) => 26.0,
                        (Block::Forge, _) => 27.0,
                        (Block::Furnace, _) => 29.0,
                        (Block::Sand, _) => 35.0,
                        (Block::RedRock, _) => 36.0,
                        (Block::Clay, _) => 37.0,
                        (Block::Basalt, _) => 38.0,
                        (Block::Magma, _) => 39.0,
                        (Block::Cactus, _) => 40.0,
                        _ => 2.0,
                    };
                    let base = vertices.len() as u32;
                    for i in 0..4 {
                        let c = face.corners[i];
                        vertices.push(Vertex {
                            pos: [x as f32 + c[0], y as f32 + c[1], z as f32 + c[2]],
                            uv: face.uv[i],
                            layer,
                            shade: face.shade,
                        });
                    }
                    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                }
            }
        }
    }
    (vertices, indices)
}

#[cfg(test)]
mod tests {
    use super::*;

#[test]
fn outline_has_twelve_bars_around_the_block() {
    let (v, i) = block_outline(3, 4, 5, 12.0);
    assert_eq!(v.len(), 12 * 8);
    assert_eq!(i.len(), 12 * 36);
    assert!(i.iter().all(|&n| (n as usize) < v.len()));
    for vert in &v {
        for (p, o) in vert.pos.iter().zip([3.0, 4.0, 5.0]) {
            assert!(*p >= o - 0.02 && *p <= o + 1.02);
        }
    }
}

#[test]
fn house_plant_is_a_small_model_that_does_not_hide_its_neighbours() {
    let mut w = World::generate(5);
    let s = w.spawn_point();
    // A cell with open air on every side, a few blocks above the ground.
    let (x, y, z) = (s.x.floor() as i32, s.y.floor() as i32 + 3, s.z.floor() as i32);
    let before = build_mesh(&w).0.len();
    assert!(w.place_block(x, y, z, Block::HousePlant));
    let (vertices, _) = build_mesh(&w);
    assert_eq!(vertices.len(), before + PLANT_PARTS.len() * 6 * 4);

    // The cell was empty with air all around, so every vertex in it is the plant's. They
    // are all well inside the cell sideways (the model is smaller than a cube).
    let in_cell: Vec<&Vertex> = vertices
        .iter()
        .filter(|v| {
            (x as f32..=x as f32 + 1.0).contains(&v.pos[0])
                && (y as f32..=y as f32 + 1.0).contains(&v.pos[1])
                && (z as f32..=z as f32 + 1.0).contains(&v.pos[2])
        })
        .collect();
    assert_eq!(in_cell.len(), PLANT_PARTS.len() * 6 * 4);
    assert!(in_cell.iter().all(|v| {
        v.pos[0] > x as f32 + 0.05
            && v.pos[0] < x as f32 + 0.95
            && v.pos[2] > z as f32 + 0.05
            && v.pos[2] < z as f32 + 0.95
    }));

    // A block next to it still gets its face on the plant's side.
    assert!(w.place_block(x + 1, y, z, Block::Stone));
    let with_stone = build_mesh(&w).0.len();
    assert_eq!(with_stone, before + PLANT_PARTS.len() * 6 * 4 + 6 * 4);
}
}
