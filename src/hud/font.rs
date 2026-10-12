/// 3x5 pixel font: each row is 3 bits, most significant bit on the left.
pub(super) fn glyph(c: char) -> [u8; 5] {
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
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '=' => [0b000, 0b111, 0b000, 0b111, 0b000],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b111, 0b101, 0b101, 0b101, 0b111],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'Q' => [0b010, 0b101, 0b101, 0b111, 0b011],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '?' => [0b111, 0b001, 0b011, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '\'' => [0b010, 0b010, 0b000, 0b000, 0b000],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '*' => [0b101, 0b010, 0b111, 0b010, 0b101],
        _ => [0; 5],
    }
}

/// `text` as the font can show it: capitals only, and `?` for anything it has no letter for.
pub fn display_text(text: &str) -> String {
    text.chars()
        .map(|c| {
            let c = c.to_ascii_uppercase();
            if c == ' ' || glyph(c) != [0; 5] { c } else { '?' }
        })
        .collect()
}

/// `text` broken into rows of at most `width` letters, at spaces where it can be. Rows after
/// the first are indented two letters.
pub(super) fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    // The letters of the row being built, not counting its indent.
    let mut row = String::new();
    let mut len = 0;
    let indent_of = |rows: &Vec<String>| if rows.is_empty() { 0 } else { 2 };
    let finish = |rows: &mut Vec<String>, row: &mut String| {
        let indent = indent_of(rows);
        rows.push(format!("{}{}", " ".repeat(indent), row));
        row.clear();
    };
    for word in text.split(' ').filter(|w| !w.is_empty()) {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let room = width.saturating_sub(indent_of(&rows)).max(1);
            let gap = usize::from(len > 0);
            if len + gap + word.len() <= room {
                if len > 0 {
                    row.push(' ');
                }
                row.extend(word.iter());
                len += gap + word.len();
                break;
            }
            if len > 0 {
                finish(&mut rows, &mut row);
                len = 0;
                continue;
            }
            // A word longer than a whole row is chopped.
            let rest = word.split_off(room);
            row.extend(word.iter());
            finish(&mut rows, &mut row);
            word = rest;
        }
    }
    if len > 0 || rows.is_empty() {
        finish(&mut rows, &mut row);
    }
    rows
}
