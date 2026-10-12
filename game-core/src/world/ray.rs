use super::*;

impl World {
    /// Walks a ray through the grid and returns the first solid block within `max_dist`.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<(i32, i32, i32)> {
        self.raycast_face(origin, dir, max_dist).map(|hit| hit.block)
    }

    /// Like `raycast`, but also reports the empty cell the ray was in just before it hit,
    /// which is where a new block would go against the face that was hit.
    pub fn raycast_face(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let mut cell = [
            origin.x.floor() as i32,
            origin.y.floor() as i32,
            origin.z.floor() as i32,
        ];
        let o = origin.to_array();
        let d = dir.to_array();
        let mut step = [0i32; 3];
        let mut t_max = [f32::INFINITY; 3];
        let mut t_delta = [f32::INFINITY; 3];
        for i in 0..3 {
            if d[i] > 0.0 {
                step[i] = 1;
                t_delta[i] = 1.0 / d[i];
                t_max[i] = ((cell[i] + 1) as f32 - o[i]) / d[i];
            } else if d[i] < 0.0 {
                step[i] = -1;
                t_delta[i] = -1.0 / d[i];
                t_max[i] = (o[i] - cell[i] as f32) / -d[i];
            }
        }
        let mut previous: Option<(i32, i32, i32)> = None;
        loop {
            let (x, y, z) = (cell[0], cell[1], cell[2]);
            if self.get(x, y, z) != Block::Air && !self.is_liquid(x, y, z) {
                return Some(RayHit {
                    block: (cell[0], cell[1], cell[2]),
                    place: previous,
                });
            }
            previous = Some((cell[0], cell[1], cell[2]));
            // Step into whichever neighbouring cell the ray reaches first.
            let axis = if t_max[0] < t_max[1] && t_max[0] < t_max[2] {
                0
            } else if t_max[1] < t_max[2] {
                1
            } else {
                2
            };
            if t_max[axis] > max_dist {
                return None;
            }
            cell[axis] += step[axis];
            t_max[axis] += t_delta[axis];
        }
    }
}
