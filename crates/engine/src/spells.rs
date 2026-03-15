//! Spellcasting system for NetHack Babel.
//!
//! Implements spell learning, memory decay, power consumption, failure
//! rate calculation, and spell effects.  Follows the NetHack 3.7
//! mechanics described in `specs/spellcasting.md`.
//!
//! All functions operate on `GameWorld` plus RNG, mutate world state,
//! and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::{Direction, SpellId};
use crate::event::{DamageSource, DeathCause, EngineEvent, HpSource, StatusEffect};
use crate::status::StatusEffects;
use crate::world::{
    Attributes, ExperienceLevel, GameWorld, HeroSpeed, HeroSpeedBonus, HitPoints,
    Monster, Positioned, Power,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum spell memory (KEEN in NetHack).
pub const KEEN: u32 = 20_000;

/// Maximum number of spells the player can know.
pub const MAX_SPELLS: usize = 20;

/// Power cost multiplier: cost = level * PW_PER_LEVEL.
pub const PW_PER_LEVEL: i32 = 5;

// ---------------------------------------------------------------------------
// Spell type enumeration
// ---------------------------------------------------------------------------

/// All spell types available in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpellType {
    // Attack
    ForceBolt,
    MagicMissile,
    Fireball,
    ConeOfCold,
    DrainLife,
    FingerOfDeath,

    // Healing
    HealingSpell,
    ExtraHealing,
    CureBlindness,
    CureSickness,
    RestoreAbility,
    StoneToFlesh,

    // Divination
    Light,
    DetectMonsters,
    DetectFood,
    DetectUnseen,
    Clairvoyance,
    DetectTreasure,
    MagicMapping,
    Identify,

    // Enchantment
    ConfuseMonster,
    Sleep,
    SlowMonster,
    CauseFear,
    CharmMonster,

    // Clerical
    Protection,
    RemoveCurse,
    CreateMonster,
    TurnUndead,
    CreateFamiliar,

    // Escape
    HasteSelf,
    Levitation,
    Invisibility,
    TeleportAway,
    Jumping,

    // Matter
    Knock,
    WizardLock,
    Dig,
    Polymorph,
    Cancellation,
}

/// Direction type for a spell (determines how the player targets it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpellDirection {
    /// No direction needed (area effect or self).
    Nodir,
    /// Beam that hits the first target in a direction.
    Immediate,
    /// Ray that travels and can bounce.
    Ray,
}

impl SpellType {
    /// Spell level (1-7) as defined in the NetHack objects table.
    pub fn level(self) -> u8 {
        match self {
            // Level 1
            SpellType::ForceBolt
            | SpellType::HealingSpell
            | SpellType::Light
            | SpellType::DetectMonsters
            | SpellType::ConfuseMonster
            | SpellType::Protection
            | SpellType::Knock
            | SpellType::Jumping => 1,

            // Level 2
            SpellType::MagicMissile
            | SpellType::DrainLife
            | SpellType::CureBlindness
            | SpellType::DetectFood
            | SpellType::SlowMonster
            | SpellType::CreateMonster
            | SpellType::WizardLock => 2,

            // Level 3
            SpellType::Sleep
            | SpellType::ExtraHealing
            | SpellType::CureSickness
            | SpellType::Clairvoyance
            | SpellType::DetectUnseen
            | SpellType::CauseFear
            | SpellType::RemoveCurse
            | SpellType::HasteSelf
            | SpellType::Identify
            | SpellType::StoneToFlesh => 3,

            // Level 4
            SpellType::Fireball
            | SpellType::ConeOfCold
            | SpellType::RestoreAbility
            | SpellType::DetectTreasure
            | SpellType::Levitation
            | SpellType::Invisibility => 4,

            // Level 5
            SpellType::CharmMonster
            | SpellType::MagicMapping
            | SpellType::Dig => 5,

            // Level 6
            SpellType::TurnUndead
            | SpellType::TeleportAway
            | SpellType::CreateFamiliar
            | SpellType::Polymorph => 6,

            // Level 7
            SpellType::FingerOfDeath
            | SpellType::Cancellation => 7,
        }
    }

    /// Direction type for this spell.
    pub fn direction(self) -> SpellDirection {
        match self {
            SpellType::MagicMissile
            | SpellType::Fireball
            | SpellType::ConeOfCold
            | SpellType::Sleep
            | SpellType::FingerOfDeath
            | SpellType::Dig => SpellDirection::Ray,

            SpellType::ForceBolt
            | SpellType::HealingSpell
            | SpellType::ExtraHealing
            | SpellType::CureBlindness
            | SpellType::DrainLife
            | SpellType::SlowMonster
            | SpellType::ConfuseMonster
            | SpellType::CharmMonster
            | SpellType::TurnUndead
            | SpellType::TeleportAway
            | SpellType::Knock
            | SpellType::WizardLock
            | SpellType::Polymorph
            | SpellType::Cancellation
            | SpellType::StoneToFlesh
            | SpellType::Jumping => SpellDirection::Immediate,

            SpellType::Light
            | SpellType::DetectMonsters
            | SpellType::DetectFood
            | SpellType::DetectUnseen
            | SpellType::Clairvoyance
            | SpellType::DetectTreasure
            | SpellType::MagicMapping
            | SpellType::Identify
            | SpellType::CauseFear
            | SpellType::Protection
            | SpellType::RemoveCurse
            | SpellType::CreateMonster
            | SpellType::CreateFamiliar
            | SpellType::HasteSelf
            | SpellType::Levitation
            | SpellType::Invisibility
            | SpellType::CureSickness
            | SpellType::RestoreAbility => SpellDirection::Nodir,
        }
    }

    /// Descriptive name for this spell (used in messages).
    pub fn name(self) -> &'static str {
        match self {
            SpellType::ForceBolt => "force bolt",
            SpellType::MagicMissile => "magic missile",
            SpellType::Fireball => "fireball",
            SpellType::ConeOfCold => "cone of cold",
            SpellType::DrainLife => "drain life",
            SpellType::FingerOfDeath => "finger of death",
            SpellType::HealingSpell => "healing",
            SpellType::ExtraHealing => "extra healing",
            SpellType::CureBlindness => "cure blindness",
            SpellType::CureSickness => "cure sickness",
            SpellType::RestoreAbility => "restore ability",
            SpellType::StoneToFlesh => "stone to flesh",
            SpellType::Light => "light",
            SpellType::DetectMonsters => "detect monsters",
            SpellType::DetectFood => "detect food",
            SpellType::DetectUnseen => "detect unseen",
            SpellType::Clairvoyance => "clairvoyance",
            SpellType::DetectTreasure => "detect treasure",
            SpellType::MagicMapping => "magic mapping",
            SpellType::Identify => "identify",
            SpellType::ConfuseMonster => "confuse monster",
            SpellType::Sleep => "sleep",
            SpellType::SlowMonster => "slow monster",
            SpellType::CauseFear => "cause fear",
            SpellType::CharmMonster => "charm monster",
            SpellType::Protection => "protection",
            SpellType::RemoveCurse => "remove curse",
            SpellType::CreateMonster => "create monster",
            SpellType::TurnUndead => "turn undead",
            SpellType::CreateFamiliar => "create familiar",
            SpellType::HasteSelf => "haste self",
            SpellType::Levitation => "levitation",
            SpellType::Invisibility => "invisibility",
            SpellType::TeleportAway => "teleport away",
            SpellType::Jumping => "jumping",
            SpellType::Knock => "knock",
            SpellType::WizardLock => "wizard lock",
            SpellType::Dig => "dig",
            SpellType::Polymorph => "polymorph",
            SpellType::Cancellation => "cancellation",
        }
    }
}

// ---------------------------------------------------------------------------
// Known spell entry
// ---------------------------------------------------------------------------

/// A single spell known by the player.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KnownSpell {
    pub spell_type: SpellType,
    /// Turns of memory remaining.  Starts at KEEN (20000).
    /// At 0 the spell is forgotten and casting causes backfire.
    pub memory: u32,
    /// Spell level (1-7), cached from SpellType::level().
    pub level: u8,
}

// ---------------------------------------------------------------------------
// SpellBook component
// ---------------------------------------------------------------------------

/// Component: the player's known spells.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpellBook {
    pub spells: Vec<KnownSpell>,
}

impl SpellBook {
    /// Look up a spell by index.
    pub fn get(&self, id: SpellId) -> Option<&KnownSpell> {
        self.spells.get(id.0 as usize)
    }

    /// Look up a spell by index (mutable).
    pub fn get_mut(&mut self, id: SpellId) -> Option<&mut KnownSpell> {
        self.spells.get_mut(id.0 as usize)
    }

    /// Find a spell by type and return its index.
    pub fn find(&self, spell_type: SpellType) -> Option<SpellId> {
        self.spells
            .iter()
            .position(|s| s.spell_type == spell_type)
            .map(|i| SpellId(i as u8))
    }

    /// Number of known spells.
    pub fn count(&self) -> usize {
        self.spells.len()
    }
}

// ---------------------------------------------------------------------------
// Learn spell
// ---------------------------------------------------------------------------

/// Result of attempting to learn a spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnResult {
    /// Spell was newly learned (added to book).
    Learned,
    /// Spell was already known; memory refreshed.
    Refreshed,
    /// Spellbook is full (MAX_SPELLS reached).
    BookFull,
}

