//! Rain and wind streaks, as flat quads floating around the camera.

use glam::Vec3;

use super::Vertex;
use crate::world::hash;

// ---- rain and wind streaks ----
//
// These are real little lines floating in the world around the player, not a picture glued to
// the screen. Each one has a fixed place in a box that follows the player (it wraps around
// when the player walks away from it), so turning your head or walking moves them across the
// view the way real rain or wind would.

/// The most rain streaks drawn at once (at full rain), and the most wind streaks.
pub const MAX_RAIN: usize = 1400;
pub const MAX_WIND: usize = 90;
/// Most vertices `streak_mesh` can return.
pub const MAX_STREAK_VERTICES: usize = (MAX_RAIN + MAX_WIND) * 4;

/// Size of the box around the player the rain and the wind streaks live in, in blocks.
const RAIN_BOX: Vec3 = Vec3::new(30.0, 20.0, 30.0);
const WIND_BOX: Vec3 = Vec3::new(44.0, 12.0, 44.0);
/// Blocks per second.
const RAIN_FALL_SPEED: f32 = 15.0;
const WIND_SPEED: f32 = 9.0;

// Texture layers (see textures.rs).
const RAIN_LAYER: f32 = 32.0;
const WIND_LAYER: f32 = 33.0;
const RAIN_WIDTH: f32 = 0.012;
const SNOW_WIDTH: f32 = 0.035;
/// Snowflakes drift down slowly, as short fat streaks.
const SNOW_FALL_SPEED: f32 = 2.5;
const WIND_WIDTH: f32 = 0.02;

/// Where the wind blows, as a direction along the ground. It wanders a little over time.
fn wind_direction(time: f32) -> Vec3 {
    let angle = 0.9 + 0.25 * (time * 0.05).sin();
    Vec3::new(angle.cos(), 0.0, angle.sin())
}

/// Wraps `p` into a box of size `size` centered on `center`.
fn wrap_into_box(p: Vec3, center: Vec3, size: Vec3) -> Vec3 {
    let rel = p - center;
    let shift = (rel / size + Vec3::splat(0.5)).floor() * size;
    center + rel - shift
}

/// A streak's (head, tail) position.
type Streak = (Vec3, Vec3);

/// All the streaks' (head, tail) positions: rain, then wind.
pub fn streaks(
    cam: Vec3,
    time: f32,
    rain: f32,
    wind: f32,
    snowing: bool,
) -> (Vec<Streak>, Vec<Streak>) {
    let wind_dir = wind_direction(time);

    let rain_count = (MAX_RAIN as f32 * rain.clamp(0.0, 1.0)) as usize;
    let mut rain_streaks = Vec::with_capacity(rain_count);
    for i in 0..rain_count {
        let h = |k: u32| hash(i as u32, k, 0x4A1E);
        let base = Vec3::new(h(0), h(1), h(2)) * RAIN_BOX;
        let fall = if snowing { SNOW_FALL_SPEED } else { RAIN_FALL_SPEED } * (0.8 + 0.4 * h(3));
        // The wind leans the rain over, and each drop leans a little differently.
        let lean = wind * 3.0 * (0.8 + 0.4 * h(4));
        let velocity = Vec3::new(wind_dir.x * lean, -fall, wind_dir.z * lean);
        let position = wrap_into_box(base + velocity * time, cam, RAIN_BOX);
        let length = if snowing { 0.1 + 0.1 * h(5) } else { 0.5 + 0.6 * h(5) };
        let tail = position - velocity.normalize() * length;
        rain_streaks.push((position, tail));
    }

    // Wind streaks only show up when it is properly windy.
    let gale = ((wind - 0.55) / 0.45).clamp(0.0, 1.0);
    let wind_count = (MAX_WIND as f32 * gale) as usize;
    let mut wind_streaks = Vec::with_capacity(wind_count);
    for i in 0..wind_count {
        let h = |k: u32| hash(i as u32, k, 0x71D0);
        // Each streak has its own heading, a few degrees off the wind's, and its own drift
        // up or down, so they are never perfectly parallel.
        let turn = (h(0) - 0.5) * 0.7;
        let (sin, cos) = turn.sin_cos();
        let heading = Vec3::new(
            wind_dir.x * cos - wind_dir.z * sin,
            (h(1) - 0.5) * 0.35,
            wind_dir.x * sin + wind_dir.z * cos,
        )
        .normalize();
        let speed = WIND_SPEED * (0.7 + 0.6 * h(2));
        let base = Vec3::new(h(3), h(4), h(5)) * WIND_BOX;
        // Streaks float around head height, spread through the box.
        let position = wrap_into_box(base + heading * speed * time, cam + Vec3::Y * 2.0, WIND_BOX);
        let length = 2.0 + 2.5 * h(6);
        wind_streaks.push((position, position - heading * length));
    }
    (rain_streaks, wind_streaks)
}

