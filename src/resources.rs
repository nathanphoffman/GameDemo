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
    /// What mining `block` yields, and so what placing one costs.
    ///
    /// Any earth gives grey; snow-topped earth also gives white and grass-topped earth also
    /// gives green. Wood gives brown, and leaves give two green. Brown is the rarest since it
    /// only comes from a few trunk blocks per tree.
    fn value_of(block: Block) -> Resources {
        let (grey, white, green, brown) = match block {
            Block::Air => (0, 0, 0, 0),
            Block::Dirt | Block::Stone => (1, 0, 0, 0),
            Block::Grass => (1, 0, 1, 0),
            Block::Snow => (1, 1, 0, 0),
            Block::Log => (0, 0, 0, 1),
            Block::Leaves | Block::PineLeaves => (0, 0, 2, 0),
        };
        Resources {
            grey,
            white,
            green,
            brown,
        }
    }

    /// Adds what mining `block` yields.
    pub fn gain(&mut self, block: Block) {
        let v = Self::value_of(block);
        self.grey += v.grey;
        self.white += v.white;
        self.green += v.green;
        self.brown += v.brown;
    }

    /// Whether there is enough to place `block`.
    pub fn can_afford(&self, block: Block) -> bool {
        let v = Self::value_of(block);
        block != Block::Air
            && self.grey >= v.grey
            && self.white >= v.white
            && self.green >= v.green
            && self.brown >= v.brown
    }

    /// Pays for placing `block`. Returns false (and takes nothing) if it can't be afforded.
    pub fn spend(&mut self, block: Block) -> bool {
        if !self.can_afford(block) {
            return false;
        }
        let v = Self::value_of(block);
        self.grey -= v.grey;
        self.white -= v.white;
        self.green -= v.green;
        self.brown -= v.brown;
        true
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

    #[test]
    fn placing_costs_what_mining_gives() {
        let all = [
            Block::Dirt,
            Block::Stone,
            Block::Grass,
            Block::Snow,
            Block::Log,
            Block::Leaves,
            Block::PineLeaves,
        ];
        for block in all {
            let mut r = Resources::default();
            r.gain(block);
            // Exactly what was gained pays for exactly one, and leaves nothing.
            assert!(r.can_afford(block), "{block:?}");
            assert!(r.spend(block));
            assert_eq!(r, Resources::default(), "{block:?}");
            assert!(!r.spend(block), "{block:?} can't be paid for twice");
        }
    }

    #[test]
    fn a_grass_block_needs_grey_and_green() {
        let mut r = Resources {
            grey: 5,
            ..Resources::default()
        };
        assert!(!r.can_afford(Block::Grass), "no green");
        assert!(!r.spend(Block::Grass));
        assert_eq!(r.grey, 5, "nothing taken on failure");
        r.green = 1;
        assert!(r.spend(Block::Grass));
        assert_eq!(r.as_array(), [4, 0, 0, 0]);
        assert!(!r.can_afford(Block::Air));
    }
}