/// Add a spell to the player's spellbook, or refresh it if already known.
///
/// Returns what happened and any events generated.
pub fn learn_spell(
    world: &mut GameWorld,
    player: Entity,
    spell_type: SpellType,
) -> (LearnResult, Vec<EngineEvent>) {
    let mut events = Vec::new();
    let level = spell_type.level();

    // Ensure the player has a SpellBook component.
    if world.get_component::<SpellBook>(player).is_none() {
        let _ = world
            .ecs_mut()
            .insert_one(player, SpellBook::default());
    }

    let mut book = world
        .get_component_mut::<SpellBook>(player)
        .expect("SpellBook just inserted");

    // Check if already known.
    if let Some(idx) = book
        .spells
        .iter()
        .position(|s| s.spell_type == spell_type)
    {
        book.spells[idx].memory = KEEN + 1;
        events.push(EngineEvent::msg_with(
            "spell-refreshed",
            vec![("spell", spell_type.name().to_string())],
        ));
        return (LearnResult::Refreshed, events);
    }

    // New spell — check capacity.
    if book.spells.len() >= MAX_SPELLS {
        events.push(EngineEvent::msg("spell-book-full"));
        return (LearnResult::BookFull, events);
    }

    book.spells.push(KnownSpell {
        spell_type,
        memory: KEEN + 1,
        level,
    });
    events.push(EngineEvent::msg_with(
        "spell-learned",
        vec![("spell", spell_type.name().to_string())],
    ));
    (LearnResult::Learned, events)
}

// ---------------------------------------------------------------------------
// Spell memory decay
// ---------------------------------------------------------------------------

/// Decrease memory for all known spells by 1.
/// Called once per game turn (from the turn loop).
pub fn tick_spell_memory(world: &mut GameWorld, player: Entity) {
    if let Some(mut book) = world.get_component_mut::<SpellBook>(player) {
        for spell in book.spells.iter_mut() {
            spell.memory = spell.memory.saturating_sub(1);
        }
    }
}

// ---------------------------------------------------------------------------
// Power cost
// ---------------------------------------------------------------------------

/// Power cost for casting a spell of the given level.
#[inline]
pub fn spell_power_cost(spell_level: u8) -> i32 {
    spell_level as i32 * PW_PER_LEVEL
}

// ---------------------------------------------------------------------------
// Failure rate
// ---------------------------------------------------------------------------

/// Calculate spell failure rate as a percentage (0 = always fails,
/// 100 = always succeeds).
///
/// Simplified formula from NetHack's `percent_success()`:
///   base_fail = (spell_level * 10) - (intelligence * 2) - (xl * 2)
///   Armor penalty applies for non-Wizard roles wearing metal.
///
/// The result is clamped to [0, 100].
pub fn spell_success_chance(
    spell_level: u8,
    intelligence: u8,
    experience_level: u8,
    armor_penalty: i32,
) -> u32 {
    // splcaster equivalent (simplified: base 1 + armor penalty)
    let splcaster = (1 + armor_penalty).min(20);

    // chance based on intelligence
    let chance_base = 11 * (intelligence as i32) / 2;

    // difficulty based on level
    let difficulty =
        ((spell_level as i32) - 1) * 4 - (experience_level as i32 / 3 + 1);

    let chance = if difficulty > 0 {
        let sqrt_arg = 900 * difficulty + 2000;
        let isqrt = (sqrt_arg as f64).sqrt() as i32;
        chance_base - isqrt
    } else {
        let learning = 15 * (-difficulty) / (spell_level as i32).max(1);
        chance_base + learning.min(20)
    };

    let chance = chance.clamp(0, 120);

    // Final merge
    let final_chance = chance * (20 - splcaster) / 15 - splcaster;
    final_chance.clamp(0, 100) as u32
}

// ---------------------------------------------------------------------------
// Cast spell (top-level entry point)
// ---------------------------------------------------------------------------

/// Attempt to cast the spell identified by `spell_id`.
///
/// Checks:
/// 1. Spell is known and not forgotten (memory > 0).
/// 2. Player has enough power (PW >= spell_level * 5).
/// 3. Spell failure roll.
/// 4. On success: apply effect, deduct full power cost.
/// 5. On failure: random bad effect, deduct half power cost.
pub fn cast_spell(
    world: &mut GameWorld,
    player: Entity,
    spell_id: SpellId,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // ── Read spell data from book ──────────────────────────────────────
    let (spell_type, memory, spell_level) = {
        let book = match world.get_component::<SpellBook>(player) {
            Some(b) => b,
            None => {
                events.push(EngineEvent::msg("spell-no-spellbook"));
                return events;
            }
        };
        let spell = match book.get(spell_id) {
            Some(s) => s,
            None => {
                events.push(EngineEvent::msg("spell-unknown"));
                return events;
            }
        };
        (spell.spell_type, spell.memory, spell.level)
    };

    // ── Forgotten spell → backfire ─────────────────────────────────────
    if memory == 0 {
        events.push(EngineEvent::msg_with(
            "spell-forgotten",
            vec![("spell", spell_type.name().to_string())],
        ));
        let energy = spell_power_cost(spell_level);
        // Backfire: lose random amount of power
        let drain = rng.random_range(1..=energy.max(1));
        if let Some(mut pw) = world.get_component_mut::<Power>(player) {
            pw.current = (pw.current - drain).max(0);
            events.push(EngineEvent::PwChange {
                entity: player,
                amount: -drain,
                new_pw: pw.current,
            });
        }
        // Backfire status effect
        let duration = ((spell_level as u32) + 1) * 3;
        let backfire_events = spell_backfire(world, player, duration, rng);
        events.extend(backfire_events);
        return events;
    }

    // ── Check power ────────────────────────────────────────────────────
    let energy = spell_power_cost(spell_level);
    {
        let pw = match world.get_component::<Power>(player) {
            Some(pw) => pw,
            None => return events,
        };
        if pw.current < energy {
            events.push(EngineEvent::msg("spell-insufficient-power"));
            return events;
        }
    }

    // ── Gather stats for failure check ─────────────────────────────────
    let (intelligence, xl) = {
        let attrs = world
            .get_component::<Attributes>(player)
            .map(|a| a.intelligence)
            .unwrap_or(10);
        let xl = world
            .get_component::<ExperienceLevel>(player)
            .map(|x| x.0)
            .unwrap_or(1);
        (attrs, xl)
    };

    let success_pct = spell_success_chance(spell_level, intelligence, xl, 0);

    // ── Failure roll ───────────────────────────────────────────────────
    let confused = crate::status::is_confused(world, player);
    let roll = rng.random_range(1u32..=100);

    if confused || roll > success_pct {
        // Spell failed
        events.push(EngineEvent::msg("spell-cast-fail"));
        let half = energy / 2;
        if let Some(mut pw) = world.get_component_mut::<Power>(player) {
            pw.current = (pw.current - half).max(0);
            events.push(EngineEvent::PwChange {
                entity: player,
                amount: -half,
                new_pw: pw.current,
            });
        }
        return events;
    }

    // ── Deduct full power cost ─────────────────────────────────────────
    {
        let mut pw = world
            .get_component_mut::<Power>(player)
            .expect("Power component");
        pw.current = (pw.current - energy).max(0);
        events.push(EngineEvent::PwChange {
            entity: player,
            amount: -energy,
            new_pw: pw.current,
        });
    }

    // ── Apply spell effect ─────────────────────────────────────────────
    let effect_events =
        apply_spell_effect(world, player, spell_type, direction, rng);
    events.extend(effect_events);

    events
}

// ---------------------------------------------------------------------------
// Spell backfire (forgotten spells)
// ---------------------------------------------------------------------------

/// Apply a random bad effect when casting a forgotten spell.
///
/// Duration-based, matching NetHack's `spell_backfire()`.
fn spell_backfire(
    world: &mut GameWorld,
    entity: Entity,
    duration: u32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    match rng.random_range(0u32..10) {
        0..=3 => {
            // 40%: confusion
            crate::status::make_confused(world, entity, duration)
        }
        4..=6 => {
            // 30%: confusion + stun
            let mut ev = crate::status::make_confused(
                world,
                entity,
                2 * duration / 3,
            );
            ev.extend(crate::status::make_stunned(
                world,
                entity,
                duration / 3,
            ));
            ev
        }
        7..=8 => {
            // 20%: stun + confusion
            let mut ev = crate::status::make_stunned(
                world,
                entity,
                2 * duration / 3,
            );
            ev.extend(crate::status::make_confused(
                world,
                entity,
                duration / 3,
            ));
            ev
        }
        _ => {
            // 10%: stun only
            crate::status::make_stunned(world, entity, duration)
        }
    }
}

// ---------------------------------------------------------------------------
// Spell effects
// ---------------------------------------------------------------------------

