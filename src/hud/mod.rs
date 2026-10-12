//! The 2D overlay: a pixel font, rectangles and triangles, built fresh each frame.

mod builder;
mod chat;
mod dice;
mod font;
mod info;
mod layout;
mod resources;
mod vitals;

#[cfg(test)]
mod tests;

use bytemuck::{Pod, Zeroable};

use builder::Builder;
use layout::Screen;

pub use font::display_text;
pub use info::HudInfo;
pub use layout::{menu_click_at, project_to_screen, resource_row_at};

/// Room for the crosshair, the rock counter, the dice, and the chat.
pub const MAX_HUD_VERTICES: usize = 300_000;

/// Row colors for grey (stone), white (snow), green (grass and leaves), brown (wood) and
/// black (coal).
const RESOURCE_COLORS: [[f32; 3]; 5] = [
    [0.55, 0.56, 0.60],
    [0.96, 0.97, 1.00],
    [0.30, 0.68, 0.22],
    [0.50, 0.32, 0.17],
    [0.06, 0.06, 0.07],
];

/// Number of resource kinds, and so rows in the list and in the apply menu.
const KINDS: usize = 5;

const WHITE: [f32; 3] = [1.0, 1.0, 1.0];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub color: [f32; 3],
    /// 1 is solid; less lets the scene show through.
    pub alpha: f32,
}

/// Everything drawn on top of the world: crosshair, resource counters, dice, health bar.
pub fn build(width: u32, height: u32, info: &HudInfo) -> Vec<HudVertex> {
    let s = Screen::new(width, height);
    let mut b = Builder {
        width: s.w,
        height: s.h,
        verts: Vec::with_capacity(1024),
    };

    // Crosshair: four small bars around the center.
    let (thick, len, gap) = (2.0, 10.0, 4.0);
    let (cx, cy) = (s.w / 2.0, s.h / 2.0);
    b.rect(cx + gap, cy - thick / 2.0, len, thick, WHITE);
    b.rect(cx - gap - len, cy - thick / 2.0, len, thick, WHITE);
    b.rect(cx - thick / 2.0, cy + gap, thick, len, WHITE);
    b.rect(cx - thick / 2.0, cy - gap - len, thick, len, WHITE);

    resources::draw_rows(&mut b, info, &s);

    // Upper right: the current weather.
    let label = info.weather_label;
    let text_width = label.chars().count() as f32 * 4.0 * s.scale - s.scale;
    b.text(s.w - s.margin - text_width, s.margin, s.scale, label, [0.85, 0.9, 1.0]);

    resources::draw_selection(&mut b, info.selection, &s);

    // Over the aimed-at block: what the selected resource would build there.
    if let Some((label, px, py)) = info.target_label {
        let text_scale = s.scale * 1.5;
        b.centered_text(px, py - 3.5 * text_scale, text_scale, label, [1.0, 0.85, 0.3]);
    }

    if let Some(labels) = info.menu {
        resources::draw_menu(&mut b, labels, info.dense, &s);
    }
    if let Some(roll) = info.player_roll {
        vitals::draw_player_roll(&mut b, roll, &s);
    }
    vitals::draw_health(&mut b, info.hp, info.enemy_rolls, &s);
    if let Some(sickness) = info.sickness {
        vitals::draw_sickness(&mut b, sickness, &s);
    }

    // Names over other players' heads.
    for (name, x, y) in info.name_tags {
        b.centered_text(*x, *y, s.scale, name, [1.0, 1.0, 0.8]);
    }

    chat::draw_chat(&mut b, info.chat, info.chat_input, info.chat_box, &s);
    if let Some(names) = info.player_list {
        chat::draw_player_list(&mut b, names, &s);
    }

    b.verts
}
