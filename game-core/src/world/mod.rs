//! The block world: its terrain, the changes made to it, and asking it questions.
//!
//! `World` is one struct; what it can do is spread over the files here, one theme each.

mod access;
mod block;
mod dirty;
mod edit;
mod generate;
mod noise;
mod plants;
mod ray;
mod space;
mod spots;
mod sync;
mod terrain;
mod trees;
#[cfg(test)]
mod tests;

pub use block::Block;
pub use noise::hash;
pub(crate) use noise::{fbm_with, hill_height, mountain_height, smoothstep, value_noise};

use crate::biome::Biome;
use std::collections::HashMap;

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const SIZE_X: usize = 1024;
pub const SIZE_Z: usize = 1024;

/// The world is meshed in square chunks of this many columns, so an edit only rebuilds one.
pub const CHUNK: usize = 32;
pub const CHUNKS_X: usize = SIZE_X / CHUNK;
pub const CHUNKS_Z: usize = SIZE_Z / CHUNK;
pub const SIZE_Y: usize = 64;

/// Water fills every column lower than this, up to (not including) this height.
pub const WATER_LEVEL: usize = 6;

/// Small margin so a box resting exactly on a block boundary isn't treated as inside it.
pub const EPS: f32 = 1e-4;

/// What a ray found: the block it hit, and the empty cell in front of the face it hit (None
/// if the ray started inside the block).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayHit {
    pub block: (i32, i32, i32),
    pub place: Option<(i32, i32, i32)>,
}

/// One cell of the world changing to a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockChange {
    pub pos: (i32, i32, i32),
    pub block: Block,
}

pub struct World {
    blocks: Vec<Block>,
    heights: Vec<u8>,
    /// One above the highest non-air block, so meshing can skip the empty sky.
    top_y: usize,
    /// Per column: which biome it is in.
    biomes: Vec<Biome>,
    /// Per chunk: changed since the mesh was last built.
    dirty: Vec<bool>,
    /// Per column: water here belongs to a pond, lake or river that is entirely in the snow
    /// biome, so it is drawn as ice.
    frozen: Vec<bool>,
    /// Every cell that is different from what `generate` made, with what it is now. Anyone
    /// with the seed and this can rebuild the world.
    edits: HashMap<(i32, i32, i32), Block>,
    /// The changes since `take_changes` was last called, oldest first.
    fresh: Vec<BlockChange>,
}
