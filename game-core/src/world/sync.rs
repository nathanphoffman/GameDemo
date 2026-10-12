use super::*;

impl World {
    /// Every change made since the world was generated (only the latest for each cell). With
    /// the seed, this rebuilds the world as it is now.
    pub fn all_edits(&self) -> Vec<BlockChange> {
        self.edits
            .iter()
            .map(|(&pos, &block)| BlockChange { pos, block })
            .collect()
    }

    /// The changes since the last call, in the order they happened.
    pub fn take_changes(&mut self) -> Vec<BlockChange> {
        std::mem::take(&mut self.fresh)
    }

    /// Puts back the changes of a saved game (see `all_edits`) on a freshly generated world.
    /// They count as edits again, so they are saved and sent to new players like any other.
    pub fn load_edits(&mut self, changes: &[BlockChange]) {
        for change in changes {
            let (x, y, z) = change.pos;
            if !Self::in_bounds(x, y, z) {
                continue;
            }
            self.blocks[Self::index(x as usize, y as usize, z as usize)] = change.block;
            if change.block != Block::Air {
                self.top_y = self.top_y.max(y as usize + 1);
            }
            self.mark_dirty(x, z);
            self.edits.insert((x, y, z), change.block);
        }
    }

    /// Makes a change that happened somewhere else (on the host) true here, with no rules
    /// applied: the cell just becomes `block`. Out of range changes are ignored.
    pub fn apply_change(&mut self, change: BlockChange) {
        let (x, y, z) = change.pos;
        if !Self::in_bounds(x, y, z) {
            return;
        }
        self.blocks[Self::index(x as usize, y as usize, z as usize)] = change.block;
        if change.block != Block::Air {
            self.top_y = self.top_y.max(y as usize + 1);
        }
        self.mark_dirty(x, z);
    }
}
