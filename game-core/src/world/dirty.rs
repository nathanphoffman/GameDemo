use super::*;

impl World {
    /// Notes that the mesh of the chunk holding column (x, z) is out of date, and of the
    /// next chunk over if the column is on the edge (faces there depend on it).
    pub(super) fn mark_dirty(&mut self, x: i32, z: i32) {
        if x < 0 || z < 0 || x >= SIZE_X as i32 || z >= SIZE_Z as i32 {
            return;
        }
        let (x, z) = (x as usize, z as usize);
        let (cx, cz) = (x / CHUNK, z / CHUNK);
        self.dirty[cz * CHUNKS_X + cx] = true;
        if x % CHUNK == 0 && cx > 0 {
            self.dirty[cz * CHUNKS_X + cx - 1] = true;
        }
        if x % CHUNK == CHUNK - 1 && cx + 1 < CHUNKS_X {
            self.dirty[cz * CHUNKS_X + cx + 1] = true;
        }
        if z % CHUNK == 0 && cz > 0 {
            self.dirty[(cz - 1) * CHUNKS_X + cx] = true;
        }
        if z % CHUNK == CHUNK - 1 && cz + 1 < CHUNKS_Z {
            self.dirty[(cz + 1) * CHUNKS_X + cx] = true;
        }
    }

    /// The chunks (x, z) whose meshes need rebuilding since the last call.
    pub fn take_dirty(&mut self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, flag) in self.dirty.iter_mut().enumerate() {
            if *flag {
                *flag = false;
                out.push((i % CHUNKS_X, i / CHUNKS_X));
            }
        }
        out
    }
}
