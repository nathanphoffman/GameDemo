//! The apply menu and applying a resource to a block.

use super::*;

impl App {
    /// Opens the apply menu on the block under the crosshair, freeing the mouse.
    pub(crate) fn open_menu(&mut self) {
        if self.ui_open() {
            return;
        }
        if let Some(target) = self.block_in_sights() {
            self.menu = Some(target);
            self.keys.mining = false;
            self.release_cursor();
        }
    }

    pub(crate) fn close_menu(&mut self) {
        self.menu = None;
        if self.focused && !self.keys.ctrl {
            self.grab_cursor();
        }
    }

    /// A left click while the menu is open: apply a resource, or close it.
    pub(crate) fn menu_click(&mut self) {
        let (width, height) = self
            .gfx
            .as_ref()
            .map_or((0, 0), |g| (g.config.width, g.config.height));
        let (x, y) = self.cursor_pos;
        match hud::menu_click_at(width, height, x, y) {
            None => self.close_menu(),
            Some(None) => {}
            Some(Some(kind)) => self.apply_resource(kind),
        }
    }

    /// Applies one dense resource of `kind` to the block the menu is open on. If it does
    /// nothing to that block, nothing is used up and the menu stays open.
    pub(crate) fn apply_resource(&mut self, kind: usize) {
        let Some(pos) = self.menu else {
            return;
        };
        if self.apply_dense(kind, pos) {
            self.close_menu();
        }
    }

    /// Right click with a dense resource selected: applies it to the block under the
    /// crosshair, just like picking it from the menu.
    pub(crate) fn apply_selected(&mut self, kind: usize) {
        if let Some(pos) = self.block_in_sights() {
            self.apply_dense(kind, pos);
        }
    }
}