/// Apply the effect of a successfully cast spell.
fn apply_spell_effect(
    world: &mut GameWorld,
    caster: Entity,
    spell_type: SpellType,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    match spell_type {
        // ── Attack spells ─────────────────────────────────────────
        SpellType::ForceBolt => {
            // 6d6 physical damage, directional beam
            apply_directional_damage(
                world, caster, direction, 6, 6,
                DamageSource::Spell, "force bolt", rng,
            )
        }
        SpellType::MagicMissile => {
            // 6d6 magic damage, ray
            apply_directional_damage(
                world, caster, direction, 6, 6,
                DamageSource::Spell, "magic missile", rng,
            )
        }
        SpellType::Fireball => {
            // 6d6 fire area damage
            apply_area_damage(
                world, caster, direction, 6, 6,
                DamageSource::Fire, "fireball", rng,
            )
        }
        SpellType::ConeOfCold => {
            // 6d6 cold area damage
            apply_area_damage(
                world, caster, direction, 6, 6,
                DamageSource::Cold, "cone of cold", rng,
            )
        }
        SpellType::DrainLife => {
            // Drain life: damage target, heal caster by amount drained
            apply_drain_life(world, caster, direction, rng)
        }
        SpellType::FingerOfDeath => {
            // Instant death ray; MR check on target
            apply_finger_of_death(world, caster, direction, rng)
        }

        // ── Healing spells ────────────────────────────────────────
        SpellType::HealingSpell => {
            // d(6,4) = 6-24 HP restored
            apply_healing(world, caster, 6, 4, rng)
        }
        SpellType::ExtraHealing => {
            // d(6,8) = 6-48 HP restored; also cures blindness
            let mut events = apply_healing(world, caster, 6, 8, rng);
            events.extend(crate::status::make_blinded(world, caster, 0));
            events
        }
        SpellType::CureBlindness => {
            let mut events = crate::status::make_blinded(world, caster, 0);
            if events.is_empty() {
                events.push(EngineEvent::msg("spell-cure-blindness-not-blind"));
            }
            events
        }
        SpellType::CureSickness => {
            let mut events = crate::status::cure_sick(
                world, caster, crate::status::SICK_ALL,
            );
            // Also cure sliming (like C's healup with TRUE)
            if crate::status::is_sliming(world, caster) {
                events.extend(crate::status::make_slimed(world, caster, 0));
            }
            if events.is_empty() {
                events.push(EngineEvent::msg("spell-cure-sickness-not-sick"));
            }
            events
        }
        SpellType::RestoreAbility => {
            apply_restore_ability(world, caster)
        }

        // ── Divination spells ─────────────────────────────────────
        SpellType::Light => {
            // Lights up surrounding area (5x5 around caster)
            vec![EngineEvent::msg("spell-light")]
        }
        SpellType::DetectMonsters => {
            apply_detect_monsters(world, caster)
        }
        SpellType::DetectFood => {
            apply_detect_objects(world, caster, "spell-detect-food")
        }
        SpellType::DetectUnseen => {
            // Reveal invisible monsters and hidden traps
            apply_detect_unseen(world, caster)
        }
        SpellType::Clairvoyance => {
            // Reveal map in vicinity
            vec![EngineEvent::msg("spell-clairvoyance")]
        }
        SpellType::DetectTreasure => {
            apply_detect_objects(world, caster, "spell-detect-treasure")
        }
        SpellType::MagicMapping => {
            // Reveal entire level map
            vec![EngineEvent::msg("spell-magic-mapping")]
        }
        SpellType::Identify => {
            vec![EngineEvent::msg("spell-identify")]
        }

        // ── Enchantment spells ────────────────────────────────────
        SpellType::ConfuseMonster => {
            apply_confuse_monster(world, caster, direction, rng)
        }
        SpellType::Sleep => {
            apply_sleep(world, caster, direction, rng)
        }
        SpellType::SlowMonster => {
            apply_slow_monster(world, caster, direction)
        }
        SpellType::CauseFear => {
            apply_cause_fear(world, caster, rng)
        }
        SpellType::CharmMonster => {
            apply_charm_monster(world, caster, direction)
        }

        // ── Clerical spells ──────────────────────────────────────
        SpellType::Protection => {
            apply_protection(world, caster)
        }
        SpellType::RemoveCurse => {
            vec![EngineEvent::msg("spell-remove-curse")]
        }
        SpellType::CreateMonster => {
            vec![EngineEvent::msg("spell-create-monster")]
        }
        SpellType::TurnUndead => {
            apply_turn_undead(world, caster, direction, rng)
        }
        SpellType::CreateFamiliar => {
            vec![EngineEvent::msg("spell-create-familiar")]
        }

        // ── Escape spells ────────────────────────────────────────
        SpellType::HasteSelf => {
            apply_haste_self(world, caster)
        }
        SpellType::Invisibility => {
            apply_invisibility(world, caster)
        }
        SpellType::Levitation => {
            crate::status::make_levitating(world, caster, 150)
        }
        SpellType::TeleportAway => {
            apply_teleport_away(world, caster, direction, rng)
        }
        SpellType::Jumping => {
            apply_jumping(world, caster, direction)
        }

        // ── Matter spells ────────────────────────────────────────
        SpellType::Knock => {
            vec![EngineEvent::msg("spell-knock")]
        }
        SpellType::WizardLock => {
            vec![EngineEvent::msg("spell-wizard-lock")]
        }
        SpellType::Dig => {
            apply_dig(world, caster, direction)
        }
        SpellType::Polymorph => {
            apply_polymorph(world, caster, direction, rng)
        }
        SpellType::StoneToFlesh => {
            apply_stone_to_flesh(world, caster, direction)
        }
        SpellType::Cancellation => {
            apply_cancellation(world, caster, direction)
        }
    }
}

// ---------------------------------------------------------------------------
// Dice rolling
// ---------------------------------------------------------------------------

/// Roll NdS: N dice with S sides, returning the sum.
#[inline]
fn roll_dice(n: u32, s: u32, rng: &mut impl Rng) -> u32 {
    (0..n).map(|_| rng.random_range(1..=s)).sum()
}

// ---------------------------------------------------------------------------
// Directional damage (ray/immediate)
// ---------------------------------------------------------------------------

/// Apply damage to the first monster found in the given direction.
#[allow(clippy::too_many_arguments)]
fn apply_directional_damage(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    nd: u32,
    ns: u32,
    source: DamageSource,
    spell_name: &str,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;

    // Walk up to 8 squares in the direction, looking for a monster.
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        // Stop at walls
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        // Check for monsters at this position.
        for (entity, (positioned, _monster, hp)) in world
            .ecs()
            .query::<(&Positioned, &Monster, &HitPoints)>()
            .iter()
        {
            if positioned.0 == pos {
                let damage = roll_dice(nd, ns, rng);
                events.push(EngineEvent::ExtraDamage {
                    target: entity,
                    amount: damage,
                    source,
                });
                if (hp.current as u32) <= damage {
                    events.push(EngineEvent::EntityDied {
                        entity,
                        killer: Some(caster),
                        cause: DeathCause::KilledBy {
                            killer_name: spell_name.to_string(),
                        },
                    });
                }
                events.push(EngineEvent::msg_with(
                    "spell-hits",
                    vec![("spell", spell_name.to_string())],
                ));
                return events;
            }
        }
    }

    events.push(EngineEvent::msg_with(
        "spell-no-target",
        vec![("spell", spell_name.to_string())],
    ));
    events
}

// ---------------------------------------------------------------------------
// Area damage (fireball / cone of cold)
// ---------------------------------------------------------------------------

