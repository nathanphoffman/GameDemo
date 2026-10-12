use serde::{Deserialize, Serialize};

use crate::world::Block;

/// Seconds between conversion attempts.
pub const CONVERT_INTERVAL: f32 = 10.0;
/// How many of a resource go into one dense one.
pub const CONVERT_COST: u32 = 4;

/// What the player has collected. The five kinds are in HUD row order: grey, white, green,
/// brown, black.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub grey: u32,
    pub white: u32,
    pub green: u32,
    pub brown: u32,
    /// Coal, from burnt wood and leaves.
    pub black: u32,
    /// Refined, "triangle" versions of each kind, in the same order. Flint (dense grey) can be
    /// applied to blocks; the rest are only collected for now.
    pub dense: [u32; 5],
}

impl Resources {
    /// What mining `block` yields, and so what placing one costs.
    ///
    /// Any earth gives grey; snow-topped earth also gives white and grass-topped earth also
    /// gives green. Oak wood gives brown and oak leaves give two green. Pines are the other
    /// way around: their wood is worth two brown, but their needles only one green.
    pub(crate) fn value_of(block: Block) -> Resources {
        let (grey, white, green, brown, black) = match block {
            // Burning blocks give nothing; the coal they turn into does. Water and the
            // crafted blocks give nothing either.
            Block::Air
            | Block::Burning
            | Block::Water
            | Block::WoodFloor
            | Block::StoneFloor
            | Block::Forge
            | Block::Furnace
            | Block::HousePlant => (0, 0, 0, 0, 0),
            Block::Dirt | Block::Stone => (1, 0, 0, 0, 0),
            Block::Sand | Block::RedRock | Block::Clay | Block::Basalt => (1, 0, 0, 0, 0),
            // Cooled magma still has some fuel in it.
            Block::Magma => (1, 0, 0, 0, 1),
            Block::Cactus => (0, 0, 2, 0, 0),
            Block::Grass => (1, 0, 1, 0, 0),
            Block::Snow => (1, 1, 0, 0, 0),
            Block::Log => (0, 0, 0, 1, 0),
            Block::PineLog => (0, 0, 0, 2, 0),
            Block::Leaves => (0, 0, 2, 0, 0),
            Block::PineLeaves => (0, 0, 1, 0, 0),
            Block::Coal => (0, 0, 0, 0, 1),
        };
        Resources {
            grey,
            white,
            green,
            brown,
            black,
            dense: [0; 5],
        }
    }

    /// The dense resource that was used up to build this crafted block, which mining it gives
    /// back (0 = grey ... 4 = black). One of each, matching what the apply effects cost.
    fn dense_refund(block: Block) -> Option<usize> {
        match block {
            Block::WoodFloor => Some(3),
            Block::StoneFloor => Some(0),
            Block::Forge | Block::Furnace => Some(4),
            Block::HousePlant => Some(2),
            _ => None,
        }
    }

    /// Adds what mining `block` yields: the raw resources for natural blocks, or the dense
    /// resource that went into building a crafted one.
    pub fn gain(&mut self, block: Block) {
        let v = Self::value_of(block);
        self.grey += v.grey;
        self.white += v.white;
        self.green += v.green;
        self.brown += v.brown;
        self.black += v.black;
        if let Some(kind) = Self::dense_refund(block) {
            self.dense[kind] += 1;
        }
    }

    /// Blocks that are paid for with one dense resource instead of raw ones, and which kind
    /// (0 = grey ... 4 = black). Placing a wooden floor cube costs a dense brown.
    fn dense_price(block: Block) -> Option<usize> {
        match block {
            Block::WoodFloor => Some(3),
            _ => None,
        }
    }

    /// Whether there is enough to place `block`.
    pub fn can_afford(&self, block: Block) -> bool {
        if let Some(kind) = Self::dense_price(block) {
            return self.dense[kind] >= 1;
        }
        let v = Self::value_of(block);
        // Fire, water and most crafted blocks can't be bought, and air isn't a block.
        !matches!(
            block,
            Block::Air
                | Block::Burning
                | Block::Water
                | Block::StoneFloor
                | Block::Forge
                | Block::Furnace
                | Block::HousePlant
        )
            && self.grey >= v.grey
            && self.white >= v.white
            && self.green >= v.green
            && self.brown >= v.brown
            && self.black >= v.black
    }

