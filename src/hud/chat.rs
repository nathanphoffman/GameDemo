use super::builder::Builder;
use super::font::wrap_text;
use super::layout::Screen;
use super::WHITE;

/// The most wrapped chat rows on screen at once.
const MAX_CHAT_ROWS: usize = 12;

/// Lower left: the chat, with the line being typed at the bottom. Lines wrap at half the
/// width of the screen, on a grey box that fades in while there is something to read.
pub(super) fn draw_chat(
    b: &mut Builder,
    chat: &[String],
    chat_input: Option<&str>,
    chat_box: f32,
    s: &Screen,
) {
    let Screen { w, h, scale, margin } = *s;
    let fits = (((w * 0.5 - 2.0 * margin) / (4.0 * scale)) as usize).max(8);
    let line = 7.0 * scale;
    let bottom_edge = h - margin - 6.0 * scale;
    let mut bottom = bottom_edge;
    let mut rows: Vec<String> = Vec::new();
    for text in chat {
        rows.extend(wrap_text(text, fits));
    }
    let skip = rows.len().saturating_sub(MAX_CHAT_ROWS);
    let rows = &rows[skip..];
    let typing_rows = usize::from(chat_input.is_some());
    if chat_box > 0.0 && (!rows.is_empty() || typing_rows > 0) {
        let height = (rows.len() + typing_rows) as f32 * line + 2.0 * scale;
        b.rect_alpha(
            margin - 2.0 * scale,
            bottom_edge + line - height - scale,
            w * 0.5,
            height + 2.0 * scale,
            [0.12, 0.12, 0.14],
            0.5 * chat_box.clamp(0.0, 1.0),
        );
    }
    if let Some(typing) = chat_input {
        let skip = (typing.chars().count() + 3).saturating_sub(fits);
        let shown = format!("> {}_", typing.chars().skip(skip).collect::<String>());
        b.text(margin, bottom, scale, &shown, WHITE);
        bottom -= line;
    }
    for (i, text) in rows.iter().rev().enumerate() {
        b.text(margin, bottom - i as f32 * line, scale, text, [0.9, 0.95, 1.0]);
    }
}

/// Top center: who is playing, while the list is held.
pub(super) fn draw_player_list(b: &mut Builder, names: &[String], s: &Screen) {
    let Screen { w, scale, margin, .. } = *s;
    let cx = w / 2.0;
    let line = 7.0 * scale;
    let rows = names.len() as f32 + 1.0;
    let half = 45.0 * scale;
    b.rect(cx - half, margin - scale, half * 2.0, rows * line + 2.0 * scale, [0.06, 0.06, 0.10]);
    b.centered_text(cx, margin, scale, "PLAYERS", [1.0, 0.8, 0.3]);
    for (i, name) in names.iter().enumerate() {
        b.centered_text(cx, margin + (i as f32 + 1.0) * line, scale, name, WHITE);
    }
}
