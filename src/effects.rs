use crate::world::Block;

/// How long a block burns before it turns into coal.
pub const BURN_SECONDS: f32 = 10.0;

// Indices of the dense resources: grey, white, green, brown, black.
pub const FLINT: usize = 0; // dense grey (stone)
pub const ICE: usize = 1; // dense white
pub const SEED: usize = 2; // dense green
pub const WOOD: usize = 3; // dense brown
pub const CARBON: usize = 4; // dense black

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Effect {
    /// Sets the block on fire; it becomes coal when the fire dies down.
    Ignite,
    /// Puts a fire out, leaving water.
    Douse,
    /// Grass turns to snow.
    Freeze,
    /// Dirt turns to grass.
    Seed,
    /// The landscape's native tree grows on the block: an oak on grass.
    GrowOak,
    /// ...and a pine on snow.
    GrowPine,
    /// The block becomes a wooden floor.
    LayFloor,
    /// A wooden floor becomes a stone floor.
    StoneFloor,
    /// A forge is built on top of a wooden floor.
    BuildForge,
    /// A house plant is put on top of a wooden floor.
    PlantHouse,
    /// A furnace is built on top of a stone floor.
    BuildFurnace,
    /// A staircase of wooden steps climbs away from a wooden floor.
    BuildStairs,
}

impl Effect {
    /// Short name shown in the apply menu.
    pub fn label(self) -> &'static str {
        match self {
            Effect::Ignite => "FIRE",
            Effect::Douse => "WATER",
            Effect::Freeze => "SNOW",
            Effect::Seed => "GRASS",
            Effect::GrowOak | Effect::GrowPine => "TREE",
            Effect::LayFloor => "FLOOR",
            Effect::StoneFloor => "STONE",
            Effect::BuildForge => "FORGE",
            Effect::PlantHouse => "PLANT",
            Effect::BuildFurnace => "FURNACE",
            Effect::BuildStairs => "STAIRS",
        }
    }

    /// Whether this puts something new in the cell above the block (so that cell must be
    /// free), rather than changing the block itself.
    pub fn builds_on_top(self) -> bool {
        matches!(
            self,
            Effect::BuildForge | Effect::PlantHouse | Effect::BuildFurnace
        )
    }
}

/// Steps the right-click selection through "clone" (`None`), then each dense resource
/// (0 = grey ... 4 = black), and back around.
pub fn cycle_selection(current: Option<usize>, forward: bool) -> Option<usize> {
    const KINDS: usize = 5;
    // Position in the ring: 0 is clone, 1..=5 are the dense resources.
    let at = current.map_or(0, |kind| kind + 1);
    let next = if forward {
        (at + 1) % (KINDS + 1)
    } else {
        (at + KINDS) % (KINDS + 1)
    };
    next.checked_sub(1)
}

/// Blocks that are green or brown: grass, leaves and wood.
pub fn burnable(block: Block) -> bool {
    matches!(
        block,
        Block::Grass | Block::Leaves | Block::PineLeaves | Block::Log | Block::PineLog
    )
}

/// The natural ground, which a wooden floor can be laid over.
pub fn is_ground(block: Block) -> bool {
    matches!(block, Block::Grass | Block::Dirt | Block::Snow | Block::Stone)
}

/// What applying a dense resource (`dense_kind`: 0 = grey ... 4 = black) to `block` does, going
/// by the type of block alone. Trees and the things built on a floor also need room, which
/// the caller has to check.
pub fn effect_for(dense_kind: usize, block: Block) -> Option<Effect> {
    match (dense_kind, block) {
        (FLINT, Block::WoodFloor) => Some(Effect::StoneFloor),
        (FLINT, b) if burnable(b) => Some(Effect::Ignite),
        (ICE, Block::Burning) => Some(Effect::Douse),
        (ICE, Block::Grass) => Some(Effect::Freeze),
        (SEED, Block::Dirt) => Some(Effect::Seed),
        (SEED, Block::Grass) => Some(Effect::GrowOak),
        (SEED, Block::Snow) => Some(Effect::GrowPine),
        (SEED, Block::WoodFloor) => Some(Effect::PlantHouse),
        (WOOD, b) if is_ground(b) => Some(Effect::LayFloor),
        (WOOD, Block::WoodFloor) => Some(Effect::BuildStairs),
        (CARBON, Block::StoneFloor) => Some(Effect::BuildFurnace),
        (CARBON, Block::WoodFloor) => Some(Effect::BuildForge),
        _ => None,
    }
}

