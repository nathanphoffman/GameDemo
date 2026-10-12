use crate::enemy::Dice;

pub const MAX_PLAYER_HP: i32 = 50;

/// How long the dice tumble before they settle on their result.
pub const ROLL_ANIM_SECONDS: f32 = 1.5;
/// How long a finished roll stays on screen.
pub const ROLL_DISPLAY_SECONDS: f32 = 3.0;
/// The most creatures that can attack the player in one round (the HUD shows this many pairs).
pub const MAX_ATTACKERS: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Player,
    Enemy,
}

/// An enemy that is in range for this purpose, as the app sees it right now.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub id: u32,
    pub hp: i32,
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

/// The enemy the player's roll is against, and its health once that roll has landed.
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub id: u32,
    pub hp: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Idle,
    /// Every attacker's dice are tumbling together.
    Enemies,
    /// The player's single attack is tumbling.
    Player,
}

/// Fighting happens in rounds. In a round every hostile creature in its attack range rolls
/// against the player at once, and the player makes one attack against the weakest hostile
/// creature in the player's range. Which of those two happens first depends on who started
/// the round; each takes a moment to tumble and land before the next begins.
pub struct Combat {
    pub player_hp: i32,
    /// The player's dice, shown at the top of the screen.
    pub player_roll: Option<Roll>,
    /// Who the player's roll is against.
    pub target: Option<Target>,
    /// The creatures' dice, shown next to the player's health bar.
    pub enemy_rolls: Vec<Roll>,
    phase: Phase,
    /// The phase that follows the current one.
    then: Option<Phase>,
}

impl Combat {
    pub fn new() -> Combat {
        Combat {
            player_hp: MAX_PLAYER_HP,
            player_roll: None,
            target: None,
            enemy_rolls: Vec::new(),
            phase: Phase::Idle,
            then: None,
        }
    }

