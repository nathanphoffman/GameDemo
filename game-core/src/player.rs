use glam::Vec3;

use crate::world::{World, EPS};

pub const HALF_WIDTH: f32 = 0.3;
pub const HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
const GRAVITY: f32 = 32.0;
const JUMP_SPEED: f32 = 9.0; // about 1.25 blocks high
const TERMINAL_SPEED: f32 = 50.0;
const WATER_GRAVITY: f32 = 10.0;
const SINK_SPEED: f32 = 3.0;
const SWIM_SPEED: f32 = 5.0;
const SWIM_SLOWDOWN: f32 = 0.6;
const SUBSTEP: f32 = 1.0 / 120.0;
const SNAP_GAP: f32 = 1e-3;

pub struct Player {
    /// Position of the feet (bottom center of the collision box).
    pub pos: Vec3,
    pub vel_y: f32,
    pub on_ground: bool,
    pub yaw: f32,
    pub pitch: f32,
}

impl Player {
    pub fn new(pos: Vec3) -> Self {
        Player {
            pos,
            vel_y: 0.0,
            on_ground: false,
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, EYE_HEIGHT, 0.0)
    }

    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
            -self.pitch.cos() * self.yaw.cos(),
        )
    }

    /// Horizontal forward/right, so looking up or down doesn't change walk speed.
    pub fn flat_axes(&self) -> (Vec3, Vec3) {
        (
            Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos()),
            Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin()),
        )
    }

    /// `velocity` is the desired horizontal movement in blocks per second.
    pub fn update(&mut self, dt: f32, velocity: Vec3, jump: bool, world: &World) {
        let feet_wet = world.in_water(self.pos + Vec3::new(0.0, 0.1, 0.0));
        let body_wet = world.in_water(self.pos + Vec3::new(0.0, 0.9, 0.0));
        let velocity = if body_wet { velocity * SWIM_SLOWDOWN } else { velocity };
        if jump && self.on_ground {
            self.vel_y = JUMP_SPEED;
        } else if jump && feet_wet {
            // Swim up, which is also enough of a kick to climb onto a bank.
            self.vel_y = self.vel_y.max(SWIM_SPEED);
        }
        let steps = (dt / SUBSTEP).ceil().max(1.0) as u32;
        let h = dt / steps as f32;
        for _ in 0..steps {
            // Water holds you up: you sink slowly instead of falling.
            let wet = world.in_water(self.pos + Vec3::new(0.0, 0.9, 0.0));
            self.vel_y = if wet {
                (self.vel_y - WATER_GRAVITY * h).max(-SINK_SPEED)
            } else {
                (self.vel_y - GRAVITY * h).max(-TERMINAL_SPEED)
            };
            self.on_ground = false;
            self.move_axis(world, 0, velocity.x * h);
            self.move_axis(world, 2, velocity.z * h);
            self.move_axis(world, 1, self.vel_y * h);
        }
    }

    fn blocked(&self, world: &World, p: Vec3) -> bool {
        let half = Vec3::new(HALF_WIDTH, 0.0, HALF_WIDTH);
        world.collides(p - half, p + half + Vec3::new(0.0, HEIGHT, 0.0))
    }

    fn move_axis(&mut self, world: &World, axis: usize, delta: f32) {
        if delta == 0.0 {
            return;
        }
        let mut p = self.pos;
        p[axis] += delta;
        if !self.blocked(world, p) {
            self.pos = p;
            return;
        }
        // Blocked: snap flush against the block we ran into.
        if axis == 1 {
            if delta < 0.0 {
                self.pos.y = p.y.floor() + 1.0;
                self.on_ground = true;
            } else {
                self.pos.y = (p.y + HEIGHT - EPS).floor() - HEIGHT - SNAP_GAP;
            }
            self.vel_y = 0.0;
        } else if delta > 0.0 {
            self.pos[axis] = (p[axis] + HALF_WIDTH - EPS).floor() - HALF_WIDTH - SNAP_GAP;
        } else {
            self.pos[axis] = (p[axis] - HALF_WIDTH).floor() + 1.0 + HALF_WIDTH + SNAP_GAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lands_jumps_and_never_clips() {
        for seed in [1, 2, 3, 99] {
            let world = World::generate(seed);
            let mut p = Player::new(world.spawn_point() + Vec3::new(0.0, 10.0, 0.0));
            for _ in 0..120 {
                p.update(1.0 / 60.0, Vec3::ZERO, false, &world);
            }
            assert!(p.on_ground, "should land");
            let ground = p.pos.y;
            assert!((ground - world.spawn_point().y).abs() < 0.1);

            p.update(1.0 / 60.0, Vec3::ZERO, true, &world);
            let mut peak = 0.0f32;
            for _ in 0..60 {
                p.update(1.0 / 60.0, Vec3::ZERO, false, &world);
                peak = peak.max(p.pos.y - ground);
            }
            assert!(peak > 1.1 && peak < 1.4, "jump peak {peak}");

            // Wander (jumping constantly) and make sure we never end up inside terrain.
            for i in 0..3000 {
                let a = i as f32 * 0.003;
                let v = Vec3::new(a.cos(), 0.0, a.sin()) * 8.0;
                p.update(1.0 / 60.0, v, true, &world);
                assert!(!p.blocked(&world, p.pos), "stuck in block at {:?}", p.pos);
                assert!(p.pos.y >= 0.0);
            }
        }
    }
}
