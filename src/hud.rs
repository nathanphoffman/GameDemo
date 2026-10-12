use bytemuck::{Pod, Zeroable};

/// Room for the crosshair, the rock counter and the dice.
pub const MAX_HUD_VERTICES: usize = 16384;

/// Row colors for grey (stone), white (snow), green (grass and leaves), brown (wood).
const RESOURCE_COLORS: [[f32; 3]; 4] = [
    [0.55, 0.56, 0.60],
    [0.96, 0.97, 1.00],
    [0.30, 0.68, 0.22],
    [0.50, 0.32, 0.17],
];

use crate::combat::ROLL_ANIM_SECONDS;

/// After landing, the damage line shows up this much later than the sum.
const DAMAGE_LINE_DELAY: f32 = 0.4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub color: [f32; 3],
}

/// 3x5 pixel font: each row is 3 bits, most significant bit on the left.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        _ => [0; 5],
    }
}

struct Builder {
    width: f32,
    height: f32,
    verts: Vec<HudVertex>,
}

impl Builder {
    /// Rectangle in pixels, measured from the top-left of the screen.
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 3]) {
        let to_ndc = |px: f32, py: f32| [px / self.width * 2.0 - 1.0, 1.0 - py / self.height * 2.0];
        let (a, b, c, d) = (
            to_ndc(x, y),
            to_ndc(x + w, y),
            to_ndc(x + w, y + h),
            to_ndc(x, y + h),
        );
        for pos in [a, b, c, a, c, d] {
            self.verts.push(HudVertex { pos, color });
        }
    }

    /// Filled square centered on (cx, cy), turned by `angle` radians.
    fn turned_square(&mut self, cx: f32, cy: f32, half: f32, angle: f32, color: [f32; 3]) {
        let (sin, cos) = angle.sin_cos();
        let corner = |x: f32, y: f32| {
            [
                (cx + x * cos - y * sin) / self.width * 2.0 - 1.0,
                1.0 - (cy + x * sin + y * cos) / self.height * 2.0,
            ]
        };
        let (a, b, c, d) = (
            corner(-half, -half),
            corner(half, -half),
            corner(half, half),
            corner(-half, half),
        );
        for pos in [a, b, c, a, c, d] {
            self.verts.push(HudVertex { pos, color });
        }
    }

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

    /// Text centered horizontally on `cx`.
    fn centered_text(&mut self, cx: f32, y: f32, scale: f32, text: &str, color: [f32; 3]) {
        let width = text.chars().count() as f32 * 4.0 * scale - scale;
        self.text(cx - width / 2.0, y, scale, text, color);
    }

    fn text(&mut self, x: f32, y: f32, scale: f32, text: &str, color: [f32; 3]) {
        // Dark copy underneath so it stays readable on snow and sky.
        for (offset, col) in [(scale, [0.0, 0.0, 0.0]), (0.0, color)] {
            let mut cx = x;
            for ch in text.chars() {
                for (row, bits) in glyph(ch).iter().enumerate() {
                    for column in 0..3 {
                        if bits & (0b100 >> column) != 0 {
                            self.rect(
                                cx + column as f32 * scale + offset,
                                y + row as f32 * scale + offset,
                                scale,
                                scale,
                                col,
                            );
                        }
                    }
                }
                cx += 4.0 * scale;
            }
        }
    }
}

struct DiceStyle {
    body: [f32; 3],
    pip: [f32; 3],
    border: [f32; 3],
    sum_color: [f32; 3],
    damage_color: [f32; 3],
}

const PLAYER_DICE: DiceStyle = DiceStyle {
    body: [0.96, 0.95, 0.90],
    pip: [0.10, 0.10, 0.12],
    border: [0.10, 0.10, 0.12],
    sum_color: [1.0, 0.9, 0.35],
    damage_color: [1.0, 0.45, 0.35],
};

const ENEMY_DICE: DiceStyle = DiceStyle {
    body: [0.80, 0.12, 0.10],
    pip: [1.0, 1.0, 1.0],
    border: [0.20, 0.02, 0.02],
    sum_color: [1.0, 0.9, 0.35],
    damage_color: [1.0, 0.45, 0.35],
};

/// Two dice that slide in, bounce and tumble to a stop centered on `cx`, resting at height
/// `rest_y`. Once landed, the sum and the damage appear above or below them.
fn draw_roll(
    b: &mut Builder,
    cx: f32,
    rest_y: f32,
    scale: f32,
    (first, second, age): (u32, u32, f32),
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
    let sum = first + second;
    let text_scale = scale * 1.5;
    let line_height = 8.0 * text_scale;
    let (sum_y, damage_y) = if text_below {
        let y = rest_y + die_size * 0.5 + 3.0 * scale;
        (y, y + line_height)
    } else {
        let y = rest_y - die_size * 0.5 - 3.0 * scale - line_height;
        (y, y - line_height)
    };
    b.centered_text(cx, sum_y, text_scale, &format!("{first} + {second} = {sum}"), style.sum_color);
    if age >= ROLL_ANIM_SECONDS + DAMAGE_LINE_DELAY {
        b.centered_text(cx, damage_y, text_scale, &format!("DAMAGE {sum}"), style.damage_color);
    }
}