/// Blocks that are currently on fire, and how long each has left.
#[derive(Default)]
pub struct Fires {
    burning: Vec<((i32, i32, i32), f32)>,
}

impl Fires {
    pub fn ignite(&mut self, pos: (i32, i32, i32)) {
        if !self.burning.iter().any(|(p, _)| *p == pos) {
            self.burning.push((pos, BURN_SECONDS));
        }
    }

    /// Advances the fires and returns the positions that have burnt out.
    pub fn update(&mut self, dt: f32) -> Vec<(i32, i32, i32)> {
        for (_, left) in &mut self.burning {
            *left -= dt;
        }
        let done = self
            .burning
            .iter()
            .filter(|(_, left)| *left <= 0.0)
            .map(|(pos, _)| *pos)
            .collect();
        self.burning.retain(|(_, left)| *left > 0.0);
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::Resources;

    #[test]
    fn flint_ignites_exactly_the_green_and_brown_blocks() {
        for block in Block::ALL {
            let value = Resources::value_of(block);
            let green_or_brown = value.green > 0 || value.brown > 0;
            assert_eq!(
                effect_for(FLINT, block) == Some(Effect::Ignite),
                green_or_brown,
                "{block:?}"
            );
        }
    }

    #[test]
    fn every_combination_is_what_the_table_says() {
        use Block::*;
        // (dense kind, block, what it does). Anything not listed must do nothing.
        let mut table = vec![
            (FLINT, WoodFloor, Effect::StoneFloor),
            (ICE, Burning, Effect::Douse),
            (ICE, Grass, Effect::Freeze),
            (SEED, Dirt, Effect::Seed),
            (SEED, Grass, Effect::GrowOak),
            (SEED, Snow, Effect::GrowPine),
            (SEED, WoodFloor, Effect::PlantHouse),
            (CARBON, WoodFloor, Effect::BuildForge),
            (CARBON, StoneFloor, Effect::BuildFurnace),
            (WOOD, WoodFloor, Effect::BuildStairs),
        ];
        // Wood lays a floor over any natural ground.
        for ground in [Grass, Dirt, Snow, Stone] {
            table.push((WOOD, ground, Effect::LayFloor));
        }
        for kind in 0..5 {
            for block in Block::ALL {
                let listed = table.iter().find(|(k, b, _)| *k == kind && *b == block);
                let got = effect_for(kind, block);
                match listed {
                    Some((_, _, effect)) => assert_eq!(got, Some(*effect), "{kind} on {block:?}"),
                    // Flint's burning rule is checked in the test above.
                    None if kind == FLINT && burnable(block) => {}
                    None => assert_eq!(got, None, "{kind} on {block:?}"),
                }
            }
        }
    }

    #[test]
    fn crafted_things_stand_on_a_wooden_floor_and_the_floor_is_not_ground() {
        assert!(!is_ground(Block::WoodFloor));
        assert!(!is_ground(Block::StoneFloor));
        for ground in [Block::Grass, Block::Dirt, Block::Snow, Block::Stone] {
            assert_eq!(effect_for(CARBON, ground), None, "forges need a floor");
            assert_ne!(effect_for(WOOD, ground), Some(Effect::BuildStairs));
        }
        // A forge goes on wood, a furnace on stone: the next level up.
        assert_eq!(effect_for(CARBON, Block::WoodFloor), Some(Effect::BuildForge));
        assert_eq!(effect_for(CARBON, Block::StoneFloor), Some(Effect::BuildFurnace));
    }

    #[test]
    fn the_scroll_selection_goes_round_through_clone_and_the_five_dense_kinds() {
        let mut selection = None; // clone
        let mut seen = vec![selection];
        for _ in 0..6 {
            selection = cycle_selection(selection, true);
            seen.push(selection);
        }
        assert_eq!(
            seen,
            vec![None, Some(0), Some(1), Some(2), Some(3), Some(4), None]
        );
        // Backwards from clone wraps to the last one.
        assert_eq!(cycle_selection(None, false), Some(4));
        assert_eq!(cycle_selection(Some(0), false), None);
        assert_eq!(cycle_selection(Some(3), false), Some(2));
    }

    #[test]
    fn fire_lasts_ten_seconds() {
        let mut fires = Fires::default();
        fires.ignite((1, 2, 3));
        fires.ignite((1, 2, 3)); // lighting it twice doesn't restart or duplicate
        assert!(fires.update(BURN_SECONDS - 0.1).is_empty());
        assert_eq!(fires.update(0.2), vec![(1, 2, 3)]);
        assert!(fires.update(100.0).is_empty(), "reported once");
    }
}
