use super::font::glyph;
use super::HudVertex;

/// Collects the HUD's triangles. All positions are in pixels from the top-left of the screen.
pub(super) struct Builder {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) verts: Vec<HudVertex>,
}

impl Builder {
    /// Rectangle in pixels, measured from the top-left of the screen.
    pub(super) fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 3]) {
        self.rect_alpha(x, y, w, h, color, 1.0);
    }

    /// Same, but only `alpha` solid (0 to 1).
    pub(super) fn rect_alpha(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 3], alpha: f32) {
        let to_ndc = |px: f32, py: f32| [px / self.width * 2.0 - 1.0, 1.0 - py / self.height * 2.0];
        let (a, b, c, d) = (
            to_ndc(x, y),
            to_ndc(x + w, y),
            to_ndc(x + w, y + h),
            to_ndc(x, y + h),
        );
        for pos in [a, b, c, a, c, d] {
            self.verts.push(HudVertex { pos, color, alpha });
        }
    }

    /// A bar: a backing, with `fraction` (0 to 1) of its width filled from the left. Used for
    /// how far the converter is through its wait, and for how much sickness is left.
    pub(super) fn progress_bar(
        &mut self,
        rect: (f32, f32, f32, f32),
        fraction: f32,
        backing: [f32; 3],
        fill: [f32; 3],
    ) {
        let (x, y, w, h) = rect;
        self.rect(x, y, w, h, backing);
        self.rect(x, y, w * fraction.clamp(0.0, 1.0), h, fill);
    }

    /// Filled triangle with its corners given in pixels.
    pub(super) fn triangle(&mut self, corners: [(f32, f32); 3], color: [f32; 3]) {
        for (x, y) in corners {
            self.verts.push(HudVertex {
                pos: [x / self.width * 2.0 - 1.0, 1.0 - y / self.height * 2.0],
                color,
                alpha: 1.0,
            });
        }
    }

    /// Filled square centered on (cx, cy), turned by `angle` radians.
    pub(super) fn turned_square(&mut self, cx: f32, cy: f32, half: f32, angle: f32, color: [f32; 3]) {
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
            self.verts.push(HudVertex { pos, color, alpha: 1.0 });
        }
    }

    /// Text centered horizontally on `cx`.
    pub(super) fn centered_text(&mut self, cx: f32, y: f32, scale: f32, text: &str, color: [f32; 3]) {
        let width = text.chars().count() as f32 * 4.0 * scale - scale;
        self.text(cx - width / 2.0, y, scale, text, color);
    }

    pub(super) fn text(&mut self, x: f32, y: f32, scale: f32, text: &str, color: [f32; 3]) {
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
