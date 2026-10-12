use super::*;

impl World {
    /// Puts a block in an empty cell inside the world. Returns whether it was placed.
    pub fn place_block(&mut self, x: i32, y: i32, z: i32, block: Block) -> bool {
        let inside = x >= 0
            && y >= 0
            && z >= 0
            && x < SIZE_X as i32
            && y < SIZE_Y as i32
            && z < SIZE_Z as i32;
        if !inside
            || block == Block::Air
            || (self.get(x, y, z) != Block::Air && !self.is_liquid(x, y, z))
        {
            return false;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = block;
        self.top_y = self.top_y.max(y as usize + 1);
        self.mark_dirty(x, z);
        self.record(x, y, z, block);
        true
    }

    /// Swaps an existing block for another (e.g. grass for fire). Returns whether it did.
    pub fn replace_block(&mut self, x: i32, y: i32, z: i32, block: Block) -> bool {
        if block == Block::Air || self.get(x, y, z) == Block::Air {
            return false;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = block;
        self.mark_dirty(x, z);
        self.record(x, y, z, block);
        true
    }

    /// Removes a block and returns what it was. The bottom layer is kept so there's never a
    /// hole to the void.
    pub fn remove_block(&mut self, x: i32, y: i32, z: i32) -> Option<Block> {
        let block = self.get(x, y, z);
        if y <= 0 || block == Block::Air {
            return None;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = Block::Air;
        self.mark_dirty(x, z);
        self.record(x, y, z, Block::Air);
        self.flow_into(x, y, z);
        Some(block)
    }

    pub(super) fn set_cell(&mut self, x: i32, y: i32, z: i32, block: Block) {
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = block;
        self.mark_dirty(x, z);
        self.record(x, y, z, block);
    }

    /// Notes that a cell was changed, for `all_edits` and `take_changes`.
    pub(super) fn record(&mut self, x: i32, y: i32, z: i32, block: Block) {
        self.edits.insert((x, y, z), block);
        self.fresh.push(BlockChange { pos: (x, y, z), block });
    }

    /// A cell that was just dug out fills with water if there is water above it or beside
    /// it. The water above doesn't go down: the hole just extends the water.
    pub(super) fn flow_into(&mut self, x: i32, y: i32, z: i32) {
        let wet = [(x, y + 1, z), (x + 1, y, z), (x - 1, y, z), (x, y, z + 1), (x, y, z - 1)]
            .into_iter()
            .any(|(a, b, c)| self.is_liquid(a, b, c));
        if wet {
            self.set_cell(x, y, z, Block::Water);
        }
    }
}
