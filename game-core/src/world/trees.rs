use super::*;

impl World {
    /// A bare trunk, 3 to 5 tall, for swamps.
    pub(super) fn plant_dead_tree(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        for dy in 0..3 + (size_roll * 3.0) as i32 {
            self.set_if_air(x, ground + dy, z, Block::Log);
        }
    }

    /// Trunk height for a tree, from a roll in 0..1: oaks are 4 or 5, pines 6 to 8.
    pub(super) fn trunk_height(pine: bool, size_roll: f32) -> i32 {
        if pine {
            6 + (size_roll * 3.0) as i32
        } else {
            4 + (size_roll * 2.0) as i32
        }
    }

    /// Whether a tree could grow on the block at (x, y, z): its trunk column has to be open
    /// air, and it has to be clear of the world's edges.
    pub fn tree_fits(&self, x: i32, y: i32, z: i32, pine: bool) -> bool {
        let trunk = Self::trunk_height(pine, Self::tree_roll(x, z));
        let inside = x >= 3 && z >= 3 && x + 3 < SIZE_X as i32 && z + 3 < SIZE_Z as i32;
        inside
            && y + 1 + trunk + 2 < SIZE_Y as i32
            && (1..=trunk).all(|dy| self.get(x, y + dy, z) == Block::Air)
    }

    /// Grows a tree on top of the block at (x, y, z), if there is room. The same spot always
    /// gives the same size of tree.
    pub fn grow_tree(&mut self, x: i32, y: i32, z: i32, pine: bool) -> bool {
        if !self.tree_fits(x, y, z, pine) {
            return false;
        }
        let roll = Self::tree_roll(x, z);
        if pine {
            self.plant_pine(x, y + 1, z, roll);
        } else {
            self.plant_oak(x, y + 1, z, roll);
        }
        true
    }

    pub(super) fn tree_roll(x: i32, z: i32) -> f32 {
        hash(x as u32, z as u32, 0x7233)
    }

    pub(super) fn plant_oak(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        let trunk = Self::trunk_height(false, size_roll);
        for dy in 0..trunk {
            self.set_if_air(x, ground + dy, z, Block::Log);
        }
        // Rounded canopy: two wide layers, then a smaller cap.
        for (dy, radius) in [(trunk - 2, 2i32), (trunk - 1, 2), (trunk, 1), (trunk + 1, 1)] {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    let corner = dx.abs() == radius && dz.abs() == radius;
                    if corner && (radius == 2 || dy == trunk + 1) {
                        continue;
                    }
                    self.set_if_air(x + dx, ground + dy, z + dz, Block::Leaves);
                }
            }
        }
    }

    pub(super) fn plant_pine(&mut self, x: i32, ground: i32, z: i32, size_roll: f32) {
        let trunk = Self::trunk_height(true, size_roll);
        for dy in 0..trunk {
            self.set_if_air(x, ground + dy, z, Block::PineLog);
        }
        // Tiered cone: widest near the bottom, with a single needle at the very top.
        let top = ground + trunk;
        for y in (ground + 2)..=top {
            let d = top - y;
            let radius: i32 = match d {
                0 => 0,
                1 | 2 => 1,
                n if n % 2 == 1 => 2,
                _ => 1,
            };
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if radius == 2 && dx.abs() == 2 && dz.abs() == 2 {
                        continue;
                    }
                    self.set_if_air(x + dx, y, z + dz, Block::PineLeaves);
                }
            }
        }
    }
}
