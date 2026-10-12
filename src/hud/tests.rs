use super::font::{glyph, wrap_text};
use super::layout::{menu_origin, MENU_ROW_PITCH, MENU_TITLE_HEIGHT};
use super::*;

#[test]
fn hud_always_fits_in_the_buffer() {
    // The most chat that can be on screen at once, in the widest letters.
    let chat = vec!["W".repeat(200); 8];
    let names = vec!["WWWWWWWWWWWWWWWW".to_string(); 16];
    let tags = vec![("WWWWWWWWWWWWWWWW".to_string(), 960.0, 400.0); 16];
    for age in [0.0, 0.3, 0.9, 1.4, 1.5, 1.7, 2.5, 3.0] {
        for roll in [(1, 1, 0, age), (6, 6, 0, age), (3, 5, 5, age)] {
            let four = [roll; 4];
            let info = HudInfo {
                resources: [99999; KINDS],
                dense: [9999; KINDS],
                menu: Some([Some(("FIRE", 1)); KINDS]),
                converting: Some((2, 0.5)),
                ctrl_held: true,
                weather_label: "HEAVY RAIN",
                selection: Some((4, 99999)),
                target_label: Some(("FURNACE", 960.0, 540.0)),
                conversion_boost: 4,
                player_roll: Some(roll),
                enemy_rolls: &four,
                hp: (50.0, 50, 50),
                sickness: Some((59.0, 60.0)),
                chat: &chat,
                chat_input: Some("HELLO THERE FRIENDS"),
                chat_box: 1.0,
                player_list: Some(&names),
                name_tags: &tags,
            };
            let v = build(3840, 2160, &info);
            assert!(v.len() <= MAX_HUD_VERTICES, "{} at age {age}", v.len());
        }
    }
    let quiet = build(
        1920,
        1080,
        &HudInfo {
            resources: [0; KINDS],
            dense: [0; KINDS],
            menu: None,
            converting: None,
            ctrl_held: false,
            weather_label: "SUNNY",
            selection: None,
            target_label: None,
            conversion_boost: 1,
            player_roll: None,
            enemy_rolls: &[],
            hp: (50.0, 50, 50),
            sickness: None,
            chat: &[],
            chat_input: None,
            chat_box: 0.0,
            player_list: None,
            name_tags: &[],
        },
    );
    assert!(quiet.len() < 6000, "{}", quiet.len());
}

#[test]
fn long_text_wraps_at_spaces_and_chops_huge_words() {
    assert_eq!(wrap_text("ONE TWO THREE", 20), ["ONE TWO THREE"]);
    assert_eq!(wrap_text("ONE TWO THREE FOUR", 11), ["ONE TWO", "  THREE", "  FOUR"]);
    let chopped = wrap_text(&"W".repeat(25), 10);
    assert!(chopped.iter().all(|r| r.chars().count() <= 10), "{chopped:?}");
    assert_eq!(chopped.concat().replace(' ', "").len(), 25);
    assert_eq!(wrap_text("", 10), [""]);
}

#[test]
fn rows_can_be_hit_and_the_rest_of_the_screen_cannot() {
    let (h, scale) = (1080u32, 4.0f32);
    let margin = 4.0 * scale;
    for row in 0..KINDS {
        let y = margin + row as f32 * 8.0 * scale + 3.0 * scale;
        assert_eq!(resource_row_at(h, margin + 10.0, y), Some(row));
    }
    assert_eq!(resource_row_at(h, 900.0, 20.0), None, "far to the right");
    assert_eq!(resource_row_at(h, margin + 10.0, 600.0), None, "far below");
    assert_eq!(resource_row_at(h, 0.0, 0.0), None, "the corner");
}

#[test]
fn menu_rows_title_and_outside_are_told_apart() {
    let (w, h) = (1920u32, 1080u32);
    let (x0, y0, scale) = menu_origin(w, h);
    let mid_x = x0 + 30.0 * scale;
    for row in 0..KINDS {
        let y = y0 + (MENU_TITLE_HEIGHT + row as f32 * MENU_ROW_PITCH + 4.0) * scale;
        assert_eq!(menu_click_at(w, h, mid_x, y), Some(Some(row)));
    }
    assert_eq!(menu_click_at(w, h, mid_x, y0 + 3.0 * scale), Some(None), "the title");
    assert_eq!(menu_click_at(w, h, 10.0, 10.0), None, "outside");
    assert_eq!(menu_click_at(w, h, w as f32 - 10.0, h as f32 / 2.0), None, "outside");
}

#[test]
fn points_ahead_land_in_the_middle_and_points_behind_are_dropped() {
    use glam::{Mat4, Vec3};
    let view = Mat4::look_to_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y);
    let proj = Mat4::perspective_rh(80f32.to_radians(), 16.0 / 9.0, 0.05, 1000.0);
    let vp = proj * view;
    let (x, y) = project_to_screen(vp, Vec3::new(0.0, 0.0, -5.0), 1920.0, 1080.0).unwrap();
    assert!((x - 960.0).abs() < 0.5 && (y - 540.0).abs() < 0.5, "{x},{y}");
    // To the right and up goes right and up on the screen.
    let (rx, ry) = project_to_screen(vp, Vec3::new(1.0, 1.0, -5.0), 1920.0, 1080.0).unwrap();
    assert!(rx > 960.0 && ry < 540.0);
    assert!(project_to_screen(vp, Vec3::new(0.0, 0.0, 5.0), 1920.0, 1080.0).is_none());
}

#[test]
fn chat_text_is_made_showable() {
    assert_eq!(display_text("Hello, world! 42"), "HELLO, WORLD! 42");
    assert_eq!(display_text("a#b\u{e9}"), "A?B?");
    // Every letter and digit has a glyph.
    for c in ('A'..='Z').chain('0'..='9') {
        assert_ne!(glyph(c), [0; 5], "{c}");
    }
}