/// Everything drawn on top of the world: crosshair, resource counters, dice, health bar.
pub fn build(
    width: u32,
    height: u32,
    // grey, white, green, brown
    resources: [u32; 4],
    // Each roll is (first die, second die, seconds since the roll started).
    player_roll: Option<(u32, u32, f32)>,
    enemy_rolls: &[(u32, u32, f32)],
    // (health shown on the bar, actual health, max health). The shown value trails the actual
    // one after a hit, which paints the draining chunk.
    hp: (f32, i32, i32),
) -> Vec<HudVertex> {
    let (w, h) = (width as f32, height as f32);
    let mut b = Builder {
        width: w,
        height: h,
        verts: Vec::with_capacity(1024),
    };

    // Crosshair: four small bars around the center.
    let white = [1.0, 1.0, 1.0];
    let (thick, len, gap) = (2.0, 10.0, 4.0);
    let (cx, cy) = (w / 2.0, h / 2.0);
    b.rect(cx + gap, cy - thick / 2.0, len, thick, white);
    b.rect(cx - gap - len, cy - thick / 2.0, len, thick, white);
    b.rect(cx - thick / 2.0, cy + gap, thick, len, white);
    b.rect(cx - thick / 2.0, cy - gap - len, thick, len, white);

    // Upper-left: one row per resource, a colored square and its count.
    let scale = (h / 270.0).floor().max(2.0);
    let margin = 4.0 * scale;
    for (row, (count, color)) in resources.iter().zip(RESOURCE_COLORS).enumerate() {
        let y = margin + row as f32 * 8.0 * scale;
        b.rect(margin, y, 6.0 * scale, 6.0 * scale, [0.10, 0.10, 0.12]);
        b.rect(margin + scale, y + scale, 4.0 * scale, 4.0 * scale, color);
        b.text(margin + 8.0 * scale, y + 0.5 * scale, scale, &count.to_string(), white);
    }

    // Top center: the player's dice.
    if let Some(roll) = player_roll {
        let top = h * 0.2;
        b.centered_text(cx, top - 16.0 * scale * 1.25, scale * 1.5, "2D6", [0.9, 0.9, 0.9]);
        draw_roll(&mut b, cx, top, scale, roll, &PLAYER_DICE, true);
    }

    // Bottom center: health bar, with the skeleton's red dice just above it.
    let (shown, actual, max) = hp;
    let max = max.max(1) as f32;
    let (bar_w, bar_h) = (80.0 * scale, 6.0 * scale);
    let (bar_x, bar_y) = (cx - bar_w / 2.0, h - 8.0 * scale - bar_h);
    b.rect(
        bar_x - scale,
        bar_y - scale,
        bar_w + 2.0 * scale,
        bar_h + 2.0 * scale,
        [0.05, 0.05, 0.06],
    );
    b.rect(bar_x, bar_y, bar_w, bar_h, [0.30, 0.05, 0.05]);
    let fraction = |v: f32| (v / max).clamp(0.0, 1.0);
    // The trail first (what was just lost), then the real health over it.
    b.rect(bar_x, bar_y, bar_w * fraction(shown), bar_h, [0.95, 0.60, 0.20]);
    b.rect(bar_x, bar_y, bar_w * fraction(actual as f32), bar_h, [0.25, 0.78, 0.28]);
    b.centered_text(
        cx,
        bar_y - 7.0 * scale,
        scale,
        &format!("HP {}/{}", actual.max(0), max as i32),
        white,
    );
    // One pair of red dice per attacker, side by side.
    let rest_y = bar_y - 7.0 * scale - 3.0 * scale - 8.0 * scale;
    let spacing = 17.0 * 4.0 * scale;
    let count = enemy_rolls.len() as f32;
    for (i, roll) in enemy_rolls.iter().enumerate() {
        let x = cx + (i as f32 - (count - 1.0) / 2.0) * spacing;
        draw_roll(&mut b, x, rest_y, scale, *roll, &ENEMY_DICE, false);
    }

    b.verts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_always_fits_in_the_buffer() {
        for age in [0.0, 0.3, 0.9, 1.4, 1.5, 1.7, 2.5, 3.0] {
            for roll in [(1, 1, age), (6, 6, age), (3, 5, age)] {
                let four = [roll; 4];
                let v = build(3840, 2160, [99999; 4], Some(roll), &four, (50.0, 50, 50));
                assert!(v.len() <= MAX_HUD_VERTICES, "{} at age {age}", v.len());
            }
        }
        let quiet = build(1920, 1080, [0; 4], None, &[], (50.0, 50, 50));
        assert!(quiet.len() < 3000, "{}", quiet.len());
    }
}
