use super::*;

impl World {
    /// Does the box from `min` to `max` overlap any solid block?
    pub fn collides(&self, min: Vec3, max: Vec3) -> bool {
        let (x0, x1) = ((min.x + EPS).floor() as i32, (max.x - EPS).floor() as i32);
        let (y0, y1) = ((min.y + EPS).floor() as i32, (max.y - EPS).floor() as i32);
        let (z0, z1) = ((min.z + EPS).floor() as i32, (max.z - EPS).floor() as i32);
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    if self.solid(x, y, z) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// The cells of a staircase that climbs away from the block at (x, y, z): one step per
    /// cell, each one block further along (dx, dz) and one block higher.
    pub fn stair_cells(
        x: i32,
        y: i32,
        z: i32,
        (dx, dz): (i32, i32),
        steps: i32,
    ) -> impl Iterator<Item = (i32, i32, i32)> {
        (1..=steps).map(move |i| (x + dx * i, y + i, z + dz * i))
    }

    /// Whether every step of such a staircase is inside the world and empty.
    pub fn stairs_fit(&self, x: i32, y: i32, z: i32, dir: (i32, i32), steps: i32) -> bool {
        Self::stair_cells(x, y, z, dir, steps).all(|(sx, sy, sz)| {
            let inside = sx >= 0
                && sz >= 0
                && sy >= 0
                && sx < SIZE_X as i32
                && sz < SIZE_Z as i32
                && sy < SIZE_Y as i32;
            inside && self.get(sx, sy, sz) == Block::Air
        })
    }

    /// Whether there is a `block` within `radius` of `center` (measured to block centers).
    pub fn block_within(&self, center: Vec3, radius: f32, block: Block) -> bool {
        let r = radius.ceil() as i32;
        let (cx, cy, cz) = (
            center.x.floor() as i32,
            center.y.floor() as i32,
            center.z.floor() as i32,
        );
        for y in cy - r..=cy + r {
            for z in cz - r..=cz + r {
                for x in cx - r..=cx + r {
                    if self.get(x, y, z) != block {
                        continue;
                    }
                    let to = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) - center;
                    if to.length() <= radius {
                        return true;
                    }
                }
            }
        }
        false
    }
}