/// Flat quads for all the streaks, each turned to face the camera. Four vertices per streak.
pub fn streak_mesh(cam: Vec3, time: f32, rain: f32, wind: f32, snowing: bool) -> Vec<Vertex> {
    let (rain_streaks, wind_streaks) = streaks(cam, time, rain, wind, snowing);
    let mut out = Vec::with_capacity((rain_streaks.len() + wind_streaks.len()) * 4);
    let mut push = |(a, b): (Vec3, Vec3), half_width: f32, layer: f32| {
        let along = b - a;
        let to_camera = cam - (a + b) * 0.5;
        let side = along.cross(to_camera).try_normalize().unwrap_or(Vec3::X) * half_width;
        for corner in [a - side, a + side, b + side, b - side] {
            out.push(Vertex {
                pos: corner.to_array(),
                uv: [0.0, 0.0],
                layer,
                shade: 1.0,
            });
        }
    };
    for streak in rain_streaks {
        if snowing {
            push(streak, SNOW_WIDTH, WIND_LAYER); // plain white
        } else {
            push(streak, RAIN_WIDTH, RAIN_LAYER);
        }
    }
    for streak in wind_streaks {
        push(streak, WIND_WIDTH, WIND_LAYER);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streak_counts_follow_rain_and_wind() {
        let cam = Vec3::new(100.0, 20.0, 100.0);
        let count = |rain: f32, wind: f32| {
            let (r, w) = streaks(cam, 3.0, rain, wind, false);
            (r.len(), w.len())
        };
        assert_eq!(count(0.0, 0.3), (0, 0), "sunny: nothing");
        let (light, _) = count(0.45, 0.4);
        let (heavy, _) = count(1.0, 0.8);
        assert!(0 < light && light < heavy);
        assert_eq!(heavy, MAX_RAIN);
        let (_, calm_wind) = count(0.0, 0.5);
        let (_, gale) = count(0.0, 1.0);
        assert_eq!(calm_wind, 0, "a breeze has no streaks");
        assert_eq!(gale, MAX_WIND);
        assert_eq!(streak_mesh(cam, 3.0, 1.0, 1.0, false).len(), (MAX_RAIN + MAX_WIND) * 4);
        assert!(streak_mesh(cam, 3.0, 1.0, 1.0, false).len() <= MAX_STREAK_VERTICES);
    }

    #[test]
    fn streaks_stay_where_they_are_in_the_world_when_the_player_moves() {
        let cam = Vec3::new(100.0, 20.0, 100.0);
        let (before, before_wind) = streaks(cam, 5.0, 1.0, 1.0, false);
        let (after, after_wind) = streaks(cam + Vec3::new(1.5, 0.0, -1.0), 5.0, 1.0, 1.0, false);
        // Almost every streak is at exactly the same place in the world; only the few that
        // had to wrap around to the other side of the box have jumped.
        let same = |a: &[(Vec3, Vec3)], b: &[(Vec3, Vec3)]| {
            a.iter()
                .zip(b)
                .filter(|(p, q)| (p.0 - q.0).length() < 1e-3)
                .count() as f32
                / a.len() as f32
        };
        assert!(same(&before, &after) > 0.85, "rain {}", same(&before, &after));
        assert!(
            same(&before_wind, &after_wind) > 0.85,
            "wind {}",
            same(&before_wind, &after_wind)
        );
    }

    #[test]
    fn rain_falls_and_stays_near_the_player() {
        let cam = Vec3::new(50.0, 30.0, 50.0);
        let (now, _) = streaks(cam, 2.0, 1.0, 0.0, false);
        let (later, _) = streaks(cam, 2.05, 1.0, 0.0, false);
        let dropped = now
            .iter()
            .zip(&later)
            .filter(|(a, b)| b.0.y < a.0.y)
            .count();
        assert!(dropped as f32 / now.len() as f32 > 0.9, "falling, bar the wrapped ones");
        for (head, _) in &now {
            let rel = *head - cam;
            assert!(rel.x.abs() <= RAIN_BOX.x / 2.0 + 1e-3);
            assert!(rel.y.abs() <= RAIN_BOX.y / 2.0 + 1e-3);
            assert!(rel.z.abs() <= RAIN_BOX.z / 2.0 + 1e-3);
        }
    }

    #[test]
    fn wind_streaks_are_not_parallel() {
        let (_, wind) = streaks(Vec3::new(0.0, 20.0, 0.0), 1.0, 0.0, 1.0, false);
        let directions: Vec<Vec3> = wind
            .iter()
            .map(|(head, tail)| (*head - *tail).normalize())
            .collect();
        let mut widest: f32 = 0.0;
        for d in &directions {
            widest = widest.max(directions[0].dot(*d).clamp(-1.0, 1.0).acos());
        }
        assert!(widest > 0.2, "widest spread {widest} radians");
        // ...but they all roughly blow the same way.
        let mean = directions.iter().fold(Vec3::ZERO, |a, b| a + *b).normalize();
        assert!(directions.iter().all(|d| d.dot(mean) > 0.5));
    }
}
