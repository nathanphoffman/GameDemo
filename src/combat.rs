use crate::enemy::Dice;

pub const MAX_PLAYER_HP: i32 = 50;

/// How long the dice tumble before they settle on their result.
pub const ROLL_ANIM_SECONDS: f32 = 1.5;
/// How long a finished roll stays on screen.
pub const ROLL_DISPLAY_SECONDS: f32 = 3.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Player,
    Enemy,
}

impl Side {
    fn other(self) -> Side {
        match self {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        }
    }
}

/// One 2d6 roll. Its damage lands when the dice finish tumbling.
#[derive(Clone, Copy, Debug)]
pub struct Roll {
    pub a: u32,
    pub b: u32,
    pub age: f32,
    pub applied: bool,
}

impl Roll {
    fn new((a, b): (u32, u32)) -> Roll {
        Roll {
            a,
            b,
            age: 0.0,
            applied: false,
        }
    }

    pub fn sum(&self) -> i32 {
        (self.a + self.b) as i32
    }

    /// (first die, second die, seconds since the roll started), as the HUD wants it.
    pub fn view(&self) -> (u32, u32, f32) {
        (self.a, self.b, self.age)
    }
}

/// The back-and-forth between the player and the skeleton.
///
/// An exchange is two rolls: whoever attacks first rolls, and when those dice land the other
/// side always rolls back. A reply never triggers another reply.
pub struct Combat {
    pub player_hp: i32,
    /// Health of whoever the current (or last) exchange is against; set by `begin`.
    pub enemy_hp: i32,
    /// The player's dice, shown at the top of the screen.
    pub player_roll: Option<Roll>,
    /// The skeleton's dice, shown next to the player's health bar.
    pub enemy_roll: Option<Roll>,
    /// Who still owes a reply once the current roll lands.
    owed: Option<Side>,
}

impl Combat {
    pub fn new() -> Combat {
        Combat {
            player_hp: MAX_PLAYER_HP,
            enemy_hp: 0,
            player_roll: None,
            enemy_roll: None,
            owed: None,
        }
    }

    fn slot(&mut self, side: Side) -> &mut Option<Roll> {
        match side {
            Side::Player => &mut self.player_roll,
            Side::Enemy => &mut self.enemy_roll,
        }
    }

    pub fn both_alive(&self) -> bool {
        self.player_hp > 0 && self.enemy_hp > 0
    }

    /// True while dice are still in the air or a reply is still to come.
    pub fn busy(&self) -> bool {
        self.owed.is_some()
            || [self.player_roll, self.enemy_roll]
                .iter()
                .flatten()
                .any(|r| !r.applied)
    }

    /// Starts an exchange against an opponent with `opponent_hp`. Call `resolve_now` first if
    /// one is already in progress.
    pub fn begin(&mut self, initiator: Side, opponent_hp: i32, dice: &mut Dice) {
        self.enemy_hp = opponent_hp;
        if !self.both_alive() {
            return;
        }
        *self.slot(initiator) = Some(Roll::new(dice.roll_2d6()));
        self.owed = Some(initiator.other());
    }

    /// Applies a roll's damage; if it was the opening roll, the other side replies.
    fn land(&mut self, side: Side, dice: &mut Dice) {
        let Some(roll) = self.slot(side).as_mut() else {
            return;
        };
        roll.applied = true;
        let damage = roll.sum();
        match side {
            Side::Player => self.enemy_hp = (self.enemy_hp - damage).max(0),
            Side::Enemy => self.player_hp = (self.player_hp - damage).max(0),
        }
        if self.owed == Some(side.other()) {
            self.owed = None;
            if self.both_alive() {
                *self.slot(side.other()) = Some(Roll::new(dice.roll_2d6()));
            }
        }
    }

    pub fn update(&mut self, dt: f32, dice: &mut Dice) {
        for side in [Side::Player, Side::Enemy] {
            if let Some(roll) = self.slot(side) {
                roll.age += dt;
            }
        }
        for side in [Side::Player, Side::Enemy] {
            let ready = self
                .slot(side)
                .is_some_and(|r| !r.applied && r.age >= ROLL_ANIM_SECONDS);
            if ready {
                self.land(side, dice);
            }
        }
        for side in [Side::Player, Side::Enemy] {
            let expired = self
                .slot(side)
                .is_some_and(|r| r.applied && r.age > ROLL_DISPLAY_SECONDS);
            if expired {
                *self.slot(side) = None;
            }
        }
    }

