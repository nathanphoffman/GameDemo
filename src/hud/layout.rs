use super::KINDS;

/// Screen size in pixels, the size of one font pixel, and the gap kept from the edges.
#[derive(Clone, Copy)]
pub(super) struct Screen {
    pub w: f32,
    pub h: f32,
    pub scale: f32,
    pub margin: f32,
}

impl Screen {
    pub(super) fn new(width: u32, height: u32) -> Self {
        let scale = ui_scale(height as f32);
        Screen {
            w: width as f32,
            h: height as f32,
            scale,
            margin: 4.0 * scale,
        }
    }
}

/// Size of one font pixel, in screen pixels.
fn ui_scale(height: f32) -> f32 {
    (height / 270.0).floor().max(2.0)
}

/// Where a point in the world lands on the screen, in pixels from the top left, or None if
/// it is behind the camera.
pub fn project_to_screen(
    view_proj: glam::Mat4,
    point: glam::Vec3,
    width: f32,
    height: f32,
) -> Option<(f32, f32)> {
    let clip = view_proj * point.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = (clip.x / clip.w, clip.y / clip.w);
    Some(((ndc.0 * 0.5 + 0.5) * width, (0.5 - ndc.1 * 0.5) * height))
}

/// Width of a resource row (including its backing), in font pixels.
pub(super) const ROW_WIDTH: f32 = 61.0;

/// Which resource row (0 = grey ... 4 = black) is under the pixel (x, y), if any.
pub fn resource_row_at(height: u32, x: f32, y: f32) -> Option<usize> {
    let scale = ui_scale(height as f32);
    let margin = 4.0 * scale;
    if x < margin - scale || x > margin + (ROW_WIDTH - 1.0) * scale {
        return None;
    }
    (0..KINDS).find(|row| {
        let top = margin + *row as f32 * 8.0 * scale;
        y >= top - scale && y < top + 7.0 * scale
    })
}

// The apply menu, in font pixels: a panel in the middle of the screen with a title and one
// row per dense resource.
pub(super) const MENU_WIDTH: f32 = 70.0;
pub(super) const MENU_TITLE_HEIGHT: f32 = 11.0;
pub(super) const MENU_ROW_PITCH: f32 = 9.0;
pub(super) const MENU_HEIGHT: f32 = MENU_TITLE_HEIGHT + KINDS as f32 * MENU_ROW_PITCH + 2.0;

/// Top-left corner of the panel, and the font pixel size.
pub(super) fn menu_origin(width: u32, height: u32) -> (f32, f32, f32) {
    let scale = ui_scale(height as f32);
    (
        width as f32 / 2.0 - MENU_WIDTH * scale / 2.0,
        height as f32 / 2.0 - MENU_HEIGHT * scale / 2.0,
        scale,
    )
}

/// What a click at (x, y) means while the menu is open: `Some(Some(kind))` for a row,
/// `Some(None)` for the panel itself, `None` for outside it (which closes the menu).
pub fn menu_click_at(width: u32, height: u32, x: f32, y: f32) -> Option<Option<usize>> {
    let (x0, y0, scale) = menu_origin(width, height);
    if x < x0 || x > x0 + MENU_WIDTH * scale || y < y0 || y > y0 + MENU_HEIGHT * scale {
        return None;
    }
    let row = ((y - y0 - MENU_TITLE_HEIGHT * scale) / (MENU_ROW_PITCH * scale)).floor();
    if row >= 0.0 && (row as usize) < KINDS {
        Some(Some(row as usize))
    } else {
        Some(None)
    }
}