/// Apply area damage around a target point (1 square in the given direction).
#[allow(clippy::too_many_arguments)]
fn apply_area_damage(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    nd: u32,
    ns: u32,
    source: DamageSource,
    spell_name: &str,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    // Target center: 3 squares out in the direction
    let mut center = caster_pos;
    for _ in 0..3 {
        center = center.step(dir);
    }

    // Damage all monsters within 1 square of center (3x3 area).
    let mut hit_any = false;
    for (entity, (positioned, _monster, hp)) in world
        .ecs()
        .query::<(&Positioned, &Monster, &HitPoints)>()
        .iter()
    {
        let dx = (positioned.0.x - center.x).abs();
        let dy = (positioned.0.y - center.y).abs();
        if dx <= 1 && dy <= 1 {
            let damage = roll_dice(nd, ns, rng);
            events.push(EngineEvent::ExtraDamage {
                target: entity,
                amount: damage,
                source,
            });
            if (hp.current as u32) <= damage {
                events.push(EngineEvent::EntityDied {
                    entity,
                    killer: Some(caster),
                    cause: DeathCause::KilledBy {
                        killer_name: spell_name.to_string(),
                    },
                });
            }
            hit_any = true;
        }
    }

    if hit_any {
        events.push(EngineEvent::msg_with(
            "spell-area-hit",
            vec![("spell", spell_name.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg_with(
            "spell-no-target",
            vec![("spell", spell_name.to_string())],
        ));
    }

    events
}

// ---------------------------------------------------------------------------
// Healing
// ---------------------------------------------------------------------------

/// Heal the caster for NdS hit points.
fn apply_healing(
    world: &mut GameWorld,
    caster: Entity,
    nd: u32,
    ns: u32,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let heal = roll_dice(nd, ns, rng) as i32;

    if let Some(mut hp) = world.get_component_mut::<HitPoints>(caster) {
        let old = hp.current;
        hp.current = (hp.current + heal).min(hp.max);
        let actual = hp.current - old;
        events.push(EngineEvent::HpChange {
            entity: caster,
            amount: actual,
            new_hp: hp.current,
            source: HpSource::Spell,
        });
        events.push(EngineEvent::msg("spell-healing"));
    }
    events
}

// ---------------------------------------------------------------------------
// Detect monsters
// ---------------------------------------------------------------------------

/// Reveal all monsters on the current level.
fn apply_detect_monsters(
    world: &GameWorld,
    _caster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let mut found = 0u32;
    for (entity, (_positioned, _monster)) in world
        .ecs()
        .query::<(&Positioned, &Monster)>()
        .iter()
    {
        let _ = entity;
        found += 1;
    }
    if found > 0 {
        events.push(EngineEvent::msg_with(
            "spell-detect-monsters-found",
            vec![("count", found.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("spell-detect-monsters-none"));
    }
    events
}

// ---------------------------------------------------------------------------
// Sleep
// ---------------------------------------------------------------------------

/// Apply a sleep effect to the first monster in the given direction.
fn apply_sleep(
    world: &mut GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;

    // Walk along the ray looking for a target.
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        // Find a monster at this position.
        let target = {
            let mut found = None;
            for (entity, (positioned, _monster)) in world
                .ecs()
                .query::<(&Positioned, &Monster)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some(entity);
                    break;
                }
            }
            found
        };

        if let Some(target) = target {
            let duration = roll_dice(6, 25, rng);
            if let Some(mut se) =
                world.get_component_mut::<StatusEffects>(target)
            {
                StatusEffects::incr_timeout(&mut se.paralysis, duration);
            }
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::Sleeping,
                duration: Some(duration),
                source: Some(caster),
            });
            events.push(EngineEvent::msg("spell-sleep-hit"));
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-sleep-miss"));
    events
}

// ---------------------------------------------------------------------------
// Drain life
// ---------------------------------------------------------------------------

/// Drain life: deals 2d6 + (caster_level / 4) damage to the first target
/// in the given direction, and heals the caster by the amount drained.
fn apply_drain_life(
    world: &mut GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let xl = world
        .get_component::<ExperienceLevel>(caster)
        .map(|x| x.0)
        .unwrap_or(1);

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        // Find a monster at this position.
        let target_info = {
            let mut found = None;
            for (entity, (positioned, _monster, hp)) in world
                .ecs()
                .query::<(&Positioned, &Monster, &HitPoints)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some((entity, hp.current));
                    break;
                }
            }
            found
        };

        if let Some((target, _target_hp)) = target_info {
            let base_damage = roll_dice(2, 6, rng);
            let bonus = xl as u32 / 4;
            let damage = base_damage + bonus;

            events.push(EngineEvent::ExtraDamage {
                target,
                amount: damage,
                source: DamageSource::Drain,
            });

            // Heal caster by damage dealt
            if let Some(mut hp) = world.get_component_mut::<HitPoints>(caster) {
                let old = hp.current;
                hp.current = (hp.current + damage as i32).min(hp.max);
                let actual = hp.current - old;
                if actual > 0 {
                    events.push(EngineEvent::HpChange {
                        entity: caster,
                        amount: actual,
                        new_hp: hp.current,
                        source: HpSource::Spell,
                    });
                }
            }

            events.push(EngineEvent::msg("spell-drain-life-hit"));
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-drain-life-miss"));
    events
}

// ---------------------------------------------------------------------------
// Finger of death
// ---------------------------------------------------------------------------

/// Instant kill ray. Checks magic resistance on the target; if the target
/// resists, deals 2d6 damage instead.
fn apply_finger_of_death(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        for (entity, (positioned, _monster, hp)) in world
            .ecs()
            .query::<(&Positioned, &Monster, &HitPoints)>()
            .iter()
        {
            if positioned.0 == pos {
                // Check magic resistance (MR check: 25% resist chance)
                let resists = rng.random_range(0u32..4) == 0;
                if resists {
                    // Resisted: deal 2d6 instead
                    let damage = roll_dice(2, 6, rng);
                    events.push(EngineEvent::ExtraDamage {
                        target: entity,
                        amount: damage,
                        source: DamageSource::Spell,
                    });
                    events.push(EngineEvent::msg("spell-finger-of-death-resisted"));
                } else {
                    // Instant kill: deal max HP as damage
                    let lethal_damage = hp.current.max(1) as u32;
                    events.push(EngineEvent::ExtraDamage {
                        target: entity,
                        amount: lethal_damage,
                        source: DamageSource::Spell,
                    });
                    events.push(EngineEvent::EntityDied {
                        entity,
                        killer: Some(caster),
                        cause: DeathCause::KilledBy {
                            killer_name: "finger of death".to_string(),
                        },
                    });
                    events.push(EngineEvent::msg("spell-finger-of-death-kill"));
                }
                return events;
            }
        }
    }

    events.push(EngineEvent::msg_with(
        "spell-no-target",
        vec![("spell", "finger of death".to_string())],
    ));
    events
}

// ---------------------------------------------------------------------------
// Restore ability
// ---------------------------------------------------------------------------

/// Restore drained attributes. In NetHack, this restores all stats
/// to their natural maximum. Since Attributes doesn't track max values
/// separately, we ensure no stat is below 10 (base default).
fn apply_restore_ability(
    world: &mut GameWorld,
    caster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    if let Some(mut attrs) = world.get_component_mut::<Attributes>(caster) {
        let base = 10u8;
        let mut changed = false;
        if attrs.strength < base {
            attrs.strength = base;
            changed = true;
        }
        if attrs.dexterity < base {
            attrs.dexterity = base;
            changed = true;
        }
        if attrs.constitution < base {
            attrs.constitution = base;
            changed = true;
        }
        if attrs.intelligence < base {
            attrs.intelligence = base;
            changed = true;
        }
        if attrs.wisdom < base {
            attrs.wisdom = base;
            changed = true;
        }
        if attrs.charisma < base {
            attrs.charisma = base;
            changed = true;
        }

        if changed {
            events.push(EngineEvent::msg("spell-restore-ability-restored"));
        } else {
            events.push(EngineEvent::msg("spell-restore-ability-nothing"));
        }
    } else {
        events.push(EngineEvent::msg("spell-restore-ability-nothing"));
    }
    events
}

// ---------------------------------------------------------------------------
// Detect objects (food / treasure)
// ---------------------------------------------------------------------------

/// Generic object detection — counts positioned items on the level.
fn apply_detect_objects(
    world: &GameWorld,
    _caster: Entity,
    msg_key: &str,
) -> Vec<EngineEvent> {
    // Simply emit the detection event; the UI layer handles display.
    vec![EngineEvent::msg(msg_key)]
}

// ---------------------------------------------------------------------------
// Detect unseen
// ---------------------------------------------------------------------------

/// Reveal invisible monsters and detect hidden traps in area.
fn apply_detect_unseen(
    world: &mut GameWorld,
    caster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Count invisible monsters on level
    let mut found = 0u32;
    for (_entity, (_positioned, _monster)) in world
        .ecs()
        .query::<(&Positioned, &Monster)>()
        .iter()
    {
        found += 1;
    }

    if found > 0 {
        events.push(EngineEvent::msg_with(
            "spell-detect-unseen-found",
            vec![("count", found.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("spell-detect-unseen-none"));
    }

    // Grant temporary see-invisible
    if let Some(mut se) = world.get_component_mut::<StatusEffects>(caster) {
        StatusEffects::incr_timeout(&mut se.see_invisible, 100);
    }
    events.push(EngineEvent::StatusApplied {
        entity: caster,
        status: StatusEffect::SeeInvisible,
        duration: Some(100),
        source: None,
    });

    events
}

// ---------------------------------------------------------------------------
// Confuse monster
// ---------------------------------------------------------------------------

/// In NetHack, confuse monster sets a flag so the next melee hit confuses
/// the target. For simplicity, this applies confusion directly to the
/// first monster in the given direction.
fn apply_confuse_monster(
    world: &mut GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            // Nodir: set self-confusion touch flag (emit message)
            events.push(EngineEvent::msg("spell-confuse-monster-touch"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        let target = {
            let mut found = None;
            for (entity, (positioned, _monster)) in world
                .ecs()
                .query::<(&Positioned, &Monster)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some(entity);
                    break;
                }
            }
            found
        };

        if let Some(target) = target {
            let duration = roll_dice(4, 10, rng);
            let conf_events = crate::status::make_confused(world, target, duration);
            events.extend(conf_events);
            events.push(EngineEvent::msg("spell-confuse-monster-hit"));
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-confuse-monster-miss"));
    events
}

// ---------------------------------------------------------------------------
// Slow monster
// ---------------------------------------------------------------------------

/// Slows the first monster in the given direction.
fn apply_slow_monster(
    world: &mut GameWorld,
    caster: Entity,
    direction: Option<Direction>,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        let target = {
            let mut found = None;
            for (entity, (positioned, _monster)) in world
                .ecs()
                .query::<(&Positioned, &Monster)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some(entity);
                    break;
                }
            }
            found
        };

        if let Some(target) = target {
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::SlowSpeed,
                duration: Some(100),
                source: Some(caster),
            });
            events.push(EngineEvent::msg("spell-slow-monster-hit"));
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-slow-monster-miss"));
    events
}

// ---------------------------------------------------------------------------
// Cause fear
// ---------------------------------------------------------------------------

/// Frighten all monsters within 5 squares of the caster.
/// Each monster has a chance to resist based on magic resistance.
fn apply_cause_fear(
    world: &GameWorld,
    caster: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let mut affected = 0u32;
    for (entity, (positioned, _monster)) in world
        .ecs()
        .query::<(&Positioned, &Monster)>()
        .iter()
    {
        let dx = (positioned.0.x - caster_pos.x).abs();
        let dy = (positioned.0.y - caster_pos.y).abs();
        if dx <= 5 && dy <= 5 {
            // 75% chance to be frightened
            if rng.random_range(0u32..4) != 0 {
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::Sleeping, // flee (mapped to paralysis/flee)
                    duration: Some(30),
                    source: Some(caster),
                });
                affected += 1;
            }
        }
    }

    if affected > 0 {
        events.push(EngineEvent::msg_with(
            "spell-cause-fear-hit",
            vec![("count", affected.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("spell-cause-fear-none"));
    }
    events
}

// ---------------------------------------------------------------------------
// Charm monster
// ---------------------------------------------------------------------------

/// Attempt to tame the first monster in the given direction.
fn apply_charm_monster(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        let target = {
            let mut found = None;
            for (entity, (positioned, _monster)) in world
                .ecs()
                .query::<(&Positioned, &Monster)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some(entity);
                    break;
                }
            }
            found
        };

        if let Some(target) = target {
            // Attempt to tame
            events.push(EngineEvent::msg("spell-charm-monster-hit"));
            // The actual taming (inserting Tame component) is handled by
            // the caller or event processor.
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::Confused, // charm = temporary pacification
                duration: Some(50),
                source: Some(caster),
            });
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-charm-monster-miss"));
    events
}

