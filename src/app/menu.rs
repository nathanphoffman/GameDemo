//! The apply menu.

use super::*;

impl App {
    /// Opens the apply menu on the block under the crosshair, freeing the mouse.
    pub(crate) fn open_menu(&mut self) {
        if self.ui_open() {
            return;
        }
        if let Some(target) = self.game.block_in_sights(self.me) {
            self.menu = Some(target);
            self.set_mining(false);
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
        if self.game.can_apply(self.me, kind, pos) {
            self.command(Command::ApplyAt { kind, pos });
            self.close_menu();
        }
    }
}