    /// Pays for placing `block`. Returns false (and takes nothing) if it can't be afforded.
    pub fn spend(&mut self, block: Block) -> bool {
        if !self.can_afford(block) {
            return false;
        }
        if let Some(kind) = Self::dense_price(block) {
            self.dense[kind] -= 1;
            return true;
        }
        let v = Self::value_of(block);
        self.grey -= v.grey;
        self.white -= v.white;
        self.green -= v.green;
        self.brown -= v.brown;
        self.black -= v.black;
        true
    }

    /// Uses up `count` dense resources of `kind` (0 = grey ... 4 = black), if there are that
    /// many. Takes nothing otherwise.
    pub fn spend_dense(&mut self, kind: usize, count: u32) -> bool {
        match self.dense.get_mut(kind) {
            Some(n) if *n >= count => {
                *n -= count;
                true
            }
            _ => false,
        }
    }

    pub fn as_array(&self) -> [u32; 5] {
        [self.grey, self.white, self.green, self.brown, self.black]
    }

    /// A generous pile of everything, raw and dense, for trying things out.
    pub fn test_kit() -> Resources {
        Resources {
            grey: 100,
            white: 100,
            green: 100,
            brown: 100,
            black: 100,
            dense: [25; 5],
        }
    }

    fn raw_mut(&mut self, kind: usize) -> &mut u32 {
        match kind {
            0 => &mut self.grey,
            1 => &mut self.white,
            2 => &mut self.green,
            3 => &mut self.brown,
            _ => &mut self.black,
        }
    }
}

/// The conversion pipeline: one kind at a time is selected, and every `CONVERT_INTERVAL`
/// seconds, if `CONVERT_COST` of it are on hand, they are turned into one dense one.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Converter {
    selected: Option<usize>,
    progress: f32,
}

