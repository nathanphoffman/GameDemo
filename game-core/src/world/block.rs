use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    Air,
    Grass,
    Dirt,
    Snow,
    Stone,
    Log,
    PineLog,
    Leaves,
    PineLeaves,
    /// On fire; turns into `Coal` after a while.
    Burning,
    Coal,
    /// Made by dousing a fire with ice.
    Water,
    /// Man-made. Wooden floor is what the other crafted things are built on.
    WoodFloor,
    StoneFloor,
    /// Speeds up converting resources while the player is near.
    Forge,
    /// A stronger forge, built on a stone floor.
    Furnace,
    HousePlant,
    /// Desert ground.
    Sand,
    /// Badlands rock comes in two alternating layers.
    RedRock,
    Clay,
    /// Volcanic ground.
    Basalt,
    /// A glowing pool surface in volcanic country.
    Magma,
    Cactus,
}

#[cfg(test)]
impl Block {
    /// Every block type, for tests that check them all.
    pub const ALL: [Block; 23] = [
        Block::Air,
        Block::Grass,
        Block::Dirt,
        Block::Snow,
        Block::Stone,
        Block::Log,
        Block::PineLog,
        Block::Leaves,
        Block::PineLeaves,
        Block::Burning,
        Block::Coal,
        Block::Water,
        Block::WoodFloor,
        Block::StoneFloor,
        Block::Forge,
        Block::Furnace,
        Block::HousePlant,
        Block::Sand,
        Block::RedRock,
        Block::Clay,
        Block::Basalt,
        Block::Magma,
        Block::Cactus,
    ];
}
