//! The game's rules and state: the world, creatures, combat, weather. Nothing in here draws
//! anything or opens a window, so a host program can use it without a screen.

pub mod biome;
pub mod combat;
pub mod effects;
pub mod enemy;
pub mod game;
pub mod player;
pub mod resources;
pub mod weather;
pub mod world;