impl Converter {
    /// Selects `kind` (0 = grey ... 4 = black), or turns the pipeline off if it already was.
    pub fn toggle(&mut self, kind: usize) {
        if kind > 4 {
            return;
        }
        self.selected = if self.selected == Some(kind) {
            None
        } else {
            Some(kind)
        };
        self.progress = 0.0;
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// How far through the current wait, from 0 to 1.
    pub fn fraction(&self) -> f32 {
        (self.progress / CONVERT_INTERVAL).clamp(0.0, 1.0)
    }

    /// Advances the clock. Returns true if a dense resource was made.
    pub fn update(&mut self, dt: f32, resources: &mut Resources) -> bool {
        let Some(kind) = self.selected else {
            return false;
        };
        self.progress += dt;
        let mut made = false;
        while self.progress >= CONVERT_INTERVAL {
            self.progress -= CONVERT_INTERVAL;
            let have = resources.raw_mut(kind);
            if *have >= CONVERT_COST {
                *have -= CONVERT_COST;
                resources.dense[kind] += 1;
                made = true;
            }
        }
        made
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mining_gives_the_right_resources() {
        let mut r = Resources::default();
        r.gain(Block::Dirt);
        assert_eq!(r.as_array(), [1, 0, 0, 0, 0]);
        r.gain(Block::Grass);
        assert_eq!(r.as_array(), [2, 0, 1, 0, 0]);
        r.gain(Block::Snow);
        assert_eq!(r.as_array(), [3, 1, 1, 0, 0]);
        r.gain(Block::Stone);
        assert_eq!(r.as_array(), [4, 1, 1, 0, 0]);
        r.gain(Block::Log);
        assert_eq!(r.as_array(), [4, 1, 1, 1, 0]);
        r.gain(Block::Leaves);
        assert_eq!(r.as_array(), [4, 1, 3, 1, 0]);
        // Pines: wood is worth double, needles are worth half of an oak's leaves.
        r.gain(Block::PineLog);
        assert_eq!(r.as_array(), [4, 1, 3, 3, 0]);
        r.gain(Block::PineLeaves);
        assert_eq!(r.as_array(), [4, 1, 4, 3, 0]);
    }

    #[test]
    fn placing_costs_what_mining_gives() {
        let all = [
            Block::Dirt,
            Block::Stone,
            Block::Grass,
            Block::Snow,
            Block::Log,
            Block::PineLog,
            Block::Leaves,
            Block::PineLeaves,
            Block::Coal,
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
        assert_eq!(r.as_array(), [4, 0, 0, 0, 0]);
        assert!(!r.can_afford(Block::Air));
    }

    #[test]
    fn converter_turns_four_into_one_every_ten_seconds() {
        let mut r = Resources {
            green: 9,
            grey: 3,
            ..Resources::default()
        };
        let mut c = Converter::default();
        // Nothing selected: nothing happens.
        assert!(!c.update(100.0, &mut r));
        assert_eq!(r.dense, [0; 5]);

        c.toggle(2); // green
        assert!(!c.update(9.9, &mut r), "not yet");
        assert_eq!((r.green, r.dense[2]), (9, 0));
        assert!(c.update(0.2, &mut r));
        assert_eq!((r.green, r.dense[2]), (5, 1));
        assert_eq!(r.grey, 3, "other kinds untouched");

        assert!(c.update(10.0, &mut r));
        assert_eq!((r.green, r.dense[2]), (1, 2));
        // One green left: the next tick has too little, so nothing is made.
        assert!(!c.update(10.0, &mut r));
        assert_eq!((r.green, r.dense[2]), (1, 2));
    }

    #[test]
    fn converter_selection_toggles_and_restarts_the_clock() {
        let mut r = Resources {
            grey: 8,
            white: 8,
            ..Resources::default()
        };
        let mut c = Converter::default();
        c.toggle(0);
        c.update(6.0, &mut r);
        assert!(c.fraction() > 0.5);
        c.toggle(1); // switching restarts the wait
        assert_eq!(c.selected(), Some(1));
        assert_eq!(c.fraction(), 0.0);
        c.toggle(1); // again turns it off
        assert_eq!(c.selected(), None);
        assert!(!c.update(60.0, &mut r));
        assert_eq!(r.dense, [0; 5]);
    }

    #[test]
    fn dense_ones_are_not_spent_on_blocks() {
        let mut r = Resources::default();
        r.dense = [5; 5];
        assert!(!r.can_afford(Block::Grass));
        assert!(!r.can_afford(Block::Log));
    }

    #[test]
    fn coal_gives_black_and_fire_gives_nothing_and_cannot_be_placed() {
        let mut r = Resources::default();
        r.gain(Block::Coal);
        assert_eq!(r.as_array(), [0, 0, 0, 0, 1]);
        r.gain(Block::Burning);
        assert_eq!(r.as_array(), [0, 0, 0, 0, 1]);
        assert!(!r.can_afford(Block::Burning), "free fire would be an exploit");
        assert!(r.spend(Block::Coal));
        assert_eq!(r.black, 0);
    }

    #[test]
    fn spending_dense_resources() {
        let mut r = Resources::default();
        assert!(!r.spend_dense(0, 1));
        r.dense[0] = 3;
        assert!(r.spend_dense(0, 1));
        assert!(!r.spend_dense(0, 3), "only 2 left");
        assert_eq!(r.dense[0], 2, "nothing taken on failure");
        assert!(r.spend_dense(0, 2));
        assert!(!r.spend_dense(0, 1));
        assert!(!r.spend_dense(9, 1), "no such kind");
    }

    #[test]
    fn water_gives_nothing_and_cannot_be_placed() {
        let mut r = Resources::default();
        r.gain(Block::Water);
        assert_eq!(r, Resources::default());
        assert!(!r.can_afford(Block::Water));
    }

    #[test]
    fn crafted_blocks_cannot_be_placed_and_give_back_their_dense_resource() {
        for (block, kind) in [
            (Block::StoneFloor, 0),
            (Block::HousePlant, 2),
            (Block::Forge, 4),
            (Block::Furnace, 4),
        ] {
            let mut r = Resources::default();
            assert!(!r.can_afford(block), "{block:?}");
            r.gain(block);
            let mut expected = [0u32; 5];
            expected[kind] = 1;
            assert_eq!(r.dense, expected, "{block:?}");
            assert_eq!(r.as_array(), [0; 5], "no raw resources, {block:?}");
        }
    }

    #[test]
    fn a_wooden_floor_cube_costs_one_dense_brown() {
        let mut r = Resources {
            grey: 50,
            brown: 50,
            dense: [9, 9, 9, 0, 9],
            ..Resources::default()
        };
        // Lots of raw brown and every other dense kind doesn't help.
        assert!(!r.can_afford(Block::WoodFloor));
        assert!(!r.spend(Block::WoodFloor));
        assert_eq!(r.brown, 50);

        r.dense[3] = 2;
        assert!(r.spend(Block::WoodFloor));
        assert_eq!((r.dense[3], r.brown), (1, 50), "one dense brown, no raw");
        assert!(r.spend(Block::WoodFloor));
        assert!(!r.can_afford(Block::WoodFloor));

        // Mining one gives that dense brown back, no more: placing and mining is a wash.
        r.gain(Block::WoodFloor);
        assert_eq!((r.dense[3], r.brown, r.grey), (1, 50, 50));
        assert!(r.spend(Block::WoodFloor));
        assert_eq!(r.dense[3], 0);
    }
}
