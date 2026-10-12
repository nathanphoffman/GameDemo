use super::builder::Builder;
use super::dice::{draw_roll, ENEMY_DICE, PLAYER_DICE};
use super::layout::Screen;
use super::{KINDS, WHITE};

/// Top center: the player's dice.
pub(super) fn draw_player_roll(b: &mut Builder, roll: (u32, u32, i32, f32), s: &Screen) {
    let Screen { w, h, scale, .. } = *s;
    let cx = w / 2.0;
    let top = h * 0.2;
    b.centered_text(cx, top - 16.0 * scale * 1.25, scale * 1.5, "2D6", [0.9, 0.9, 0.9]);
    draw_roll(b, cx, top, scale, roll, &PLAYER_DICE, true);
}

/// Bottom right: health bar, with the skeleton's red dice just above it.
pub(super) fn draw_health(
    b: &mut Builder,
    hp: (f32, i32, i32),
    enemy_rolls: &[(u32, u32, i32, f32)],
    s: &Screen,
) {
    let Screen { w, h, scale, margin } = *s;
    let (shown, actual, max) = hp;
    let max = max.max(1) as f32;
    let (bar_w, bar_h) = (80.0 * scale, 6.0 * scale);
    // In the bottom right corner, out of the way of the chat.
    let (bar_x, bar_y) = (w - margin - bar_w, h - 8.0 * scale - bar_h);
    let bar_cx = bar_x + bar_w / 2.0;
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
        bar_cx,
        bar_y - 7.0 * scale,
        scale,
        &format!("HP {}/{}", actual.max(0), max as i32),
        WHITE,
    );
    // One pair of red dice per attacker, side by side.
    let rest_y = bar_y - 7.0 * scale - 3.0 * scale - 8.0 * scale;
    let spacing = 17.0 * 4.0 * scale;
    let count = enemy_rolls.len() as f32;
    // Centered over the bar, but slid left as far as it takes to keep every pair on screen.
    let row_cx = bar_cx.min(w - margin - 22.0 * scale - (count - 1.0).max(0.0) / 2.0 * spacing);
    for (i, roll) in enemy_rolls.iter().enumerate() {
        let x = row_cx + (i as f32 - (count - 1.0) / 2.0) * spacing;
        draw_roll(b, x, rest_y, scale, *roll, &ENEMY_DICE, false);
    }
}

/// Under the resource rows: resurrection sickness, as a purple bar that drains.
pub(super) fn draw_sickness(b: &mut Builder, (left, total): (f32, f32), s: &Screen) {
    let Screen { scale, margin, .. } = *s;
    let y = margin + KINDS as f32 * 8.0 * scale + 3.0 * scale;
    b.text(margin, y, scale, "RESURRECTION SICKNESS", [0.85, 0.6, 1.0]);
    let bar = (margin - scale, y + 7.0 * scale, 86.0 * scale, 7.0 * scale);
    b.progress_bar(bar, left / total.max(1.0), [0.14, 0.06, 0.20], [0.50, 0.22, 0.62]);
    let seconds = left.ceil().max(0.0) as u32;
    let time = format!("{}:{:02}", seconds / 60, seconds % 60);
    b.centered_text(bar.0 + bar.2 / 2.0, bar.1 + 1.0 * scale, scale, &time, WHITE);
    // What it means, in a few short lines.
    let what = ["MINING AND CONVERTING", "TAKE TWICE AS LONG.", "YOU ATTACK EVERY OTHER ROUND."];
    for (i, line) in what.iter().enumerate() {
        let y = bar.1 + bar.3 + (2.0 + i as f32 * 6.5) * scale;
        b.text(margin, y, scale, line, [0.78, 0.70, 0.88]);
    }
}