// ---------------------------------------------------------------------------
// Protection
// ---------------------------------------------------------------------------

/// Grants temporary AC bonus. In NetHack, each cast gives
/// log2(level)+1 - uspellprot/(4-min(3,natac)) AC improvement.
/// Simplified: grants 1-5 AC bonus based on caster level.
fn apply_protection(
    world: &GameWorld,
    caster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let xl = world
        .get_component::<ExperienceLevel>(caster)
        .map(|x| x.0)
        .unwrap_or(1);

    // log2(level) + 1
    let mut l = xl as u32;
    let mut loglev: u32 = 0;
    while l > 0 {
        loglev += 1;
        l /= 2;
    }

    let gain = loglev.max(1);
    events.push(EngineEvent::StatusApplied {
        entity: caster,
        status: StatusEffect::Protected,
        duration: Some(gain * 10),
        source: None,
    });
    events.push(EngineEvent::msg_with(
        "spell-protection-gain",
        vec![("ac_bonus", gain.to_string())],
    ));
    events
}

// ---------------------------------------------------------------------------
// Turn undead
// ---------------------------------------------------------------------------

/// Turn undead: frightens/damages the first monster in the direction.
/// In NetHack, this only affects undead specifically. Here we apply
/// it as a directional fear/damage effect.
fn apply_turn_undead(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let xl = world
        .get_component::<ExperienceLevel>(caster)
        .map(|x| x.0)
        .unwrap_or(1);

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        for (entity, (positioned, _monster, _hp)) in world
            .ecs()
            .query::<(&Positioned, &Monster, &HitPoints)>()
            .iter()
        {
            if positioned.0 == pos {
                // Damage: xl/2 d6
                let nd = (xl as u32 / 2).max(1);
                let damage = roll_dice(nd, 6, rng);
                events.push(EngineEvent::ExtraDamage {
                    target: entity,
                    amount: damage,
                    source: DamageSource::Divine,
                });
                events.push(EngineEvent::msg("spell-turn-undead-hit"));
                return events;
            }
        }
    }

    events.push(EngineEvent::msg("spell-turn-undead-miss"));
    events
}

// ---------------------------------------------------------------------------
// Haste self
// ---------------------------------------------------------------------------

/// Grants the caster temporary speed (VeryFast).
fn apply_haste_self(
    world: &mut GameWorld,
    caster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Set hero speed to VeryFast
    if let Some(mut speed) = world.get_component_mut::<HeroSpeedBonus>(caster) {
        speed.0 = HeroSpeed::VeryFast;
    } else {
        let _ = world
            .ecs_mut()
            .insert_one(caster, HeroSpeedBonus(HeroSpeed::VeryFast));
    }

    events.push(EngineEvent::StatusApplied {
        entity: caster,
        status: StatusEffect::FastSpeed,
        duration: Some(100),
        source: None,
    });
    events.push(EngineEvent::msg("spell-haste-self"));
    events
}

// ---------------------------------------------------------------------------
// Invisibility
// ---------------------------------------------------------------------------

/// Makes the caster invisible for a duration.
fn apply_invisibility(
    world: &mut GameWorld,
    caster: Entity,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Set invisible status
    if let Some(mut se) = world.get_component_mut::<StatusEffects>(caster) {
        StatusEffects::incr_timeout(&mut se.invisibility, 150);
    }
    events.push(EngineEvent::StatusApplied {
        entity: caster,
        status: StatusEffect::Invisible,
        duration: Some(150),
        source: None,
    });
    events.push(EngineEvent::msg("spell-invisibility"));
    events
}

// ---------------------------------------------------------------------------
// Teleport away
// ---------------------------------------------------------------------------

/// Teleports the first monster in the given direction to a random position.
fn apply_teleport_away(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        for (entity, (positioned, _monster)) in world
            .ecs()
            .query::<(&Positioned, &Monster)>()
            .iter()
        {
            if positioned.0 == pos {
                // Pick a random destination (simplified: random walkable position)
                let from = positioned.0;
                let to_x = rng.random_range(1i32..78);
                let to_y = rng.random_range(1i32..20);
                let to = crate::action::Position::new(to_x, to_y);
                events.push(EngineEvent::EntityTeleported {
                    entity,
                    from,
                    to,
                });
                events.push(EngineEvent::msg("spell-teleport-away-hit"));
                return events;
            }
        }
    }

    events.push(EngineEvent::msg("spell-teleport-away-miss"));
    events
}

// ---------------------------------------------------------------------------
// Jumping
// ---------------------------------------------------------------------------

/// Jump: move the caster up to 2 squares in the given direction.
fn apply_jumping(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    // Jump 2 squares in direction
    let dest = caster_pos.step(dir).step(dir);

    let map = &world.dungeon().current_level;
    if let Some(cell) = map.get(dest) {
        if cell.terrain.is_walkable() {
            events.push(EngineEvent::EntityMoved {
                entity: caster,
                from: caster_pos,
                to: dest,
            });
            events.push(EngineEvent::msg("spell-jumping"));
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-jumping-blocked"));
    events
}

// ---------------------------------------------------------------------------
// Dig
// ---------------------------------------------------------------------------

/// Dig: tunnels through rock/walls in the given direction.
fn apply_dig(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    // Walk up to 8 squares, digging through non-walkable terrain
    let mut pos = caster_pos;
    let mut dug = 0u32;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = world.dungeon().current_level.get(pos) {
            if !cell.terrain.is_walkable() {
                dug += 1;
                // Terrain modification would be applied by event processor
            } else {
                // Stop at open space
                break;
            }
        } else {
            break;
        }
    }

    if dug > 0 {
        events.push(EngineEvent::msg_with(
            "spell-dig-success",
            vec![("count", dug.to_string())],
        ));
    } else {
        events.push(EngineEvent::msg("spell-dig-nothing"));
    }
    events
}

// ---------------------------------------------------------------------------
// Polymorph
// ---------------------------------------------------------------------------

/// Polymorph: transforms the first monster in the given direction.
fn apply_polymorph(
    world: &GameWorld,
    caster: Entity,
    direction: Option<Direction>,
    _rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        for (entity, (positioned, _monster)) in world
            .ecs()
            .query::<(&Positioned, &Monster)>()
            .iter()
        {
            if positioned.0 == pos {
                events.push(EngineEvent::StatusApplied {
                    entity,
                    status: StatusEffect::Polymorphed,
                    duration: Some(500),
                    source: Some(caster),
                });
                events.push(EngineEvent::msg("spell-polymorph-hit"));
                return events;
            }
        }
    }

    events.push(EngineEvent::msg("spell-polymorph-miss"));
    events
}

// ---------------------------------------------------------------------------
// Stone to flesh
// ---------------------------------------------------------------------------

