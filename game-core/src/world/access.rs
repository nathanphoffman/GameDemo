use super::*;

impl World {
    /// One above the highest non-air block.
    pub fn top_y(&self) -> usize {
        self.top_y
    }

    /// Is the water in column (x, z) part of a frozen-over body?
    pub fn is_frozen_column(&self, x: i32, z: i32) -> bool {
        self.frozen[z as usize * SIZE_X + x as usize]
    }

    /// The biome of the column at (x, z); outside the world it is whatever is nearest.
    pub fn biome(&self, x: i32, z: i32) -> Biome {
        let x = x.clamp(0, SIZE_X as i32 - 1) as usize;
        let z = z.clamp(0, SIZE_Z as i32 - 1) as usize;
        self.biomes[z * SIZE_X + x]
    }

    pub(super) fn index(x: usize, y: usize, z: usize) -> usize {
        (y * SIZE_Z + z) * SIZE_X + x
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Block {
        if x < 0 || y < 0 || z < 0 || x >= SIZE_X as i32 || y >= SIZE_Y as i32 || z >= SIZE_Z as i32
        {
            return Block::Air;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)]
    }

    /// Solid for collision purposes: the floor and the world's sides are solid walls. Water
    /// isn't, you swim through it.
    pub(super) fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        if y < 0 || x < 0 || z < 0 || x >= SIZE_X as i32 || z >= SIZE_Z as i32 {
            return true;
        }
        self.get(x, y, z) != Block::Air && !self.is_liquid(x, y, z)
    }

    /// Is there water you can swim through in the cell containing this point? Ice doesn't
    /// count: that is solid.
    pub fn in_water(&self, p: Vec3) -> bool {
        self.is_liquid(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
    }

    /// Is this cell a block of ice: water in a body that is frozen over?
    pub fn is_ice(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z) == Block::Water && self.frozen[z as usize * SIZE_X + x as usize]
    }

    /// Water that isn't frozen over.
    pub(super) fn is_liquid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get(x, y, z) == Block::Water && !self.frozen[z as usize * SIZE_X + x as usize]
    }

    pub fn in_bounds(x: i32, y: i32, z: i32) -> bool {
        x >= 0 && y >= 0 && z >= 0 && x < SIZE_X as i32 && y < SIZE_Y as i32 && z < SIZE_Z as i32
    }
}
