//! Experience point and level management.
//!
//! Implements the NetHack 3.7 experience system from `exper.c` and
//! `attrib.c`:
//! - XP-to-level threshold table (`newuexp`)
//! - Monster kill XP calculation (`experience`)
//! - Level gain (`pluslvl`) and level loss (`losexp`)
//! - HP/PW advancement per level (`newhp`, `newpw`)
//! - Constitution-based HP bonus
//! - Role-based energy modifier (`enermod`)
//!
//! All functions are pure: they operate on `GameWorld` plus an RNG,
//! mutate world state, and return `Vec<EngineEvent>`.  Zero IO.

use hecs::Entity;
use rand::Rng;

use crate::event::{DeathCause, EngineEvent, HpSource};
use crate::role::Role;
use crate::world::{Attributes, ExperienceLevel, GameWorld, HitPoints, Power};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum player level (matches MAXULEV in NetHack).
pub const MAX_LEVEL: u8 = 30;

// ---------------------------------------------------------------------------
// XP threshold table
// ---------------------------------------------------------------------------

/// Experience points needed to reach a given level.
///
/// Matches `newuexp()` from `exper.c`:
/// - Levels 1-9:  `10 * 2^level`
/// - Levels 10-19: `10000 * 2^(level-10)`
/// - Levels 20-30: `10000000 * (level - 19)`
pub fn newuexp(level: u8) -> i64 {
    if level < 1 {
        return 0;
    }
    if level < 10 {
        10 * (1i64 << level)
    } else if level < 20 {
        10_000 * (1i64 << (level - 10))
    } else {
        10_000_000 * (level as i64 - 19)
    }
}

// ---------------------------------------------------------------------------
// Monster XP calculation
// ---------------------------------------------------------------------------

/// Parameters for calculating XP from a monster kill.
pub struct MonsterXpParams {
    /// Monster's level (`m_lev`).
    pub monster_level: u8,
    /// Monster's AC (lower = harder to hit = more XP).
    pub monster_ac: i32,
    /// Monster's base speed.
    pub monster_speed: u32,
    /// Number of special attacks (attacks with method > AT_BUTT).
    pub special_attack_count: u32,
    /// Number of weapon attacks (AT_WEAP).
    pub weapon_attack_count: u32,
    /// Number of magic attacks (AT_MAGC).
    pub magic_attack_count: u32,
    /// Number of special damage types (AD_xxx > AD_PHYS and < AD_BLND).
    pub special_damage_count: u32,
    /// Number of very dangerous damage types (drain life, stoning, sliming).
    pub dangerous_damage_count: u32,
    /// Number of other non-physical damage types.
    pub other_damage_count: u32,
    /// Whether any attack has high damage (damn * damd > 23).
    pub high_damage_attacks: u32,
    /// Whether the monster is "extra nasty" (genocidable + strong/regen/etc).
    pub extra_nasty: bool,
    /// Whether the monster was revived or cloned.
    pub revived_or_cloned: bool,
    /// Kill count for this monster type (for diminishing returns).
    pub kill_count: u32,
}

/// Calculate XP reward for killing a monster.
///
/// Matches `experience()` from `exper.c`.
pub fn monster_xp(params: &MonsterXpParams) -> i32 {
    let mlev = params.monster_level as i32;
    let mut tmp = 1 + mlev * mlev;

    // Extra XP for good AC.
    if params.monster_ac < 3 {
        let bonus = 7 - params.monster_ac;
        tmp += bonus * if params.monster_ac < 0 { 2 } else { 1 };
    }

    // Extra XP for fast monsters.
    if params.monster_speed > 12 {
        tmp += if params.monster_speed > 18 { 5 } else { 3 };
    }

    // Special attack methods.
    tmp += params.special_attack_count as i32 * 3;
    tmp += params.weapon_attack_count as i32 * 5;
    tmp += params.magic_attack_count as i32 * 10;

    // Special damage types.
    tmp += params.special_damage_count as i32 * 2 * mlev;
    tmp += params.dangerous_damage_count as i32 * 50;
    tmp += params.other_damage_count as i32 * mlev;
    tmp += params.high_damage_attacks as i32 * mlev;

    // Extra nasty bonus.
    if params.extra_nasty {
        tmp += 7 * mlev;
    }

    // High level bonus.
    if mlev > 8 {
        tmp += 50;
    }

    // Diminishing returns for revived/cloned monsters.
    if params.revived_or_cloned {
        let mut nk = params.kill_count;
        let mut xp = tmp;
        let mut bracket_size = 20u32;
        let mut i = 0u32;
        while nk > bracket_size && xp > 1 {
            xp = (xp + 1) / 2;
            nk -= bracket_size;
            if i & 1 != 0 {
                bracket_size += 20;
            }
            i += 1;
        }
        tmp = xp;
    }

    tmp.max(1)
}

