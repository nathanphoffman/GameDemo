use super::builder::Builder;
use crate::combat::ROLL_ANIM_SECONDS;

/// After landing, the damage line shows up this much later than the sum.
const DAMAGE_LINE_DELAY: f32 = 0.4;

pub(super) struct DiceStyle {
    body: [f32; 3],
    pip: [f32; 3],
    border: [f32; 3],
    sum_color: [f32; 3],
    damage_color: [f32; 3],
}

pub(super) const PLAYER_DICE: DiceStyle = DiceStyle {
    body: [0.96, 0.95, 0.90],
    pip: [0.10, 0.10, 0.12],
    border: [0.10, 0.10, 0.12],
    sum_color: [1.0, 0.9, 0.35],
    damage_color: [1.0, 0.45, 0.35],
};

pub(super) const ENEMY_DICE: DiceStyle = DiceStyle {
    body: [0.80, 0.12, 0.10],
    pip: [1.0, 1.0, 1.0],
    border: [0.20, 0.02, 0.02],
    sum_color: [1.0, 0.9, 0.35],
    damage_color: [1.0, 0.45, 0.35],
};

impl Builder {
    /// A die face of width `size` showing `value` pips, turned by `angle`.
    fn die(&mut self, cx: f32, cy: f32, size: f32, angle: f32, value: u32, style: &DiceStyle) {
        self.turned_square(cx, cy, size / 2.0, angle, style.border);
        self.turned_square(cx, cy, size / 2.0 - size * 0.05, angle, style.body);
        let pips: &[(f32, f32)] = match value {
            1 => &[(0.0, 0.0)],
            2 => &[(-1.0, -1.0), (1.0, 1.0)],
            3 => &[(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)],
            4 => &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)],
            5 => &[(-1.0, -1.0), (1.0, -1.0), (0.0, 0.0), (-1.0, 1.0), (1.0, 1.0)],
            _ => &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0)],
        };
        let (sin, cos) = angle.sin_cos();
        for &(px, py) in pips {
            let (x, y) = (px * size * 0.26, py * size * 0.26);
            self.turned_square(
                cx + x * cos - y * sin,
                cy + x * sin + y * cos,
                size * 0.075,
                angle,
                style.pip,
            );
        }
    }
}

/// Two dice that slide in, bounce and tumble to a stop centered on `cx`, resting at height
/// `rest_y`. Once landed, the sum and the damage appear above or below them.
pub(super) fn draw_roll(
    b: &mut Builder,
    cx: f32,
    rest_y: f32,
    scale: f32,
    (first, second, bonus, age): (u32, u32, i32, f32),
    style: &DiceStyle,
    text_below: bool,
) {
    let die_size = 16.0 * scale;
    let u = (age / ROLL_ANIM_SECONDS).clamp(0.0, 1.0);
    // Fast at first, easing to a stop.
    let eased = 1.0 - (1.0 - u).powi(3);
    let tau = std::f32::consts::TAU;

    for (i, result) in [first, second].into_iter().enumerate() {
        let side = if i == 0 { -1.0 } else { 1.0 };
        // Slides in from the outside, bounces, and tumbles to a flat stop.
        let x = cx + side * (die_size * 0.8 + (1.0 - eased) * die_size * 3.0);
        let bounce = (u * 3.0 * std::f32::consts::PI).sin().abs() * (1.0 - u) * die_size * 1.2;
        let angle = (1.0 - eased) * (3.0 + i as f32) * tau * side;
        let value = if u >= 1.0 {
            result
        } else {
            // Cycle through faces, slowing down as it settles.
            ((eased * 20.0) as u32 + 3 * i as u32) * 5 % 6 + 1
        };
        b.die(x, rest_y - bounce, die_size, angle, value, style);
    }

    if age < ROLL_ANIM_SECONDS {
        return;
    }
    let sum = (first + second) as i32 + bonus;
    let text_scale = scale * 1.5;
    let line_height = 8.0 * text_scale;
    let (sum_y, damage_y) = if text_below {
        let y = rest_y + die_size * 0.5 + 3.0 * scale;
        (y, y + line_height)
    } else {
        let y = rest_y - die_size * 0.5 - 3.0 * scale - line_height;
        (y, y - line_height)
    };
    b.centered_text(cx, sum_y, text_scale, &if bonus > 0 {
            format!("{first} + {second} + {bonus} = {sum}")
        } else {
            format!("{first} + {second} = {sum}")
        }, style.sum_color);
    if age >= ROLL_ANIM_SECONDS + DAMAGE_LINE_DELAY {
        b.centered_text(cx, damage_y, text_scale, &format!("DAMAGE {sum}"), style.damage_color);
    }
}
