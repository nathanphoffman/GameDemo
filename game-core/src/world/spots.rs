use super::*;

impl World {
    /// The closest spot to (`want_x`, `want_z`) with room for a 3x3x3 creature standing on the
    /// ground, searching outward in rings. Returns the bottom center.
    pub fn open_spot(&self, want_x: i32, want_z: i32) -> Vec3 {
        for radius in 0..40i32 {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    // Only the outline of this ring; the inside was checked already.
                    if dx.abs().max(dz.abs()) != radius {
                        continue;
                    }
                    if let Some(spot) = self.standing_spot(want_x + dx, want_z + dz) {
                        return spot;
                    }
                }
            }
        }
        self.spawn_point()
    }

    /// Standing on the ground in column (x, z), if there is room there for a 3x3x3 creature.
    /// Returns the bottom center.
    pub fn standing_spot(&self, x: i32, z: i32) -> Option<Vec3> {
        if x < 2 || z < 2 || x + 2 >= SIZE_X as i32 || z + 2 >= SIZE_Z as i32 {
            return None;
        }
        let ground = self.heights[z as usize * SIZE_X + x as usize] as i32;
        let clear = (0..3).all(|dy| {
            (-1..=1).all(|ox| {
                (-1..=1).all(|oz| self.get(x + ox, ground + dy, z + oz) == Block::Air)
            })
        });
        clear.then(|| Vec3::new(x as f32 + 0.5, ground as f32, z as f32 + 0.5))
    }

    /// Feet position standing on the surface in the middle of the world.
    pub fn spawn_point(&self) -> Vec3 {
        let (x, z) = (SIZE_X / 2, SIZE_Z / 2);
        let h = self.heights[z * SIZE_X + x] as f32;
        Vec3::new(x as f32 + 0.5, h + 0.01, z as f32 + 0.5)
    }
}