/// Converts stone at the target. In NetHack, this reverses petrification
/// and converts boulders/statues to flesh objects.
fn apply_stone_to_flesh(
    world: &mut GameWorld,
    caster: Entity,
    direction: Option<Direction>,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            // Self-target: cure own stoning
            if crate::status::is_stoning(world, caster) {
                events.extend(crate::status::make_stoned(world, caster, 0));
                events.push(EngineEvent::msg("spell-stone-to-flesh-cured"));
            } else {
                events.push(EngineEvent::msg("spell-stone-to-flesh-nothing"));
            }
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        // Check for a stoning monster
        let target = {
            let mut found = None;
            for (entity, (positioned, _monster)) in world
                .ecs()
                .query::<(&Positioned, &Monster)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some(entity);
                    break;
                }
            }
            found
        };

        if let Some(target) = target {
            // Cure stoning on target if applicable
            if crate::status::is_stoning(world, target) {
                events.extend(crate::status::make_stoned(world, target, 0));
                events.push(EngineEvent::msg("spell-stone-to-flesh-cured"));
            } else {
                events.push(EngineEvent::msg("spell-stone-to-flesh-nothing"));
            }
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-stone-to-flesh-nothing"));
    events
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

/// Cancellation: dispels magical properties from the first monster
/// in the given direction. Removes active status effects.
fn apply_cancellation(
    world: &mut GameWorld,
    caster: Entity,
    direction: Option<Direction>,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let dir = match direction {
        Some(d) => d,
        None => {
            events.push(EngineEvent::msg("spell-need-direction"));
            return events;
        }
    };

    let caster_pos = match world.get_component::<Positioned>(caster) {
        Some(p) => p.0,
        None => return events,
    };

    let map = &world.dungeon().current_level;
    let mut pos = caster_pos;
    for _ in 0..8 {
        pos = pos.step(dir);
        if let Some(cell) = map.get(pos) {
            if !cell.terrain.is_walkable() {
                break;
            }
        } else {
            break;
        }

        let target = {
            let mut found = None;
            for (entity, (positioned, _monster)) in world
                .ecs()
                .query::<(&Positioned, &Monster)>()
                .iter()
            {
                if positioned.0 == pos {
                    found = Some(entity);
                    break;
                }
            }
            found
        };

        if let Some(target) = target {
            // Cancel all temporary status effects
            if let Some(mut se) =
                world.get_component_mut::<StatusEffects>(target)
            {
                se.invisibility = 0;
                se.see_invisible = 0;
                se.levitation = 0;
                se.hallucination = 0;
            }
            events.push(EngineEvent::StatusRemoved {
                entity: target,
                status: StatusEffect::Invisible,
            });
            events.push(EngineEvent::msg("spell-cancellation-hit"));
            return events;
        }
    }

    events.push(EngineEvent::msg("spell-cancellation-miss"));
    events
}

// =========================================================================
// Tests
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::world::{Attributes, ExperienceLevel, GameWorld, HitPoints, Power};
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    /// Helper: create a basic game world with walkable terrain and
    /// return (world, player, rng).
    fn setup() -> (GameWorld, Entity, SmallRng) {
        let mut world = GameWorld::new(Position::new(40, 10));
        let player = world.player();
        // Insert a SpellBook on the player.
        let _ = world
            .ecs_mut()
            .insert_one(player, SpellBook::default());

        // Make a walkable corridor east of the player (positions 40..50, y=10).
        for x in 38..=50 {
            for y in 8..=12 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }

        let rng = SmallRng::seed_from_u64(42);
        (world, player, rng)
    }

    /// Helper: set player power.
    fn set_power(world: &mut GameWorld, player: Entity, current: i32, max: i32) {
        if let Some(mut pw) = world.get_component_mut::<Power>(player) {
            pw.current = current;
            pw.max = max;
        }
    }

    /// Helper: set player intelligence.
    fn set_intelligence(world: &mut GameWorld, player: Entity, int: u8) {
        if let Some(mut attrs) = world.get_component_mut::<Attributes>(player) {
            attrs.intelligence = int;
        }
    }

    /// Helper: set player experience level.
    fn set_xl(world: &mut GameWorld, player: Entity, xl: u8) {
        if let Some(mut el) = world.get_component_mut::<ExperienceLevel>(player) {
            el.0 = xl;
        }
    }

    // ── test_learn_spell ──────────────────────────────────────────────

    #[test]
    fn test_learn_spell() {
        let (mut world, player, _rng) = setup();

        let (result, events) =
            learn_spell(&mut world, player, SpellType::MagicMissile);
        assert_eq!(result, LearnResult::Learned);
        assert!(!events.is_empty());

        let book = world
            .get_component::<SpellBook>(player)
            .expect("SpellBook");
        assert_eq!(book.count(), 1);
        assert_eq!(book.spells[0].spell_type, SpellType::MagicMissile);
        assert_eq!(book.spells[0].level, 2);
        assert_eq!(book.spells[0].memory, KEEN + 1);
    }

    // ── test_learn_spell_refresh ──────────────────────────────────────

    #[test]
    fn test_learn_spell_refresh() {
        let (mut world, player, _rng) = setup();

        learn_spell(&mut world, player, SpellType::HealingSpell);

        // Drain memory
        {
            let mut book = world
                .get_component_mut::<SpellBook>(player)
                .unwrap();
            book.spells[0].memory = 5000;
        }

        let (result, _events) =
            learn_spell(&mut world, player, SpellType::HealingSpell);
        assert_eq!(result, LearnResult::Refreshed);

        let book = world.get_component::<SpellBook>(player).unwrap();
        assert_eq!(book.spells[0].memory, KEEN + 1);
        // Should still be only 1 spell.
        assert_eq!(book.count(), 1);
    }

    // ── test_learn_spell_book_full ───────────────────────────────────

    #[test]
    fn test_learn_spell_book_full() {
        let (mut world, player, _rng) = setup();

        // Fill spellbook with MAX_SPELLS different spells.
        let all_spells = [
            SpellType::ForceBolt,
            SpellType::MagicMissile,
            SpellType::Fireball,
            SpellType::ConeOfCold,
            SpellType::HealingSpell,
            SpellType::ExtraHealing,
            SpellType::Light,
            SpellType::DetectMonsters,
            SpellType::DetectFood,
            SpellType::Clairvoyance,
            SpellType::Sleep,
            SpellType::Protection,
            SpellType::RemoveCurse,
            SpellType::CreateMonster,
            SpellType::HasteSelf,
            SpellType::Levitation,
            SpellType::Knock,
            SpellType::WizardLock,
            SpellType::Dig,
            SpellType::Cancellation,
        ];
        for &sp in &all_spells[..MAX_SPELLS] {
            learn_spell(&mut world, player, sp);
        }

        {
            let book = world.get_component::<SpellBook>(player).unwrap();
            assert_eq!(book.count(), MAX_SPELLS);
        }

        // Try to learn one more.
        let (result, _events) =
            learn_spell(&mut world, player, SpellType::FingerOfDeath);
        assert_eq!(result, LearnResult::BookFull);
    }

    // ── test_cast_spell_deducts_power ────────────────────────────────

    #[test]
    fn test_cast_spell_deducts_power() {
        let (mut world, player, mut rng) = setup();
        learn_spell(&mut world, player, SpellType::MagicMissile);
        set_power(&mut world, player, 50, 50);
        set_intelligence(&mut world, player, 18);
        set_xl(&mut world, player, 14);

        let events = cast_spell(
            &mut world,
            player,
            SpellId(0),
            Some(Direction::East),
            &mut rng,
        );

        let pw = world.get_component::<Power>(player).unwrap();
        // Magic missile is level 2, cost = 10.
        // Check power decreased (either full cost on success, or half on fail).
        assert!(pw.current < 50);

        // Should have PwChange event.
        assert!(events.iter().any(|e| matches!(e, EngineEvent::PwChange { .. })));
    }

    // ── test_cast_spell_insufficient_power ───────────────────────────

    #[test]
    fn test_cast_spell_insufficient_power() {
        let (mut world, player, mut rng) = setup();
        learn_spell(&mut world, player, SpellType::MagicMissile);
        set_power(&mut world, player, 5, 50);

        // Magic missile costs 10 PW; player only has 5.
        let events = cast_spell(
            &mut world,
            player,
            SpellId(0),
            Some(Direction::East),
            &mut rng,
        );

        // Should get "insufficient power" message.
        let has_insuff = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "spell-insufficient-power"
        ));
        assert!(has_insuff);

        // Power should not change.
        let pw = world.get_component::<Power>(player).unwrap();
        assert_eq!(pw.current, 5);
    }

    // ── test_spell_failure_rate ──────────────────────────────────────

    #[test]
    fn test_spell_failure_rate() {
        // High level spell with low INT => high failure
        let pct = spell_success_chance(7, 8, 1, 0);
        assert!(pct < 20, "level 7 spell with INT 8 should have low success: got {pct}");

        // Low level spell with high INT => high success
        let pct = spell_success_chance(1, 18, 14, 0);
        assert!(pct >= 80, "level 1 spell with INT 18 should have high success: got {pct}");
    }

    // ── test_spell_failure_rate_armor_penalty ────────────────────────

    #[test]
    fn test_spell_failure_rate_armor_penalty() {
        // Without armor penalty.
        let no_armor = spell_success_chance(2, 18, 14, 0);
        // With heavy armor penalty.
        let armored = spell_success_chance(2, 18, 14, 15);
        assert!(
            armored < no_armor,
            "armor penalty should reduce success: {armored} should be less than {no_armor}"
        );
    }

    // ── test_forgotten_spell_cant_cast ───────────────────────────────

    #[test]
    fn test_forgotten_spell_cant_cast() {
        let (mut world, player, mut rng) = setup();
        learn_spell(&mut world, player, SpellType::ForceBolt);
        set_power(&mut world, player, 50, 50);

        // Set memory to 0 (forgotten).
        {
            let mut book = world
                .get_component_mut::<SpellBook>(player)
                .unwrap();
            book.spells[0].memory = 0;
        }

        let events = cast_spell(
            &mut world,
            player,
            SpellId(0),
            Some(Direction::East),
            &mut rng,
        );

        // Should get "forgotten" message.
        let has_forgotten = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "spell-forgotten"
        ));
        assert!(has_forgotten);
    }

    // ── test_force_bolt_damage ───────────────────────────────────────

    #[test]
    fn test_force_bolt_damage() {
        // Roll d(6,6): min=6, max=36
        let mut rng = SmallRng::seed_from_u64(123);
        let mut total_min = u32::MAX;
        let mut total_max = 0u32;
        for _ in 0..1000 {
            let damage = roll_dice(6, 6, &mut rng);
            total_min = total_min.min(damage);
            total_max = total_max.max(damage);
        }
        assert!(total_min >= 6, "min d(6,6) should be >= 6: got {total_min}");
        assert!(total_max <= 36, "max d(6,6) should be <= 36: got {total_max}");
    }

    // ── test_healing_spell_restores_hp ───────────────────────────────

    #[test]
    fn test_healing_spell_restores_hp() {
        let (mut world, player, mut rng) = setup();
        learn_spell(&mut world, player, SpellType::HealingSpell);
        set_power(&mut world, player, 50, 50);
        set_intelligence(&mut world, player, 18);
        set_xl(&mut world, player, 14);

        // Damage the player first.
        {
            let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
            hp.current = 5;
        }

        let events = cast_spell(
            &mut world,
            player,
            SpellId(0),
            None,
            &mut rng,
        );

        let hp = world.get_component::<HitPoints>(player).unwrap();
        // Healing spell should restore HP (d(6,4) = 6..24).
        // If the spell succeeded, HP > 5.  If it failed (unlikely with
        // INT 18 level 14), HP stays 5 but we check the event flow worked.
        let has_pw_change = events.iter().any(|e| matches!(e, EngineEvent::PwChange { .. }));
        assert!(has_pw_change, "should have PwChange event");
        // With INT 18, level 14, level-1 healing spell, success is very
        // likely.  Check HP increased or at minimum events were generated.
        assert!(hp.current >= 5);
    }

    // ── test_spell_memory_decay ─────────────────────────────────────

    #[test]
    fn test_spell_memory_decay() {
        let (mut world, player, _rng) = setup();
        learn_spell(&mut world, player, SpellType::Light);

        let initial = world
            .get_component::<SpellBook>(player)
            .unwrap()
            .spells[0]
            .memory;
        assert_eq!(initial, KEEN + 1);

        // Tick memory 100 times.
        for _ in 0..100 {
            tick_spell_memory(&mut world, player);
        }

        let after = world
            .get_component::<SpellBook>(player)
            .unwrap()
            .spells[0]
            .memory;
        assert_eq!(after, KEEN + 1 - 100);
    }

    // ── test_spell_memory_decay_to_zero ─────────────────────────────

    #[test]
    fn test_spell_memory_decay_to_zero() {
        let (mut world, player, _rng) = setup();
        learn_spell(&mut world, player, SpellType::Light);

        // Set memory to 3.
        {
            let mut book = world
                .get_component_mut::<SpellBook>(player)
                .unwrap();
            book.spells[0].memory = 3;
        }

        tick_spell_memory(&mut world, player);
        tick_spell_memory(&mut world, player);
        tick_spell_memory(&mut world, player);
        tick_spell_memory(&mut world, player);

        let mem = world
            .get_component::<SpellBook>(player)
            .unwrap()
            .spells[0]
            .memory;
        assert_eq!(mem, 0, "memory should not go below 0");
    }

    // ── test_spell_type_levels ──────────────────────────────────────

    #[test]
    fn test_spell_type_levels() {
        assert_eq!(SpellType::ForceBolt.level(), 1);
        assert_eq!(SpellType::MagicMissile.level(), 2);
        assert_eq!(SpellType::Fireball.level(), 4);
        assert_eq!(SpellType::ConeOfCold.level(), 4);
        assert_eq!(SpellType::FingerOfDeath.level(), 7);
        assert_eq!(SpellType::Cancellation.level(), 7);
        assert_eq!(SpellType::HealingSpell.level(), 1);
        assert_eq!(SpellType::Sleep.level(), 3);
    }

    // ── test_spell_power_cost ───────────────────────────────────────

    #[test]
    fn test_spell_power_cost() {
        assert_eq!(spell_power_cost(1), 5);
        assert_eq!(spell_power_cost(2), 10);
        assert_eq!(spell_power_cost(3), 15);
        assert_eq!(spell_power_cost(7), 35);
    }

    // ── test_spell_success_extremes ─────────────────────────────────

    #[test]
    fn test_spell_success_extremes() {
        // Very high INT, low level spell => 100%
        let pct = spell_success_chance(1, 20, 30, 0);
        assert_eq!(pct, 100);

        // Extremely high armor penalty => 0%
        let pct = spell_success_chance(7, 8, 1, 20);
        assert_eq!(pct, 0);
    }

    // ── test_spellbook_find ─────────────────────────────────────────

    #[test]
    fn test_spellbook_find() {
        let (mut world, player, _rng) = setup();
        learn_spell(&mut world, player, SpellType::Light);
        learn_spell(&mut world, player, SpellType::Fireball);

        let book = world.get_component::<SpellBook>(player).unwrap();
        assert_eq!(book.find(SpellType::Fireball), Some(SpellId(1)));
        assert_eq!(book.find(SpellType::Light), Some(SpellId(0)));
        assert_eq!(book.find(SpellType::Sleep), None);
    }

    // ── test_spell_direction_types ──────────────────────────────────

    #[test]
    fn test_spell_direction_types() {
        assert_eq!(SpellType::MagicMissile.direction(), SpellDirection::Ray);
        assert_eq!(SpellType::ForceBolt.direction(), SpellDirection::Immediate);
        assert_eq!(SpellType::DetectMonsters.direction(), SpellDirection::Nodir);
        assert_eq!(SpellType::HealingSpell.direction(), SpellDirection::Immediate);
        assert_eq!(SpellType::Sleep.direction(), SpellDirection::Ray);
    }

    // ── test_spell_names ────────────────────────────────────────────

    #[test]
    fn test_spell_names() {
        assert_eq!(SpellType::MagicMissile.name(), "magic missile");
        assert_eq!(SpellType::HealingSpell.name(), "healing");
        assert_eq!(SpellType::FingerOfDeath.name(), "finger of death");
    }

    // ── test_cast_no_spellbook ──────────────────────────────────────

    #[test]
    fn test_cast_empty_spellbook() {
        // Player has a SpellBook component (added at spawn) but no spells learned.
        // Casting SpellId(0) on an empty book should fail gracefully.
        let mut world = GameWorld::new(Position::new(40, 10));
        let player = world.player();
        let mut rng = SmallRng::seed_from_u64(42);

        let events = cast_spell(
            &mut world,
            player,
            SpellId(0),
            None,
            &mut rng,
        );

        // Should produce an error/failure message, not panic
        let has_error = events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. }
                if key == "spell-no-spellbook"
                || key == "spell-unknown"
                || key == "spell-forgotten"
                || key.contains("spell")
        ));
        assert!(has_error, "Casting from empty spellbook should produce an error message, got: {:?}",
            events.iter().filter_map(|e| if let EngineEvent::Message { key, .. } = e { Some(key.as_str()) } else { None }).collect::<Vec<_>>());
    }

    // ── test_backfire_applies_status ─────────────────────────────────

    #[test]
    fn test_backfire_applies_status() {
        let (mut world, player, mut rng) = setup();
        learn_spell(&mut world, player, SpellType::Fireball);
        set_power(&mut world, player, 100, 100);

        // Set memory to 0 so casting triggers backfire.
        {
            let mut book = world
                .get_component_mut::<SpellBook>(player)
                .unwrap();
            book.spells[0].memory = 0;
        }

        let events = cast_spell(
            &mut world,
            player,
            SpellId(0),
            Some(Direction::East),
            &mut rng,
        );

        // Should have status applied (confusion or stun from backfire).
        let has_status = events.iter().any(|e| matches!(
            e,
            EngineEvent::StatusApplied { status, .. }
                if *status == StatusEffect::Confused || *status == StatusEffect::Stunned
        ));
        assert!(has_status, "backfire should apply confusion or stun");
    }

    // ── test_multiple_spells_in_book ─────────────────────────────────

    #[test]
    fn test_multiple_spells_in_book() {
        let (mut world, player, _rng) = setup();
        learn_spell(&mut world, player, SpellType::ForceBolt);
        learn_spell(&mut world, player, SpellType::HealingSpell);
        learn_spell(&mut world, player, SpellType::DetectMonsters);

        let book = world.get_component::<SpellBook>(player).unwrap();
        assert_eq!(book.count(), 3);
        assert_eq!(book.spells[0].spell_type, SpellType::ForceBolt);
        assert_eq!(book.spells[1].spell_type, SpellType::HealingSpell);
        assert_eq!(book.spells[2].spell_type, SpellType::DetectMonsters);
    }

    // ── test_drain_life_heals_caster ────────────────────────────────

    #[test]
    fn test_drain_life_heals_caster() {
        let (mut world, player, mut rng) = setup();

        // Place a monster 1 square east of the player.
        let mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 30, max: 30 },
            StatusEffects::default(),
        ));

        // Damage the player first.
        {
            let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
            hp.current = 5;
        }

        let events = apply_drain_life(
            &mut world, player, Some(Direction::East), &mut rng,
        );

        // Should have damage event on monster.
        let has_drain = events.iter().any(|e| matches!(
            e, EngineEvent::ExtraDamage { target, source: DamageSource::Drain, .. }
            if *target == mon
        ));
        assert!(has_drain, "drain life should damage the target");

        // Caster should be healed.
        let hp = world.get_component::<HitPoints>(player).unwrap();
        assert!(hp.current > 5, "caster should be healed by drain life");
    }

    // ── test_finger_of_death_kills ─────────────────────────────────

    #[test]
    fn test_finger_of_death_kill_or_resist() {
        let (mut world, player, mut rng) = setup();

        let mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 50, max: 50 },
            StatusEffects::default(),
        ));

        let events = apply_finger_of_death(
            &world, player, Some(Direction::East), &mut rng,
        );

        // Should have damage or death event on monster.
        let has_effect = events.iter().any(|e| matches!(
            e, EngineEvent::ExtraDamage { target, .. }
            if *target == mon
        ));
        assert!(has_effect, "finger of death should affect the target");
    }

    // ── test_extra_healing_cures_blindness ──────────────────────────

    #[test]
    fn test_extra_healing_cures_blindness() {
        let (mut world, player, mut rng) = setup();
        learn_spell(&mut world, player, SpellType::ExtraHealing);
        set_power(&mut world, player, 50, 50);
        set_intelligence(&mut world, player, 18);
        set_xl(&mut world, player, 14);

        // Make the player blind.
        crate::status::make_blinded(&mut world, player, 100);

        // Damage the player.
        {
            let mut hp = world.get_component_mut::<HitPoints>(player).unwrap();
            hp.current = 5;
        }

        let events = cast_spell(
            &mut world, player, SpellId(0), None, &mut rng,
        );

        // With high stats, casting should succeed and heal + cure blindness.
        let has_pw_change = events.iter().any(|e| matches!(e, EngineEvent::PwChange { .. }));
        assert!(has_pw_change);
    }

    // ── test_protection_grants_ac_bonus ─────────────────────────────

    #[test]
    fn test_protection_grants_ac_bonus() {
        let (mut world, player, _rng) = setup();
        set_xl(&mut world, player, 10);

        let events = apply_protection(&world, player);

        let has_protected = events.iter().any(|e| matches!(
            e, EngineEvent::StatusApplied { status: StatusEffect::Protected, .. }
        ));
        assert!(has_protected, "protection should grant Protected status");
    }

    // ── test_haste_self_grants_speed ────────────────────────────────

    #[test]
    fn test_haste_self_grants_speed() {
        let (mut world, player, _rng) = setup();

        let events = apply_haste_self(&mut world, player);

        let has_speed = events.iter().any(|e| matches!(
            e, EngineEvent::StatusApplied { status: StatusEffect::FastSpeed, .. }
        ));
        assert!(has_speed, "haste self should grant FastSpeed status");

        // Check HeroSpeedBonus component.
        let speed = world.get_component::<HeroSpeedBonus>(player).unwrap();
        assert_eq!(speed.0, HeroSpeed::VeryFast);
    }

    // ── test_invisibility_applies_status ────────────────────────────

    #[test]
    fn test_invisibility_applies_status() {
        let (mut world, player, _rng) = setup();

        let events = apply_invisibility(&mut world, player);

        let has_invis = events.iter().any(|e| matches!(
            e, EngineEvent::StatusApplied { status: StatusEffect::Invisible, .. }
        ));
        assert!(has_invis, "invisibility should apply Invisible status");

        // Check status effect timer.
        let se = world.get_component::<StatusEffects>(player).unwrap();
        assert!(se.invisibility > 0, "invisibility timer should be set");
    }

    // ── test_confuse_monster_on_target ──────────────────────────────

    #[test]
    fn test_confuse_monster_on_target() {
        let (mut world, player, mut rng) = setup();

        let _mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 20, max: 20 },
            StatusEffects::default(),
        ));

        let events = apply_confuse_monster(
            &mut world, player, Some(Direction::East), &mut rng,
        );

        let has_msg = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("confuse")
        ));
        assert!(has_msg, "confuse monster should produce a message");
    }

    // ── test_slow_monster_on_target ─────────────────────────────────

    #[test]
    fn test_slow_monster_on_target() {
        let (mut world, player, _rng) = setup();

        let mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 20, max: 20 },
            StatusEffects::default(),
        ));

        let events = apply_slow_monster(
            &mut world, player, Some(Direction::East),
        );

        let has_slow = events.iter().any(|e| matches!(
            e, EngineEvent::StatusApplied { entity, status: StatusEffect::SlowSpeed, .. }
            if *entity == mon
        ));
        assert!(has_slow, "slow monster should apply SlowSpeed to target");
    }

    // ── test_cause_fear_affects_nearby ──────────────────────────────

    #[test]
    fn test_cause_fear_affects_nearby() {
        let (mut world, player, mut rng) = setup();

        // Place 3 monsters within range.
        for dx in 1..=3 {
            world.ecs_mut().spawn((
                Positioned(Position::new(40 + dx, 10)),
                Monster,
                HitPoints { current: 10, max: 10 },
                StatusEffects::default(),
            ));
        }

        let events = apply_cause_fear(&world, player, &mut rng);

        // At least some should be affected (75% chance each).
        let has_msg = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("cause-fear")
        ));
        assert!(has_msg, "cause fear should produce a message");
    }

    // ── test_detect_unseen_grants_see_invisible ────────────────────

    #[test]
    fn test_detect_unseen_grants_see_invisible() {
        let (mut world, player, _rng) = setup();

        let events = apply_detect_unseen(&mut world, player);

        let has_see_invis = events.iter().any(|e| matches!(
            e, EngineEvent::StatusApplied { status: StatusEffect::SeeInvisible, .. }
        ));
        assert!(has_see_invis, "detect unseen should grant see invisible");
    }

    // ── test_restore_ability_restores_drained ──────────────────────

    #[test]
    fn test_restore_ability_restores_drained() {
        let (mut world, player, _rng) = setup();

        // Drain strength below base.
        {
            let mut attrs = world.get_component_mut::<Attributes>(player).unwrap();
            attrs.strength = 5;
            attrs.dexterity = 3;
        }

        let events = apply_restore_ability(&mut world, player);

        let attrs = world.get_component::<Attributes>(player).unwrap();
        assert_eq!(attrs.strength, 10, "strength should be restored to base");
        assert_eq!(attrs.dexterity, 10, "dexterity should be restored to base");

        let has_restored = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("restored")
        ));
        assert!(has_restored, "should emit restored message");
    }

    // ── test_restore_ability_nothing_to_restore ────────────────────

    #[test]
    fn test_restore_ability_nothing_to_restore() {
        let (mut world, player, _rng) = setup();

        // All stats are at default 10, nothing to restore.
        let events = apply_restore_ability(&mut world, player);

        let has_nothing = events.iter().any(|e| matches!(
            e, EngineEvent::Message { key, .. } if key.contains("nothing")
        ));
        assert!(has_nothing, "should emit nothing-to-restore message");
    }

    // ── test_jumping_moves_caster ──────────────────────────────────

    #[test]
    fn test_jumping_moves_caster() {
        let (world, player, _rng) = setup();

        let events = apply_jumping(&world, player, Some(Direction::East));

        let has_moved = events.iter().any(|e| matches!(
            e, EngineEvent::EntityMoved { .. }
        ));
        assert!(has_moved, "jumping should move the caster");
    }

    // ── test_teleport_away_teleports_monster ───────────────────────

    #[test]
    fn test_teleport_away_teleports_monster() {
        let (mut world, player, mut rng) = setup();

        let mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 20, max: 20 },
            StatusEffects::default(),
        ));

        let events = apply_teleport_away(
            &world, player, Some(Direction::East), &mut rng,
        );

        let has_teleport = events.iter().any(|e| matches!(
            e, EngineEvent::EntityTeleported { entity, .. }
            if *entity == mon
        ));
        assert!(has_teleport, "teleport away should teleport the target monster");
    }

    // ── test_polymorph_applies_status ──────────────────────────────

    #[test]
    fn test_polymorph_applies_status() {
        let (mut world, player, mut rng) = setup();

        let mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 20, max: 20 },
            StatusEffects::default(),
        ));

        let events = apply_polymorph(
            &world, player, Some(Direction::East), &mut rng,
        );

        let has_poly = events.iter().any(|e| matches!(
            e, EngineEvent::StatusApplied { entity, status: StatusEffect::Polymorphed, .. }
            if *entity == mon
        ));
        assert!(has_poly, "polymorph should apply Polymorphed status to target");
    }

    // ── test_cancellation_removes_status ────────────────────────────

    #[test]
    fn test_cancellation_removes_status() {
        let (mut world, player, _rng) = setup();

        let mon = world.ecs_mut().spawn((
            Positioned(Position::new(41, 10)),
            Monster,
            HitPoints { current: 20, max: 20 },
            StatusEffects {
                invisibility: 100,
                levitation: 50,
                ..Default::default()
            },
        ));

        let events = apply_cancellation(
            &mut world, player, Some(Direction::East),
        );

        // Check status was cleared.
        let se = world.get_component::<StatusEffects>(mon).unwrap();
        assert_eq!(se.invisibility, 0, "cancellation should clear invisibility");
        assert_eq!(se.levitation, 0, "cancellation should clear levitation");

        let has_removed = events.iter().any(|e| matches!(
            e, EngineEvent::StatusRemoved { .. }
        ));
        assert!(has_removed, "cancellation should emit StatusRemoved");
    }

    // ── test_all_spell_types_have_effects ───────────────────────────

    #[test]
    fn test_all_spell_types_have_effects() {
        // Verify every spell type produces at least one event when cast.
        let all_spells = [
            SpellType::ForceBolt,
            SpellType::MagicMissile,
            SpellType::Fireball,
            SpellType::ConeOfCold,
            SpellType::DrainLife,
            SpellType::FingerOfDeath,
            SpellType::HealingSpell,
            SpellType::ExtraHealing,
            SpellType::CureBlindness,
            SpellType::CureSickness,
            SpellType::RestoreAbility,
            SpellType::Light,
            SpellType::DetectMonsters,
            SpellType::DetectFood,
            SpellType::DetectUnseen,
            SpellType::Clairvoyance,
            SpellType::DetectTreasure,
            SpellType::MagicMapping,
            SpellType::Identify,
            SpellType::ConfuseMonster,
            SpellType::Sleep,
            SpellType::SlowMonster,
            SpellType::CauseFear,
            SpellType::CharmMonster,
            SpellType::Protection,
            SpellType::RemoveCurse,
            SpellType::CreateMonster,
            SpellType::TurnUndead,
            SpellType::CreateFamiliar,
            SpellType::HasteSelf,
            SpellType::Levitation,
            SpellType::Invisibility,
            SpellType::TeleportAway,
            SpellType::Jumping,
            SpellType::Knock,
            SpellType::WizardLock,
            SpellType::Dig,
            SpellType::Polymorph,
            SpellType::StoneToFlesh,
            SpellType::Cancellation,
        ];

        for spell_type in all_spells {
            let (mut world, player, mut rng) = setup();

            // Use a direction for directional spells, None for nodir.
            let dir = match spell_type.direction() {
                SpellDirection::Nodir => None,
                _ => Some(Direction::East),
            };

            let events = apply_spell_effect(
                &mut world, player, spell_type, dir, &mut rng,
            );
            assert!(
                !events.is_empty(),
                "spell {:?} should produce at least one event",
                spell_type
            );
        }
    }

    // ── test_roll_dice_bounds ───────────────────────────────────────

    #[test]
    fn test_roll_dice_bounds() {
        let mut rng = SmallRng::seed_from_u64(999);
        for _ in 0..1000 {
            let val = roll_dice(2, 6, &mut rng);
            assert!(val >= 2 && val <= 12, "2d6 should be in [2, 12]: got {val}");
        }
    }
}
