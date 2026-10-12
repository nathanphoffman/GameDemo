use super::KINDS;

/// Everything the HUD shows.
#[derive(Clone, Copy)]
pub struct HudInfo<'a> {
    /// Grey, white, green, brown, black.
    pub resources: [u32; KINDS],
    /// The dense (triangle) version of each.
    pub dense: [u32; KINDS],
    /// The apply menu, if open: for each dense resource, the name of what applying it to the
    /// chosen block would do and how many it uses (None if nothing).
    pub menu: Option<[Option<(&'static str, u32)>; KINDS]>,
    /// Which resource the converter is working on, and how far through its wait (0 to 1).
    pub converting: Option<(usize, f32)>,
    /// Ctrl is held, so the resource rows can be clicked.
    pub ctrl_held: bool,
    /// Name of the current weather, shown in the upper right corner.
    pub weather_label: &'static str,
    /// What right click does: `None` clones the block it is aimed at, `Some(kind)` applies
    /// that dense resource (0 = grey ... 4 = black) and how many of it you have.
    pub selection: Option<(usize, u32)>,
    /// What the selected resource would build on the block being aimed at, and where that
    /// block is on the screen in pixels.
    pub target_label: Option<(&'static str, f32, f32)>,
    /// How many times faster than normal conversion is running (1, 2 with a forge nearby, 4
    /// with a furnace).
    pub conversion_boost: u32,
    /// Each roll is (first die, second die, seconds since the roll started).
    pub player_roll: Option<(u32, u32, i32, f32)>,
    pub enemy_rolls: &'a [(u32, u32, i32, f32)],
    /// (health shown on the bar, actual health, max health). The shown value trails the actual
    /// one after a hit, which paints the draining chunk.
    pub hp: (f32, i32, i32),
    /// Resurrection sickness: seconds left and how long it lasts in all.
    pub sickness: Option<(f32, f32)>,
    /// Recent chat lines, oldest first, already in `display_text` form.
    pub chat: &'a [String],
    /// The line being typed, if the chat is open.
    pub chat_input: Option<&'a str>,
    /// How solid the grey box behind the chat is, 0 (gone) to 1 (fully faded in).
    pub chat_box: f32,
    /// The names of everyone playing, if the list is being shown.
    pub player_list: Option<&'a [String]>,
    /// Names to float over other players: the text and where on the screen (pixels).
    pub name_tags: &'a [(String, f32, f32)],
}
