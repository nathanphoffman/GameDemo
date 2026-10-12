use crate::world::Block;

/// What the player has collected. Order matches `NAMES` and the HUD rows.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resources {
    pub grey: u32,
    pub white: u32,
    pub green: u32,
    pub brown: u32,
}

impl Resources {
    /// Adds what mining `block` yields.
    ///
    /// Any earth gives grey; snow-topped earth also gives white and grass-topped earth also
    /// gives green. Wood gives brown, and leaves give two green. Brown is the rarest since it
    /// only comes from a few trunk blocks per tree.
    pub fn gain(&mut self, block: Block) {
        match block {
            Block::Air => {}
            Block::Dirt | Block::Stone => self.grey += 1,
            Block::Grass => {
                self.grey += 1;
                self.green += 1;
            }
            Block::Snow => {
                self.grey += 1;
                self.white += 1;
            }
            Block::Log => self.brown += 1,
            Block::Leaves | Block::PineLeaves => self.green += 2,
        }
    }

    pub fn as_array(&self) -> [u32; 4] {
        [self.grey, self.white, self.green, self.brown]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mining_gives_the_right_resources() {
        let mut r = Resources::default();
        r.gain(Block::Dirt);
        assert_eq!(r.as_array(), [1, 0, 0, 0]);
        r.gain(Block::Grass);
        assert_eq!(r.as_array(), [2, 0, 1, 0]);
        r.gain(Block::Snow);
        assert_eq!(r.as_array(), [3, 1, 1, 0]);
        r.gain(Block::Stone);
        assert_eq!(r.as_array(), [4, 1, 1, 0]);
        r.gain(Block::Log);
        assert_eq!(r.as_array(), [4, 1, 1, 1]);
        r.gain(Block::Leaves);
        r.gain(Block::PineLeaves);
        assert_eq!(r.as_array(), [4, 1, 5, 1]);
    }
}
