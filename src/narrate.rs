//! The words for what happens in a fight and in the weather, for the chat box.

use crate::biome::Biome;
use crate::enemy::Kind as Creature;
use crate::weather::Kind as Weather;

fn name(creature: Creature) -> &'static str {
    match creature {
        Creature::Deer => "deer",
        Creature::Wolf => "wolf",
        Creature::Boar => "boar",
        Creature::BogZombie => "bog zombie",
        Creature::Scorpion => "scorpion",
        Creature::Golem => "golem",
        Creature::Skeleton => "skeleton",
        Creature::Yeti => "yeti",
    }
}

/// "2+4 (6)", or "2+4+3 (9)" when there is a bonus.
fn sum_text(dice: (u32, u32), bonus: i32) -> String {
    let total = (dice.0 + dice.1) as i32 + bonus;
    if bonus > 0 {
        format!("{}+{}+{} ({})", dice.0, dice.1, bonus, total)
    } else {
        format!("{}+{} ({})", dice.0, dice.1, total)
    }
}

/// Picks one of `options` from the dice, so the same roll always reads the same way but
/// different rolls vary.
fn pick<'a>(options: &[&'a str], dice: (u32, u32)) -> &'a str {
    options[(dice.0 * 3 + dice.1) as usize % options.len()]
}

/// What a creature did to the player.
pub fn creature_hit(creature: Creature, dice: (u32, u32), bonus: i32) -> String {
    let how = match creature {
        Creature::Deer => ["kicked you with its hooves", "rammed you with its antlers"],
        Creature::Wolf => ["bit you", "sank its teeth into your leg"],
        Creature::Boar => ["gored you with its tusks", "charged into you"],
        Creature::BogZombie => ["clawed you with rotting hands", "slapped you with a slimy fist"],
        Creature::Scorpion => ["stung you with its tail", "pinched you with a claw"],
        Creature::Golem => ["slammed you with a stone fist", "stomped on you"],
        Creature::Skeleton => ["stabbed you with a rusty knife", "whacked you with a bony arm"],
        Creature::Yeti => ["clubbed you with a frozen fist", "mauled you with its claws"],
    };
    format!(
        "The {} {} for {} damage",
        name(creature),
        pick(&how, dice),
        sum_text(dice, bonus)
    )
}

/// What the player did to a creature (with bare fists, for now).
pub fn player_hit(creature: Creature, dice: (u32, u32)) -> String {
    let total = dice.0 + dice.1;
    let who = name(creature);
    let sum = sum_text(dice, 0);
    if total <= 4 {
        let ending = pick(&["your fist hurts now", "that stung you more than it", "ouch"], dice);
        format!("You punched the {who} pretty poorly for {sum} damage, {ending}")
    } else if total <= 8 {
        format!("You punched the {who} for {sum} damage")
    } else {
        let how = pick(&["landed a solid punch on", "walloped"], dice);
        format!("You {how} the {who} for {sum} damage")
    }
}

/// The line after a creature is brought down.
pub fn creature_down(creature: Creature) -> String {
    let who = name(creature);
    match creature {
        Creature::Skeleton => "The skeleton falls apart.".to_string(),
        Creature::Golem => "The golem crumbles into rubble.".to_string(),
        Creature::BogZombie => "The bog zombie collapses into the muck.".to_string(),
        _ => format!("The {who} collapses."),
    }
}

/// What the sky is doing now (`weather` is what reaches the player, after their biome).
pub fn weather_change(weather: Weather, snowing: bool, biome: Biome) -> &'static str {
    match (weather, snowing) {
        (Weather::Sunny, _) => "The clouds part and the sun comes out.",
        (Weather::Fog, _) => "A thick fog rolls in.",
        (Weather::LightRain, false) => "It starts to rain.",
        (Weather::LightRain, true) => "It starts to snow.",
        (Weather::HeavyRain, false) => "The rain comes down hard.",
        (Weather::HeavyRain, true) => "A heavy snowfall begins.",
        (Weather::Windy, _) if biome == Biome::Desert => "A dust storm kicks up.",
        (Weather::Windy, _) => "The wind picks up.",
        (Weather::Mist, _) => "A light mist settles in.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::display_text;

    #[test]
    fn lines_read_like_the_examples() {
        assert_eq!(
            creature_hit(Creature::Skeleton, (2, 4), 0),
            "The skeleton stabbed you with a rusty knife for 2+4 (6) damage"
        );
        assert!(player_hit(Creature::Skeleton, (1, 1)).starts_with(
            "You punched the skeleton pretty poorly for 1+1 (2) damage, "
        ));
        assert!(creature_hit(Creature::Skeleton, (2, 4), 3).contains("2+4+3 (9)"));
    }

    #[test]
    fn every_line_can_be_shown_by_the_font() {
        for creature in crate::enemy::Kind::ALL {
            for a in 1..=6 {
                for b in 1..=6 {
                    for line in [
                        creature_hit(creature, (a, b), 5),
                        player_hit(creature, (a, b)),
                        creature_down(creature),
                    ] {
                        assert!(!display_text(&line).contains('?'), "{line}");
                    }
                }
            }
        }
        for kind in crate::weather::Kind::ALL {
            for snowing in [false, true] {
                let line = weather_change(kind, snowing, Biome::Desert);
                assert!(!display_text(line).contains('?'), "{line}");
            }
        }
    }
}