    /// Skips the animations: lands everything in flight, replies included, right now.
    pub fn resolve_now(&mut self, dice: &mut Dice) {
        // Opening roll, then its reply: two passes cover it, a third is just slack.
        for _ in 0..3 {
            for side in [Side::Player, Side::Enemy] {
                let pending = self.slot(side).is_some_and(|r| !r.applied);
                if pending {
                    if let Some(roll) = self.slot(side) {
                        roll.age = ROLL_ANIM_SECONDS;
                    }
                    self.land(side, dice);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(c: &mut Combat, dice: &mut Dice, seconds: f32) {
        // Small steps, like real frames.
        let mut t = 0.0;
        while t < seconds {
            c.update(0.01, dice);
            t += 0.01;
        }
    }

    #[test]
    fn player_attack_is_answered_after_the_dice_land() {
        let mut dice = Dice::with_seed(7);
        let mut c = Combat::new();
        c.begin(Side::Player, 1000, &mut dice); // survives anything
        assert!(c.busy());
        step(&mut c, &mut dice, ROLL_ANIM_SECONDS - 0.2);
        assert_eq!((c.enemy_hp, c.player_hp), (1000, MAX_PLAYER_HP), "nothing lands early");
        assert!(c.enemy_roll.is_none());

        step(&mut c, &mut dice, 0.4);
        let mine = c.player_roll.unwrap();
        assert_eq!(c.enemy_hp, 1000 - mine.sum());
        assert!(c.enemy_roll.is_some(), "he rolls back as mine land");
        assert_eq!(c.player_hp, MAX_PLAYER_HP);

        step(&mut c, &mut dice, ROLL_ANIM_SECONDS);
        let his = c.enemy_roll.unwrap();
        assert_eq!(c.player_hp, MAX_PLAYER_HP - his.sum());
        assert!(!c.busy());
    }

    #[test]
    fn enemy_attack_is_answered_by_the_player() {
        let mut dice = Dice::with_seed(9);
        let mut c = Combat::new();
        c.begin(Side::Enemy, 1000, &mut dice);
        step(&mut c, &mut dice, ROLL_ANIM_SECONDS + 0.1);
        let his = c.enemy_roll.unwrap();
        assert_eq!(c.player_hp, MAX_PLAYER_HP - his.sum());
        assert!(c.player_roll.is_some(), "I auto-attack back");
        step(&mut c, &mut dice, ROLL_ANIM_SECONDS);
        let mine = c.player_roll.unwrap();
        assert_eq!(c.enemy_hp, 1000 - mine.sum());
        assert!(!c.busy());
    }

    #[test]
    fn replies_do_not_chain() {
        let mut dice = Dice::with_seed(3);
        let mut c = Combat::new();
        c.begin(Side::Player, 1000, &mut dice);
        // Long enough for both rolls to land, short enough that they are still on display.
        step(&mut c, &mut dice, 2.0 * ROLL_ANIM_SECONDS + 0.2);
        assert!(!c.busy());
        assert_eq!(MAX_PLAYER_HP - c.player_hp, c.enemy_roll.unwrap().sum());
        // Nothing more happens afterwards: no third roll, no more damage.
        let (player_hp, enemy_hp) = (c.player_hp, c.enemy_hp);
        step(&mut c, &mut dice, 10.0);
        assert!(!c.busy());
        assert_eq!((c.player_hp, c.enemy_hp), (player_hp, enemy_hp));
    }

    #[test]
    fn resolve_now_finishes_the_whole_exchange_instantly() {
        let mut dice = Dice::with_seed(11);
        let mut c = Combat::new();
        c.begin(Side::Player, 1000, &mut dice);
        step(&mut c, &mut dice, 0.5);
        c.resolve_now(&mut dice);
        assert!(!c.busy());
        let (mine, his) = (c.player_roll.unwrap(), c.enemy_roll.unwrap());
        assert_eq!(c.enemy_hp, 1000 - mine.sum());
        assert_eq!(c.player_hp, MAX_PLAYER_HP - his.sum());
    }

    #[test]
    fn dead_enemy_does_not_reply() {
        let mut dice = Dice::with_seed(5);
        let mut c = Combat::new();
        c.begin(Side::Player, 1, &mut dice);
        step(&mut c, &mut dice, ROLL_ANIM_SECONDS + 0.1);
        assert_eq!(c.enemy_hp, 0);
        assert!(c.enemy_roll.is_none());
        assert!(!c.busy());
        assert_eq!(c.player_hp, MAX_PLAYER_HP);
    }

    #[test]
    fn hp_never_goes_below_zero() {
        let mut dice = Dice::with_seed(2);
        let mut c = Combat::new();
        c.player_hp = 1;
        c.begin(Side::Enemy, 12, &mut dice);
        c.resolve_now(&mut dice);
        assert_eq!(c.player_hp, 0);
        assert!(c.player_roll.is_none(), "dead players do not strike back");
    }
}
