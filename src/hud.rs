use bytemuck::{Pod, Zeroable};

/// Room for the crosshair, the rock counter and the mode label.
pub const MAX_HUD_VERTICES: usize = 4096;

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
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
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

/// Everything drawn on top of the world: crosshair, rock counter, current mode.
pub fn build(
    width: u32,
    height: u32,
    rocks: u32,
    mode_label: &str,
    mode_color: [f32; 3],
) -> Vec<HudVertex> {
    let (w, h) = (width as f32, height as f32);
    let mut b = Builder {
        width: w,
        height: h,
        verts: Vec::with_capacity(512),
    };

    // Crosshair: four small bars around the center.
    let white = [1.0, 1.0, 1.0];
    let (thick, len, gap) = (2.0, 10.0, 4.0);
    let (cx, cy) = (w / 2.0, h / 2.0);
    b.rect(cx + gap, cy - thick / 2.0, len, thick, white);
    b.rect(cx - gap - len, cy - thick / 2.0, len, thick, white);
    b.rect(cx - thick / 2.0, cy + gap, thick, len, white);
    b.rect(cx - thick / 2.0, cy - gap - len, thick, len, white);

    // Upper-left: rock icon + count, with the mode label underneath.
    let scale = (h / 270.0).floor().max(2.0);
    let margin = 4.0 * scale;
    b.rect(margin, margin, 6.0 * scale, 6.0 * scale, [0.17, 0.17, 0.19]);
    b.rect(
        margin + scale,
        margin + scale,
        4.0 * scale,
        4.0 * scale,
        [0.55, 0.56, 0.60],
    );
    b.text(margin + 8.0 * scale, margin + 0.5 * scale, scale, &rocks.to_string(), white);
    b.text(margin, margin + 8.0 * scale, scale, mode_label, mode_color);

    b.verts
}