    /// True while a round is in progress.
    pub fn busy(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Starts a round. `first` is who strikes first. `attackers` are the hostile creatures
    /// within their attack range, `targets` the hostile ones within the player's.
    pub fn begin_round(
        &mut self,
        first: Side,
        attackers: &[Candidate],
        targets: &[Candidate],
        dice: &mut Dice,
    ) {
        self.player_roll = None;
        self.target = None;
        self.enemy_rolls.clear();
        self.phase = Phase::Idle;
        let (now, later) = match first {
            Side::Enemy => (Phase::Enemies, Phase::Player),
            Side::Player => (Phase::Player, Phase::Enemies),
        };
        self.then = Some(later);
        self.start(now, attackers, targets, dice);
    }

    /// Begins a phase's dice, or skips it if there is nobody to roll.
    fn start(&mut self, phase: Phase, attackers: &[Candidate], targets: &[Candidate], dice: &mut Dice) {
        match phase {
            Phase::Enemies => {
                let rolls: Vec<Roll> = attackers
                    .iter()
                    .filter(|c| c.hp > 0)
                    .take(MAX_ATTACKERS)
                    .map(|_| Roll::new(dice.roll_2d6()))
                    .collect();
                if rolls.is_empty() || self.player_hp <= 0 {
                    self.advance(attackers, targets, dice);
                } else {
                    self.enemy_rolls = rolls;
                    self.phase = Phase::Enemies;
                }
            }
            Phase::Player => {
                // The weakest one in range; the lower id breaks a tie.
                let weakest = targets
                    .iter()
                    .filter(|c| c.hp > 0)
                    .min_by_key(|c| (c.hp, c.id));
                match weakest {
                    Some(c) if self.player_hp > 0 => {
                        self.target = Some(Target { id: c.id, hp: c.hp });
                        self.player_roll = Some(Roll::new(dice.roll_2d6()));
                        self.phase = Phase::Player;
                    }
                    _ => self.advance(attackers, targets, dice),
                }
            }
            Phase::Idle => {}
        }
    }

    fn advance(&mut self, attackers: &[Candidate], targets: &[Candidate], dice: &mut Dice) {
        self.phase = Phase::Idle;
        if let Some(next) = self.then.take() {
            self.start(next, attackers, targets, dice);
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        dice: &mut Dice,
        attackers: &[Candidate],
        targets: &[Candidate],
    ) {
        if let Some(roll) = &mut self.player_roll {
            roll.age += dt;
        }
        for r in &mut self.enemy_rolls {
            r.age += dt;
        }

        match self.phase {
            Phase::Enemies => {
                let landed = self
                    .enemy_rolls
                    .iter()
                    .all(|r| r.age >= ROLL_ANIM_SECONDS);
                if landed {
                    for r in &mut self.enemy_rolls {
                        r.applied = true;
                        self.player_hp = (self.player_hp - r.sum()).max(0);
                    }
                    self.advance(attackers, targets, dice);
                }
            }
            Phase::Player => {
                let landed = self
                    .player_roll
                    .is_some_and(|r| r.age >= ROLL_ANIM_SECONDS);
                if landed {
                    if let (Some(roll), Some(target)) = (&mut self.player_roll, &mut self.target) {
                        roll.applied = true;
                        target.hp = (target.hp - roll.sum()).max(0);
                    }
                    self.advance(attackers, targets, dice);
                }
            }
            Phase::Idle => {}
        }

        // Old results leave the screen after a while.
        if self
            .player_roll
            .is_some_and(|r| r.applied && r.age > ROLL_DISPLAY_SECONDS)
        {
            self.player_roll = None;
        }
        self.enemy_rolls
            .retain(|r| !(r.applied && r.age > ROLL_DISPLAY_SECONDS));
    }

    /// Skips the animations: lands the rest of the round right now.
    pub fn resolve_now(&mut self, dice: &mut Dice, attackers: &[Candidate], targets: &[Candidate]) {
        // A round has at most two phases; the extra pass is just slack.
        for _ in 0..3 {
            if !self.busy() {
                break;
            }
            if let Some(roll) = &mut self.player_roll {
                if !roll.applied {
                    roll.age = ROLL_ANIM_SECONDS;
                }
            }
            for r in &mut self.enemy_rolls {
                if !r.applied {
                    r.age = ROLL_ANIM_SECONDS;
                }
            }
            self.update(0.0, dice, attackers, targets);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(id: u32, hp: i32) -> Candidate {
        Candidate { id, hp }
    }

    fn step(combat: &mut Combat, dice: &mut Dice, seconds: f32, a: &[Candidate], t: &[Candidate]) {
        let mut elapsed = 0.0;
        while elapsed < seconds {
            combat.update(0.01, dice, a, t);
            elapsed += 0.01;
        }
    }

    #[test]
    fn two_attackers_hit_together_and_i_get_one_attack_on_the_weakest() {
        let mut dice = Dice::with_seed(7);
        let mut combat = Combat::new();
        let both = [c(1, 12), c(2, 5)];
        combat.begin_round(Side::Enemy, &both, &both, &mut dice);
        assert_eq!(combat.enemy_rolls.len(), 2, "both attack");
        assert!(combat.player_roll.is_none());

        // Nothing lands early.
        step(&mut combat, &mut dice, ROLL_ANIM_SECONDS - 0.2, &both, &both);
        assert_eq!(combat.player_hp, MAX_PLAYER_HP);

        // Both land together, then my one attack starts, against the weaker (id 2).
        step(&mut combat, &mut dice, 0.4, &both, &both);
        let taken: i32 = combat.enemy_rolls.iter().map(|r| r.sum()).sum();
        assert_eq!(combat.player_hp, MAX_PLAYER_HP - taken);
        let target = combat.target.expect("I attack");
        assert_eq!(target.id, 2);
        assert!(combat.player_roll.is_some());

        step(&mut combat, &mut dice, ROLL_ANIM_SECONDS, &both, &both);
        assert_eq!(combat.target.unwrap().hp, (5 - combat.player_roll.unwrap().sum()).max(0));
        assert!(!combat.busy());
    }

    #[test]
    fn player_first_rounds_swap_the_order() {
        let mut dice = Dice::with_seed(9);
        let mut combat = Combat::new();
        let list = [c(3, 20)];
        combat.begin_round(Side::Player, &list, &list, &mut dice);
        assert!(combat.player_roll.is_some());
        assert!(combat.enemy_rolls.is_empty());
        step(&mut combat, &mut dice, ROLL_ANIM_SECONDS + 0.1, &list, &list);
        assert_eq!(combat.enemy_rolls.len(), 1, "he answers once mine land");
        assert_eq!(combat.player_hp, MAX_PLAYER_HP);
        step(&mut combat, &mut dice, ROLL_ANIM_SECONDS, &list, &list);
        assert!(combat.player_hp < MAX_PLAYER_HP);
        assert!(!combat.busy());
    }

    #[test]
    fn a_round_is_exactly_one_roll_each_and_then_quiet() {
        let mut dice = Dice::with_seed(3);
        let mut combat = Combat::new();
        let list = [c(1, 1000)];
        combat.begin_round(Side::Enemy, &list, &list, &mut dice);
        step(&mut combat, &mut dice, 2.0 * ROLL_ANIM_SECONDS + 0.2, &list, &list);
        assert!(!combat.busy());
        let (player_hp, target_hp) = (combat.player_hp, combat.target.unwrap().hp);
        step(&mut combat, &mut dice, 10.0, &list, &list);
        assert_eq!((combat.player_hp, combat.target.map(|t| t.hp)), (player_hp, Some(target_hp)));
    }

    #[test]
    fn resolve_now_finishes_the_round_instantly() {
        let mut dice = Dice::with_seed(11);
        let mut combat = Combat::new();
        let both = [c(1, 1000), c(2, 1000)];
        combat.begin_round(Side::Enemy, &both, &both, &mut dice);
        step(&mut combat, &mut dice, 0.5, &both, &both);
        combat.resolve_now(&mut dice, &both, &both);
        assert!(!combat.busy());
        let taken: i32 = combat.enemy_rolls.iter().map(|r| r.sum()).sum();
        assert_eq!(combat.player_hp, MAX_PLAYER_HP - taken);
        let (roll, target) = (combat.player_roll.unwrap(), combat.target.unwrap());
        assert_eq!(target.hp, 1000 - roll.sum());
    }

    #[test]
    fn nobody_in_my_range_means_no_attack_from_me() {
        let mut dice = Dice::with_seed(5);
        let mut combat = Combat::new();
        combat.begin_round(Side::Enemy, &[c(1, 10)], &[], &mut dice);
        step(&mut combat, &mut dice, 2.0 * ROLL_ANIM_SECONDS, &[c(1, 10)], &[]);
        assert!(combat.player_roll.is_none());
        assert!(!combat.busy());
        assert!(combat.player_hp < MAX_PLAYER_HP);
    }

    #[test]
    fn dead_players_do_not_strike_back_and_hp_stays_at_zero() {
        let mut dice = Dice::with_seed(2);
        let mut combat = Combat::new();
        combat.player_hp = 1;
        let list = [c(1, 12)];
        combat.begin_round(Side::Enemy, &list, &list, &mut dice);
        combat.resolve_now(&mut dice, &list, &list);
        assert_eq!(combat.player_hp, 0);
        assert!(combat.player_roll.is_none());
        assert!(!combat.busy());
    }

    #[test]
    fn killing_my_target_leaves_it_at_zero_and_the_round_still_ends() {
        let mut dice = Dice::with_seed(4);
        let mut combat = Combat::new();
        let list = [c(1, 1)];
        combat.begin_round(Side::Player, &list, &list, &mut dice);
        combat.resolve_now(&mut dice, &list, &list);
        assert_eq!(combat.target.unwrap().hp, 0);
        assert!(!combat.busy());
    }

    #[test]
    fn ties_go_to_the_lower_id_and_the_dead_are_skipped() {
        let mut dice = Dice::with_seed(6);
        let mut combat = Combat::new();
        let list = [c(9, 4), c(4, 4), c(7, 0)];
        combat.begin_round(Side::Player, &[], &list, &mut dice);
        assert_eq!(combat.target.unwrap().id, 4);
    }

    #[test]
    fn at_most_four_attack_at_once() {
        let mut dice = Dice::with_seed(8);
        let mut combat = Combat::new();
        let crowd: Vec<Candidate> = (1..=7).map(|id| c(id, 10)).collect();
        combat.begin_round(Side::Enemy, &crowd, &crowd, &mut dice);
        assert_eq!(combat.enemy_rolls.len(), MAX_ATTACKERS);
    }
}
