use super::builder::Builder;
use super::layout::{menu_origin, Screen, MENU_HEIGHT, MENU_ROW_PITCH, MENU_TITLE_HEIGHT, MENU_WIDTH, ROW_WIDTH};
use super::{HudInfo, KINDS, RESOURCE_COLORS, WHITE};

/// Upper-left: one row per resource: a colored square and its count, then a triangle of
/// the same color and the count of its dense version. The row being converted has an
/// amber backing that fills up over the wait, and with Ctrl held every row gets a backing
/// to show it can be clicked.
pub(super) fn draw_rows(b: &mut Builder, info: &HudInfo, s: &Screen) {
    let Screen { scale, margin, .. } = *s;
    let HudInfo { resources, dense, converting, ctrl_held, conversion_boost, .. } = *info;
    for row in 0..KINDS {
        let (count, dense_count, color) = (resources[row], dense[row], RESOURCE_COLORS[row]);
        let y = margin + row as f32 * 8.0 * scale;
        let selected = converting.filter(|(kind, _)| *kind == row);
        if selected.is_some() || ctrl_held {
            let (x0, y0, w0, h0) = (
                margin - scale,
                y - 0.5 * scale,
                ROW_WIDTH * scale,
                7.0 * scale,
            );
            let backing = if selected.is_some() {
                [0.25, 0.19, 0.04]
            } else {
                [0.16, 0.16, 0.19]
            };
            // Amber, filling up over the wait.
            let fraction = selected.map_or(0.0, |(_, fraction)| fraction);
            b.progress_bar((x0, y0, w0, h0), fraction, backing, [0.50, 0.38, 0.08]);
            if selected.is_some() {
                if conversion_boost > 1 {
                    let label = format!("X{conversion_boost}");
                    b.text(x0 + (ROW_WIDTH + 2.0) * scale, y + 0.5 * scale, scale, &label, [1.0, 0.65, 0.2]);
                }
            }
        }
        // Black needs a light frame to be seen against the dark HUD.
        let frame = if row == KINDS - 1 {
            [0.70, 0.70, 0.75]
        } else {
            [0.10, 0.10, 0.12]
        };
        b.rect(margin, y, 6.0 * scale, 6.0 * scale, frame);
        b.rect(margin + scale, y + scale, 4.0 * scale, 4.0 * scale, color);
        b.text(margin + 8.0 * scale, y + 0.5 * scale, scale, &count.to_string(), WHITE);

        // Dense version: an upward triangle with a dark outline.
        let tx = margin + 31.0 * scale;
        let o = 0.9 * scale;
        b.triangle(
            [
                (tx + 3.0 * scale, y - o),
                (tx - o * 1.6, y + 6.0 * scale + o * 0.6),
                (tx + 6.0 * scale + o * 1.6, y + 6.0 * scale + o * 0.6),
            ],
            frame,
        );
        b.triangle(
            [
                (tx + 3.0 * scale, y + 0.2 * scale),
                (tx, y + 6.0 * scale),
                (tx + 6.0 * scale, y + 6.0 * scale),
            ],
            color,
        );
        b.text(
            margin + 40.0 * scale,
            y + 0.5 * scale,
            scale,
            &dense_count.to_string(),
            WHITE,
        );
    }
}

/// Lower right: what right click will do.
pub(super) fn draw_selection(b: &mut Builder, selection: Option<(usize, u32)>, s: &Screen) {
    let Screen { w, h, scale, margin } = *s;
    let y = h - margin - 7.0 * scale;
    match selection {
        None => {
            let label = "CLONE";
            let text_width = label.len() as f32 * 4.0 * scale - scale;
            b.text(w - margin - text_width, y, scale, label, [0.85, 0.85, 0.9]);
        }
        Some((kind, count)) => {
            let text = count.to_string();
            let text_width = text.len() as f32 * 4.0 * scale - scale;
            let color = RESOURCE_COLORS[kind.min(KINDS - 1)];
            let frame = if kind == KINDS - 1 {
                [0.70, 0.70, 0.75]
            } else {
                [0.10, 0.10, 0.12]
            };
            let right = w - margin;
            b.text(right - text_width, y, scale, &text, WHITE);
            let tx = right - text_width - 9.0 * scale;
            let o = 0.9 * scale;
            b.triangle(
                [
                    (tx + 3.0 * scale, y - o),
                    (tx - o * 1.6, y + 6.0 * scale + o * 0.6),
                    (tx + 6.0 * scale + o * 1.6, y + 6.0 * scale + o * 0.6),
                ],
                frame,
            );
            b.triangle(
                [
                    (tx + 3.0 * scale, y + 0.2 * scale),
                    (tx, y + 6.0 * scale),
                    (tx + 6.0 * scale, y + 6.0 * scale),
                ],
                color,
            );
        }
    }
}

/// The apply menu, in the middle of the screen.
pub(super) fn draw_menu(
    b: &mut Builder,
    labels: [Option<(&'static str, u32)>; KINDS],
    dense: [u32; KINDS],
    s: &Screen,
) {
    let (x0, y0, scale) = menu_origin(s.w as u32, s.h as u32);
    b.rect(x0 - scale, y0 - scale, (MENU_WIDTH + 2.0) * scale, (MENU_HEIGHT + 2.0) * scale, [0.02, 0.02, 0.03]);
    b.rect(x0, y0, MENU_WIDTH * scale, MENU_HEIGHT * scale, [0.14, 0.14, 0.18]);
    b.text(x0 + 3.0 * scale, y0 + 3.0 * scale, scale, "APPLY", WHITE);
    for row in 0..KINDS {
        let top = y0 + MENU_TITLE_HEIGHT * scale + row as f32 * MENU_ROW_PITCH * scale;
        let usable = labels[row].is_some_and(|(_, cost)| dense[row] >= cost);
        let backing = if usable { [0.45, 0.33, 0.06] } else { [0.20, 0.20, 0.25] };
        b.rect(x0 + 2.0 * scale, top, (MENU_WIDTH - 4.0) * scale, 8.0 * scale, backing);
        let color = RESOURCE_COLORS[row];
        let frame = if row == KINDS - 1 { [0.70, 0.70, 0.75] } else { [0.05, 0.05, 0.06] };
        let tx = x0 + 5.0 * scale;
        b.triangle(
            [
                (tx + 3.5 * scale, top + 0.2 * scale),
                (tx - 0.8 * scale, top + 7.2 * scale),
                (tx + 7.8 * scale, top + 7.2 * scale),
            ],
            frame,
        );
        b.triangle(
            [
                (tx + 3.5 * scale, top + 1.2 * scale),
                (tx + 0.4 * scale, top + 6.6 * scale),
                (tx + 6.6 * scale, top + 6.6 * scale),
            ],
            color,
        );
        b.text(x0 + 16.0 * scale, top + 1.5 * scale, scale, &dense[row].to_string(), WHITE);
        if let Some((label, _)) = labels[row] {
            let tint = if usable { [1.0, 0.65, 0.2] } else { [0.5, 0.5, 0.55] };
            b.text(x0 + 34.0 * scale, top + 1.5 * scale, scale, label, tint);
        }
    }
}