// ---------------------------------------------------------------------------
// Constitution-based HP bonus
// ---------------------------------------------------------------------------

/// HP bonus from constitution, applied per level-up.
///
/// Matches the CON table in `newhp()` from `attrib.c`.
pub fn con_hp_bonus(con: u8) -> i32 {
    if con <= 3 {
        -2
    } else if con <= 6 {
        -1
    } else if con <= 14 {
        0
    } else if con <= 16 {
        1
    } else if con == 17 {
        2
    } else if con == 18 {
        3
    } else {
        4 // 19+ (gauntlets of power, etc.)
    }
}

// ---------------------------------------------------------------------------
// Role-based energy modifier
// ---------------------------------------------------------------------------

/// Modify energy gain based on role.
///
/// Matches `enermod()` from `exper.c`.
pub fn enermod(en: i32, role: Role) -> i32 {
    match role {
        Role::Priest | Role::Wizard => 2 * en,
        Role::Healer | Role::Knight => (3 * en) / 2,
        Role::Barbarian | Role::Valkyrie => (3 * en) / 4,
        _ => en,
    }
}

// ---------------------------------------------------------------------------
// HP gain per level
// ---------------------------------------------------------------------------

/// Calculate HP gain for a level-up.
///
/// `xlev` is the role's xlev threshold.  Below xlev, use low-level
/// advancement dice; at or above, use high-level dice.
///
/// Simplified from `newhp()` in `attrib.c`: uses fixed advancement
/// values per role tier (3+d8 below xlev, 2+d6 above) plus CON bonus.
pub fn newhp(level: u8, xlev: u8, con: u8, rng: &mut impl Rng) -> i32 {
    let base = if level < xlev {
        // Low level: rnd(8) + 3 (typical role advancement)
        rng.random_range(1..=8) + 3
    } else {
        // High level: rnd(6) + 2
        rng.random_range(1..=6) + 2
    };
    let hp = base + con_hp_bonus(con);
    hp.max(1)
}

/// Calculate PW (energy/mana) gain for a level-up.
///
/// Simplified from `newpw()` in `exper.c`.
pub fn newpw(level: u8, xlev: u8, wis: u8, role: Role, rng: &mut impl Rng) -> i32 {
    let enrnd = (wis as i32) / 2;
    let (lornd, lofix) = if level < xlev {
        // Low level: rn1(enrnd + 2, 2) typical
        (enrnd + 2, 2)
    } else {
        // High level: rn1(enrnd + 1, 1)
        (enrnd + 1, 1)
    };
    let raw = if lornd > 0 {
        rng.random_range(0..lornd) + lofix
    } else {
        lofix
    };
    let en = enermod(raw, role);
    en.max(1)
}

// ---------------------------------------------------------------------------
// Level up
// ---------------------------------------------------------------------------

/// Gain one experience level.
///
/// Increases level, HP, PW, and emits appropriate events.
/// Matches `pluslvl()` from `exper.c`.
///
/// `incremental`: true for XP-based level gain, false for potion/wraith.
pub fn pluslvl(
    world: &mut GameWorld,
    entity: Entity,
    incremental: bool,
    role: Role,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let (old_level, xlev) = {
        let lvl = match world.get_component::<ExperienceLevel>(entity) {
            Some(l) => l.0,
            None => return events,
        };
        let rd = crate::role::role_data(role);
        (lvl, rd.xlev)
    };

    if old_level >= MAX_LEVEL {
        return events;
    }

    // Calculate HP gain.
    let con = world
        .get_component::<Attributes>(entity)
        .map(|a| a.constitution)
        .unwrap_or(10);
    let hp_gain = newhp(old_level, xlev, con, rng);

    // Apply HP gain.
    if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
        hp.current += hp_gain;
        hp.max += hp_gain;
        events.push(EngineEvent::HpChange {
            entity,
            amount: hp_gain,
            new_hp: hp.current,
            source: HpSource::Other,
        });
    }

    // Calculate PW gain.
    let wis = world
        .get_component::<Attributes>(entity)
        .map(|a| a.wisdom)
        .unwrap_or(10);
    let pw_gain = newpw(old_level, xlev, wis, role, rng);

    // Apply PW gain.
    if let Some(mut pw) = world.get_component_mut::<Power>(entity) {
        pw.current += pw_gain;
        pw.max += pw_gain;
        events.push(EngineEvent::PwChange {
            entity,
            amount: pw_gain,
            new_pw: pw.current,
        });
    }

    // Increment level.
    let new_level = old_level + 1;
    if let Some(mut lvl) = world.get_component_mut::<ExperienceLevel>(entity) {
        lvl.0 = new_level;
    }

    events.push(EngineEvent::LevelUp { entity, new_level });

    if incremental {
        events.push(EngineEvent::msg_with(
            "exper-levelup",
            vec![("level", new_level.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg_with(
            "exper-levelup-feel",
            vec![("level", new_level.to_string())],
        ));
    }

    // Check for title change.
    let old_title = crate::role::role_title(role, old_level);
    let new_title = crate::role::role_title(role, new_level);
    if old_title != new_title {
        events.push(EngineEvent::msg_with(
            "exper-new-title",
            vec![("title", new_title.to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Level down
// ---------------------------------------------------------------------------

/// Lose one experience level.
///
/// Decreases level, HP, PW, and may be fatal at level 1.
/// Matches `losexp()` from `exper.c`.
///
/// `drainer`: if Some, the name of the entity causing the drain.
/// Level drain at level 1 with a drainer is fatal.
pub fn losexp(world: &mut GameWorld, entity: Entity, drainer: Option<&str>) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let current_level = match world.get_component::<ExperienceLevel>(entity) {
        Some(l) => l.0,
        None => return events,
    };

    if current_level > 1 {
        // Lose one level.
        let new_level = current_level - 1;
        if let Some(mut lvl) = world.get_component_mut::<ExperienceLevel>(entity) {
            lvl.0 = new_level;
        }

        events.push(EngineEvent::msg_with(
            "exper-leveldown",
            vec![("level", current_level.to_string())],
        ));

        // Lose some HP (simplified: lose 1d8 max HP).
        let hp_loss = 5; // Simplified average
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(entity) {
            hp.max = (hp.max - hp_loss).max(1);
            hp.current = hp.current.min(hp.max).max(1);
            events.push(EngineEvent::HpChange {
                entity,
                amount: -hp_loss,
                new_hp: hp.current,
                source: HpSource::Drain,
            });
        }

        // Lose some PW.
        let pw_loss = 3; // Simplified average
        if let Some(mut pw) = world.get_component_mut::<Power>(entity) {
            pw.max = (pw.max - pw_loss).max(0);
            pw.current = pw.current.min(pw.max).max(0);
            events.push(EngineEvent::PwChange {
                entity,
                amount: -pw_loss,
                new_pw: pw.current,
            });
        }
    } else {
        // Level 1: fatal if caused by a drainer.
        if let Some(killer_name) = drainer {
            events.push(EngineEvent::msg_with(
                "exper-leveldown",
                vec![("level", "1".to_string())],
            ));
            events.push(EngineEvent::EntityDied {
                entity,
                killer: None,
                cause: DeathCause::KilledBy {
                    killer_name: killer_name.to_string(),
                },
            });
        }
        // If no drainer (e.g., divine anger), just reset XP to 0.
    }

    events
}

// ---------------------------------------------------------------------------
// Check for level-up
// ---------------------------------------------------------------------------

/// Player experience state component.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Experience {
    /// Current experience points.
    pub xp: i64,
    /// Score-relevant experience (`u.urexp`).
    pub score_xp: i64,
}

/// Award experience points and check for level-up.
///
/// Matches `more_experienced()` + `newexplevel()` from `exper.c`.
pub fn gain_experience(
    world: &mut GameWorld,
    entity: Entity,
    xp: i32,
    role: Role,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Update XP counter.
    if let Some(mut exp) = world.get_component_mut::<Experience>(entity) {
        exp.xp = exp.xp.saturating_add(xp as i64);
        // Score XP: 4 * exp + rexp (simplified: just add 4x).
        exp.score_xp = exp.score_xp.saturating_add(4 * xp as i64);
    }

    // Check for level-up.
    let current_level = world
        .get_component::<ExperienceLevel>(entity)
        .map(|l| l.0)
        .unwrap_or(1);

    let current_xp = world
        .get_component::<Experience>(entity)
        .map(|e| e.xp)
        .unwrap_or(0);

    if current_level < MAX_LEVEL && current_xp >= newuexp(current_level) {
        let level_events = pluslvl(world, entity, true, role, rng);
        events.extend(level_events);
    }

    events
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::{ExperienceLevel, GameWorld, HitPoints, Power};
    use rand::SeedableRng;

    type TestRng = rand::rngs::SmallRng;

    fn make_test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    // --- Test 1: XP threshold table ---

    #[test]
    fn newuexp_table() {
        assert_eq!(newuexp(0), 0);
        assert_eq!(newuexp(1), 20); // 10 * 2^1 = 20
        assert_eq!(newuexp(2), 40); // 10 * 2^2 = 40
        assert_eq!(newuexp(5), 320); // 10 * 2^5 = 320
        assert_eq!(newuexp(9), 5120); // 10 * 2^9 = 5120
        assert_eq!(newuexp(10), 10_000); // 10000 * 2^0
        assert_eq!(newuexp(14), 160_000); // 10000 * 2^4
        assert_eq!(newuexp(19), 5_120_000);
        assert_eq!(newuexp(20), 10_000_000); // 10000000 * 1
        assert_eq!(newuexp(30), 110_000_000); // 10000000 * 11
    }

    // --- Test 2: XP thresholds are monotonically increasing ---

    #[test]
    fn newuexp_monotonic() {
        for level in 1..MAX_LEVEL {
            assert!(
                newuexp(level + 1) > newuexp(level),
                "newuexp({}) should be > newuexp({})",
                level + 1,
                level,
            );
        }
    }

    // --- Test 3: Monster XP calculation ---

    #[test]
    fn monster_xp_basic() {
        let params = MonsterXpParams {
            monster_level: 5,
            monster_ac: 5,
            monster_speed: 12,
            special_attack_count: 0,
            weapon_attack_count: 0,
            magic_attack_count: 0,
            special_damage_count: 0,
            dangerous_damage_count: 0,
            other_damage_count: 0,
            high_damage_attacks: 0,
            extra_nasty: false,
            revived_or_cloned: false,
            kill_count: 0,
        };
        let xp = monster_xp(&params);
        // 1 + 5*5 = 26
        assert_eq!(xp, 26);
    }

    // --- Test 4: Monster XP with low AC bonus ---

    #[test]
    fn monster_xp_low_ac() {
        let params = MonsterXpParams {
            monster_level: 5,
            monster_ac: -2,
            monster_speed: 12,
            special_attack_count: 0,
            weapon_attack_count: 0,
            magic_attack_count: 0,
            special_damage_count: 0,
            dangerous_damage_count: 0,
            other_damage_count: 0,
            high_damage_attacks: 0,
            extra_nasty: false,
            revived_or_cloned: false,
            kill_count: 0,
        };
        let xp = monster_xp(&params);
        // 1 + 25 = 26, plus AC bonus: (7 - (-2)) * 2 = 18
        assert_eq!(xp, 26 + 18);
    }

    // --- Test 5: Monster XP with dangerous attacks ---

    #[test]
    fn monster_xp_dangerous() {
        let params = MonsterXpParams {
            monster_level: 10,
            monster_ac: 5,
            monster_speed: 20,
            special_attack_count: 1,
            weapon_attack_count: 1,
            magic_attack_count: 0,
            special_damage_count: 1,
            dangerous_damage_count: 1,
            other_damage_count: 0,
            high_damage_attacks: 1,
            extra_nasty: true,
            revived_or_cloned: false,
            kill_count: 0,
        };
        let xp = monster_xp(&params);
        // base: 1 + 100 = 101
        // fast: +5
        // special attacks: 1*3 + 1*5 = 8
        // damage: 1*20 + 1*50 + 1*10 = 80
        // extra nasty: 70
        // high level: 50
        assert_eq!(xp, 101 + 5 + 8 + 80 + 70 + 50);
    }

    // --- Test 6: Diminishing returns for revived monsters ---

    #[test]
    fn monster_xp_diminishing() {
        let base_params = MonsterXpParams {
            monster_level: 5,
            monster_ac: 5,
            monster_speed: 12,
            special_attack_count: 0,
            weapon_attack_count: 0,
            magic_attack_count: 0,
            special_damage_count: 0,
            dangerous_damage_count: 0,
            other_damage_count: 0,
            high_damage_attacks: 0,
            extra_nasty: false,
            revived_or_cloned: false,
            kill_count: 0,
        };
        let full_xp = monster_xp(&base_params);

        let diminished = MonsterXpParams {
            revived_or_cloned: true,
            kill_count: 25, // Past first bracket of 20
            ..base_params
        };
        let reduced_xp = monster_xp(&diminished);
        assert!(reduced_xp < full_xp, "revived monster should give less XP");
    }

    // --- Test 7: CON HP bonus table ---

    #[test]
    fn con_hp_bonus_table() {
        assert_eq!(con_hp_bonus(3), -2);
        assert_eq!(con_hp_bonus(6), -1);
        assert_eq!(con_hp_bonus(10), 0);
        assert_eq!(con_hp_bonus(14), 0);
        assert_eq!(con_hp_bonus(16), 1);
        assert_eq!(con_hp_bonus(17), 2);
        assert_eq!(con_hp_bonus(18), 3);
        assert_eq!(con_hp_bonus(25), 4);
    }

    // --- Test 8: Energy modifier per role ---

    #[test]
    fn enermod_roles() {
        assert_eq!(enermod(10, Role::Wizard), 20);
        assert_eq!(enermod(10, Role::Priest), 20);
        assert_eq!(enermod(10, Role::Healer), 15);
        assert_eq!(enermod(10, Role::Knight), 15);
        assert_eq!(enermod(10, Role::Barbarian), 7);
        assert_eq!(enermod(10, Role::Valkyrie), 7);
        assert_eq!(enermod(10, Role::Rogue), 10);
    }

    // --- Test 9: Level up increases stats ---

    #[test]
    fn pluslvl_increases_stats() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        // Set player to level 1 with known stats.
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(player) {
            hp.current = 16;
            hp.max = 16;
        }
        if let Some(mut pw) = world.get_component_mut::<Power>(player) {
            pw.current = 4;
            pw.max = 4;
        }

        let events = pluslvl(&mut world, player, true, Role::Wizard, &mut rng);

        // Level should be 2.
        let new_level = world.get_component::<ExperienceLevel>(player).unwrap().0;
        assert_eq!(new_level, 2);

        // HP should have increased.
        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert!(hp.max > 16, "HP max should increase: got {}", hp.max);

        // PW should have increased.
        let pw = world.get_component::<Power>(player).unwrap();
        assert!(pw.max > 4, "PW max should increase: got {}", pw.max);

        // Should have LevelUp event.
        let has_levelup = events
            .iter()
            .any(|e| matches!(e, EngineEvent::LevelUp { new_level: 2, .. }));
        assert!(has_levelup, "should emit LevelUp event");
    }

    // --- Test 10: Level up capped at MAX_LEVEL ---

    #[test]
    fn pluslvl_capped_at_max() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        // Set player to max level.
        if let Some(mut lvl) = world.get_component_mut::<ExperienceLevel>(player) {
            lvl.0 = MAX_LEVEL;
        }

        let events = pluslvl(&mut world, player, true, Role::Wizard, &mut rng);
        assert!(events.is_empty(), "should not level up past max");

        let level = world.get_component::<ExperienceLevel>(player).unwrap().0;
        assert_eq!(level, MAX_LEVEL);
    }

    // --- Test 11: Level loss at level 5 ---

    #[test]
    fn losexp_at_level_5() {
        let mut world = make_test_world();
        let player = world.player();

        if let Some(mut lvl) = world.get_component_mut::<ExperienceLevel>(player) {
            lvl.0 = 5;
        }

        let _events = losexp(&mut world, player, Some("vampire"));

        let new_level = world.get_component::<ExperienceLevel>(player).unwrap().0;
        assert_eq!(new_level, 4, "should drop to level 4");

        // HP max should have decreased.
        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert!(hp.max < 16, "HP max should decrease");
    }

    // --- Test 12: Level loss at level 1 is fatal ---

    #[test]
    fn losexp_at_level_1_fatal() {
        let mut world = make_test_world();
        let player = world.player();

        // Already level 1.
        let events = losexp(&mut world, player, Some("vampire"));

        let died = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityDied { .. }));
        assert!(died, "losing level at 1 with drainer should be fatal");
    }

    // --- Test 13: Level loss at level 1 without drainer is not fatal ---

    #[test]
    fn losexp_at_level_1_no_drainer() {
        let mut world = make_test_world();
        let player = world.player();

        let events = losexp(&mut world, player, None);

        let died = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityDied { .. }));
        assert!(
            !died,
            "losing level at 1 without drainer should NOT be fatal"
        );
    }

    // --- Test 14: gain_experience triggers level-up ---

    #[test]
    fn gain_experience_levelup() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        // Add Experience component.
        let _ = world.ecs_mut().insert_one(player, Experience::default());

        // Give enough XP to reach level 2 (need 20).
        let events = gain_experience(&mut world, player, 25, Role::Wizard, &mut rng);

        let new_level = world.get_component::<ExperienceLevel>(player).unwrap().0;
        assert_eq!(new_level, 2);

        let has_levelup = events
            .iter()
            .any(|e| matches!(e, EngineEvent::LevelUp { .. }));
        assert!(has_levelup, "should emit LevelUp on sufficient XP");
    }

    // --- Test 15: gain_experience not enough for level-up ---

    #[test]
    fn gain_experience_no_levelup() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        let _ = world.ecs_mut().insert_one(player, Experience::default());

        // Give just 10 XP (need 20 for level 2).
        let events = gain_experience(&mut world, player, 10, Role::Wizard, &mut rng);

        let level = world.get_component::<ExperienceLevel>(player).unwrap().0;
        assert_eq!(level, 1);

        let has_levelup = events
            .iter()
            .any(|e| matches!(e, EngineEvent::LevelUp { .. }));
        assert!(!has_levelup, "should NOT level up with insufficient XP");
    }

    // --- Test 16: HP gain range is reasonable ---

    #[test]
    fn newhp_range() {
        let mut rng = TestRng::seed_from_u64(42);
        // Low level, CON 10 (bonus = 0).
        for _ in 0..1000 {
            let hp = newhp(1, 14, 10, &mut rng);
            // rnd(8) + 3 + 0 = [4, 11]
            assert!(hp >= 4 && hp <= 11, "hp {} out of range", hp);
        }
    }

    // --- Test 17: PW gain range is reasonable ---

    #[test]
    fn newpw_range() {
        let mut rng = TestRng::seed_from_u64(42);
        for _ in 0..1000 {
            let pw = newpw(1, 14, 10, Role::Wizard, &mut rng);
            // Wizard doubles: enermod(rn1(5+2, 2), Wizard) = 2*(rng(0..7)+2) = 2*[2,8] = [4,16]
            assert!(pw >= 1, "pw {} should be at least 1", pw);
            assert!(pw <= 20, "pw {} unreasonably high", pw);
        }
    }

    // --- Test 18: Title changes on level-up ---

    #[test]
    fn title_change_on_levelup() {
        let mut world = make_test_world();
        let player = world.player();
        let mut rng = TestRng::seed_from_u64(42);

        // Set to level 2 (still rank 0 for most roles).
        if let Some(mut lvl) = world.get_component_mut::<ExperienceLevel>(player) {
            lvl.0 = 2;
        }

        // Level up to 3 should trigger new rank for most roles.
        let events = pluslvl(&mut world, player, true, Role::Wizard, &mut rng);

        let has_title_msg = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key == "exper-new-title"
            )
        });
        assert!(has_title_msg, "leveling from 2 to 3 should change title");
    }
}
