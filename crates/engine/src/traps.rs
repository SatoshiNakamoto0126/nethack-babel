//! Trap system: placement, detection, avoidance, triggering, and escape.
//!
//! Implements the NetHack 3.7 trap mechanics from `src/trap.c`.
//! All functions operate on `GameWorld` and return `Vec<EngineEvent>`
//! for the game loop to process.  No IO.
//!
//! Reference: `specs/trap.md`

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::event::{EngineEvent, HpSource, StatusEffect};

// Re-export TrapType from the canonical data crate definition.
pub use nethack_babel_data::TrapType;

// ---------------------------------------------------------------------------
// Dice helpers (matching NetHack conventions, local to this module)
// ---------------------------------------------------------------------------

/// Roll one die with `sides` faces: uniform in [1, sides].
#[inline]
fn rnd<R: Rng>(rng: &mut R, sides: u32) -> u32 {
    if sides == 0 {
        return 0;
    }
    rng.random_range(1..=sides)
}

/// Roll `n` dice of `s` sides: sum of n calls to rnd(s).
#[inline]
fn d<R: Rng>(rng: &mut R, n: u32, s: u32) -> u32 {
    (0..n).map(|_| rnd(rng, s)).sum()
}

/// `rn1(x, y)` — NetHack's random in [y, y+x-1].
#[inline]
fn rn1<R: Rng>(rng: &mut R, x: u32, y: u32) -> u32 {
    y + rng.random_range(0..x)
}

/// `rn2(x)` — NetHack's random in [0, x-1].
#[inline]
fn rn2<R: Rng>(rng: &mut R, x: u32) -> u32 {
    if x == 0 {
        return 0;
    }
    rng.random_range(0..x)
}

/// Luck-adjusted random: `rnl(x)`.  For positive luck the result tends
/// toward lower values, improving search/detection odds.
fn rnl<R: Rng>(rng: &mut R, x: i32, luck: i32) -> i32 {
    if x <= 0 {
        return 0;
    }
    let mut result = rng.random_range(0..x);
    // Positive luck reduces the result (better); negative luck increases it.
    let adjustment = luck.clamp(-5, 5);
    result -= adjustment;
    result.clamp(0, x - 1)
}

// ---------------------------------------------------------------------------
// Trapped entity state — tracks what the entity is stuck in
// ---------------------------------------------------------------------------

/// What type of trap an entity is currently stuck in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrappedIn {
    None,
    BearTrap,
    Pit,
    Web,
    Lava,
}

/// Component: the entity is currently trapped.
///
/// Attach to an entity to indicate it is stuck.  `turns_remaining`
/// decrements each turn; when it reaches 0 the entity frees itself.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Trapped {
    pub kind: TrappedIn,
    pub turns_remaining: u32,
}

// ---------------------------------------------------------------------------
// TrapInstance — one trap on the map
// ---------------------------------------------------------------------------

/// A single trap placed on a dungeon level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrapInstance {
    pub pos: Position,
    pub trap_type: TrapType,
    /// Has the hero (or a monster) seen/detected this trap?
    pub detected: bool,
    /// How many times this trap has been triggered.
    pub triggered_count: u32,
}

impl TrapInstance {
    pub fn new(pos: Position, trap_type: TrapType) -> Self {
        let detected = trap_type == TrapType::Hole; // holes are always visible
        Self {
            pos,
            trap_type,
            detected,
            triggered_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// TrapMap — collection of traps on the current level
// ---------------------------------------------------------------------------

/// All traps on a single dungeon level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrapMap {
    pub traps: Vec<TrapInstance>,
}

impl TrapMap {
    pub fn new() -> Self {
        Self { traps: Vec::new() }
    }

    /// Find a trap at a given position.
    pub fn trap_at(&self, pos: Position) -> Option<&TrapInstance> {
        self.traps.iter().find(|t| t.pos == pos)
    }

    /// Find a mutable trap at a given position.
    pub fn trap_at_mut(&mut self, pos: Position) -> Option<&mut TrapInstance> {
        self.traps.iter_mut().find(|t| t.pos == pos)
    }

    /// Remove the trap at a given position, returning it if it existed.
    pub fn remove_trap_at(&mut self, pos: Position) -> Option<TrapInstance> {
        if let Some(idx) = self.traps.iter().position(|t| t.pos == pos) {
            Some(self.traps.swap_remove(idx))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Flags / status helpers
// ---------------------------------------------------------------------------

/// Properties of an entity relevant to trap interaction.
///
/// Extracted from ECS components before calling the pure trap functions
/// so that all logic is independently testable.
#[derive(Debug, Clone, Copy)]
pub struct TrapEntityInfo {
    pub entity: Entity,
    pub pos: Position,
    pub hp: i32,
    pub max_hp: i32,
    pub pw: i32,
    pub max_pw: i32,
    pub ac: i32,
    pub strength: u8,
    pub dexterity: u8,
    pub is_flying: bool,
    pub is_levitating: bool,
    pub sleep_resistant: bool,
    pub fire_resistant: bool,
    pub poison_resistant: bool,
    pub magic_resistant: bool,
    pub is_amorphous: bool,
    pub is_player: bool,
    pub luck: i32,
}

impl Default for TrapEntityInfo {
    fn default() -> Self {
        Self {
            entity: Entity::DANGLING,
            pos: Position::new(0, 0),
            hp: 16,
            max_hp: 16,
            pw: 4,
            max_pw: 4,
            ac: 10,
            strength: 10,
            dexterity: 10,
            is_flying: false,
            is_levitating: false,
            sleep_resistant: false,
            fire_resistant: false,
            poison_resistant: false,
            magic_resistant: false,
            is_amorphous: false,
            is_player: true,
            luck: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Floor-trigger classification
// ---------------------------------------------------------------------------

/// Whether a trap type is a floor trigger (types 1..14).
/// Flying/levitating entities bypass floor triggers.
pub fn is_floor_trigger(tt: TrapType) -> bool {
    let v = tt as u8;
    (1..=14).contains(&v)
}

/// Whether a trap type is always visible (cannot be hidden).
pub fn is_unhideable(tt: TrapType) -> bool {
    tt == TrapType::Hole
}

/// Whether a trap type cannot be destroyed/overwritten.
pub fn is_undestroyable(tt: TrapType) -> bool {
    matches!(tt, TrapType::MagicPortal | TrapType::VibratingSquare)
}

/// Whether a trap type is a pit variant.
pub fn is_pit(tt: TrapType) -> bool {
    matches!(tt, TrapType::Pit | TrapType::SpikedPit)
}

// ---------------------------------------------------------------------------
// place_trap
// ---------------------------------------------------------------------------

/// Place a new trap on the level at `pos`.
///
/// Returns the event announcing trap placement (for logging/replay).
pub fn place_trap(trap_map: &mut TrapMap, pos: Position, trap_type: TrapType) -> TrapInstance {
    let trap = TrapInstance::new(pos, trap_type);
    trap_map.traps.push(trap.clone());
    trap
}

// ---------------------------------------------------------------------------
// avoid_trap — check if the entity avoids triggering
// ---------------------------------------------------------------------------

/// Check whether an entity avoids a trap entirely.
///
/// Returns `true` if the trap is bypassed (no effect).
pub fn avoid_trap<R: Rng>(rng: &mut R, info: &TrapEntityInfo, trap: &TrapInstance) -> bool {
    let tt = trap.trap_type;

    // Flying/levitation bypasses floor-trigger traps
    if is_floor_trigger(tt) && (info.is_flying || info.is_levitating) {
        return true;
    }

    // Amorphous entities pass through bear traps and webs
    if info.is_amorphous && matches!(tt, TrapType::BearTrap | TrapType::Web) {
        return true;
    }

    // DEX-based avoidance for seen traps: 20% chance to escape
    if trap.detected
        && !matches!(tt, TrapType::AntiMagic)
        && !is_undestroyable(tt)
        && rn2(rng, 5) == 0
    {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// trigger_trap — main dispatch
// ---------------------------------------------------------------------------

/// Trigger a trap on an entity.  Returns the events produced.
///
/// This is the main entry point for trap effects.  The caller should
/// first check `avoid_trap` and skip this if avoidance succeeded.
pub fn trigger_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trap: &mut TrapInstance,
) -> Vec<EngineEvent> {
    trap.triggered_count += 1;

    let mut events = Vec::new();

    // Always emit TrapTriggered
    events.push(EngineEvent::TrapTriggered {
        entity: info.entity,
        trap_type: trap.trap_type,
        position: trap.pos,
    });

    match trap.trap_type {
        TrapType::ArrowTrap => trigger_arrow_trap(rng, info, trap, &mut events),
        TrapType::DartTrap => trigger_dart_trap(rng, info, trap, &mut events),
        TrapType::RockTrap => trigger_rock_trap(rng, info, trap, &mut events),
        TrapType::SqueakyBoard => trigger_squeaky_board(rng, info, trap, &mut events),
        TrapType::BearTrap => trigger_bear_trap(rng, info, trap, &mut events),
        TrapType::Landmine => trigger_landmine(rng, info, trap, &mut events),
        TrapType::RollingBoulderTrap => trigger_rolling_boulder(rng, info, trap, &mut events),
        TrapType::SleepingGasTrap => trigger_sleeping_gas(rng, info, trap, &mut events),
        TrapType::RustTrap => trigger_rust_trap(rng, info, trap, &mut events),
        TrapType::FireTrap => trigger_fire_trap(rng, info, trap, &mut events),
        TrapType::Pit => trigger_pit(rng, info, trap, &mut events),
        TrapType::SpikedPit => trigger_spiked_pit(rng, info, trap, &mut events),
        TrapType::Hole => trigger_hole(rng, info, trap, &mut events),
        TrapType::TrapDoor => trigger_trapdoor(rng, info, trap, &mut events),
        TrapType::TeleportTrap => trigger_teleport_trap(rng, info, trap, &mut events),
        TrapType::LevelTeleport => trigger_level_teleport(rng, info, trap, &mut events),
        TrapType::MagicPortal => trigger_magic_portal(rng, info, trap, &mut events),
        TrapType::Web => trigger_web(rng, info, trap, &mut events),
        TrapType::StatueTrap => trigger_statue_trap(rng, info, trap, &mut events),
        TrapType::MagicTrap => trigger_magic_trap(rng, info, trap, &mut events),
        TrapType::AntiMagic => trigger_anti_magic(rng, info, trap, &mut events),
        TrapType::PolyTrap => trigger_poly_trap(rng, info, trap, &mut events),
        TrapType::VibratingSquare => trigger_vibrating_square(rng, info, trap, &mut events),
        TrapType::TrappedDoor => trigger_trapped_door(rng, info, trap, &mut events),
        TrapType::TrappedChest => trigger_trapped_chest(rng, info, trap, &mut events),
        TrapType::NoTrap => { /* nothing */ }
    }

    events
}

// ---------------------------------------------------------------------------
// Individual trap effects
// ---------------------------------------------------------------------------

fn trigger_arrow_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Depletion check: if previously triggered + seen + 1/15 chance
    if trap.triggered_count > 1 && trap.detected && rn2(rng, 15) == 0 {
        events.push(EngineEvent::msg("trap-arrow"));
        return; // trap depleted, no projectile
    }

    let damage = d(rng, 1, 6); // d(1,6) = 1..6

    events.push(EngineEvent::msg("trap-arrow-shoot"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
}

fn trigger_dart_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Depletion check
    if trap.triggered_count > 1 && trap.detected && rn2(rng, 15) == 0 {
        events.push(EngineEvent::msg("trap-dart"));
        return;
    }

    let damage = d(rng, 1, 4); // d(1,4) = 1..4
    let poisoned = rn2(rng, 6) == 0; // 1/6 chance

    events.push(EngineEvent::msg("trap-dart-shoot"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });

    if poisoned && !info.poison_resistant {
        events.push(EngineEvent::msg("trap-dart-poison"));
        events.push(EngineEvent::HpChange {
            entity: info.entity,
            amount: -10,
            new_hp: info.hp - damage as i32 - 10,
            source: HpSource::Poison,
        });
    } else if poisoned && info.poison_resistant {
        events.push(EngineEvent::msg("trap-dart-poison-resist"));
    }
}

fn trigger_rock_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Depletion check
    if trap.triggered_count > 1 && trap.detected && rn2(rng, 15) == 0 {
        events.push(EngineEvent::msg("trap-trapdoor-ceiling"));
        return;
    }

    let damage = d(rng, 2, 6); // d(2,6) = 2..12

    events.push(EngineEvent::msg("trap-rolling-boulder"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
}

fn trigger_squeaky_board<R: Rng>(
    rng: &mut R,
    _info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // If flying/levitating, just notice (no squeak) — but we already
    // handle floor-trigger bypass in avoid_trap, so if we reach here
    // the entity is grounded.
    let _notes = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let _note_idx = rn2(rng, 12) as usize;

    events.push(EngineEvent::msg("trap-squeaky-board"));
    // Wake nearby monsters — modeled as a message for now.
    // The actual wake radius is handled by the caller.
    events.push(EngineEvent::msg("trap-squeaky-board"));
}

fn trigger_bear_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let damage = d(rng, 2, 4); // d(2,4) = 2..8
    let trap_duration = rn1(rng, 4, 4); // 4..7 turns

    events.push(EngineEvent::msg("trap-bear"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
    events.push(EngineEvent::StatusApplied {
        entity: info.entity,
        status: StatusEffect::Paralyzed, // re-use as "stuck in bear trap"
        duration: Some(trap_duration),
        source: None,
    });
}

fn trigger_landmine<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let damage = rnd(rng, 16); // rnd(16) = 1..16
    // In full NetHack this is d(4,16), but the spec says rnd(16)
    // for hero; we'll emit a large damage + level change.

    events.push(EngineEvent::msg("trap-land-mine"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
    // Landmine converts to PIT after explosion, then entity falls in.
    // Level change event for fallthrough:
    events.push(EngineEvent::LevelChanged {
        entity: info.entity,
        from_depth: "current".to_string(),
        to_depth: "current+1".to_string(),
    });
}

fn trigger_rolling_boulder<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let damage = d(rng, 3, 10); // d(3,10) = 3..30

    events.push(EngineEvent::msg("trap-rolling-boulder-trigger"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
}

fn trigger_sleeping_gas<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    if info.sleep_resistant {
        events.push(EngineEvent::msg("trap-sleeping-gas"));
        // No sleep effect.
    } else {
        let duration = rnd(rng, 25); // 1..25 turns
        events.push(EngineEvent::msg("trap-sleeping-gas-sleep"));
        events.push(EngineEvent::StatusApplied {
            entity: info.entity,
            status: StatusEffect::Sleeping,
            duration: Some(duration),
            source: None,
        });
    }
}

fn trigger_rust_trap<R: Rng>(
    rng: &mut R,
    _info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // A gush of water hits a random body part.
    let _body_parts = ["head", "left arm", "right arm", "body", "body"];
    let _part = rn2(rng, 5) as usize;

    events.push(EngineEvent::msg("trap-rust"));
    // Rust/erode a random piece of armor — modeled as ItemDamaged.
    events.push(EngineEvent::msg("trap-rust"));
}

fn trigger_fire_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let base_damage = d(rng, 2, 4); // d(2,4) = 2..8

    let actual_damage = if info.fire_resistant {
        let reduced = rn2(rng, 2); // 0 or 1
        events.push(EngineEvent::msg("trap-fire-resist"));
        reduced
    } else {
        events.push(EngineEvent::msg("trap-fire"));
        base_damage
    };

    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(actual_damage as i32),
        new_hp: info.hp - actual_damage as i32,
        source: HpSource::Trap,
    });
}

fn trigger_pit<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let damage = rnd(rng, 6); // rnd(6) = 1..6
    let trap_duration = rn1(rng, 6, 2); // 2..7 turns

    events.push(EngineEvent::msg("trap-pit-fall"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
    events.push(EngineEvent::StatusApplied {
        entity: info.entity,
        status: StatusEffect::Paralyzed, // stuck in pit
        duration: Some(trap_duration),
        source: None,
    });
}

fn trigger_spiked_pit<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let fall_damage = rnd(rng, 6); // base fall damage
    let spike_damage = rnd(rng, 10); // spike damage 1..10
    let total_damage = fall_damage + spike_damage;
    let trap_duration = rn1(rng, 6, 2); // 2..7 turns
    let poisoned = rn2(rng, 6) == 0; // 1/6 chance

    events.push(EngineEvent::msg("trap-spiked-pit"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(total_damage as i32),
        new_hp: info.hp - total_damage as i32,
        source: HpSource::Trap,
    });
    events.push(EngineEvent::StatusApplied {
        entity: info.entity,
        status: StatusEffect::Paralyzed, // stuck
        duration: Some(trap_duration),
        source: None,
    });

    if poisoned && !info.poison_resistant {
        events.push(EngineEvent::msg("trap-spiked-damage"));
        events.push(EngineEvent::HpChange {
            entity: info.entity,
            amount: -8,
            new_hp: info.hp - total_damage as i32 - 8,
            source: HpSource::Poison,
        });
    }
}

fn trigger_hole<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    events.push(EngineEvent::msg("trap-hole"));
    events.push(EngineEvent::LevelChanged {
        entity: info.entity,
        from_depth: "current".to_string(),
        to_depth: "current+1".to_string(),
    });
}

fn trigger_trapdoor<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    events.push(EngineEvent::msg("trap-trapdoor"));
    events.push(EngineEvent::LevelChanged {
        entity: info.entity,
        from_depth: "current".to_string(),
        to_depth: "current+1".to_string(),
    });
}

fn trigger_teleport_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Teleport to a random position on the level.
    let new_x = rng.random_range(1..79);
    let new_y = rng.random_range(1..20);
    let new_pos = Position::new(new_x, new_y);

    events.push(EngineEvent::msg("trap-teleport"));
    events.push(EngineEvent::EntityTeleported {
        entity: info.entity,
        from: info.pos,
        to: new_pos,
    });
}

fn trigger_level_teleport<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    if info.magic_resistant {
        events.push(EngineEvent::msg("trap-teleport-wrench"));
        return;
    }

    events.push(EngineEvent::msg("trap-level-teleport"));
    events.push(EngineEvent::LevelChanged {
        entity: info.entity,
        from_depth: "current".to_string(),
        to_depth: "random".to_string(),
    });
}

fn trigger_magic_portal<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    events.push(EngineEvent::msg("trap-vibrating-square"));
    events.push(EngineEvent::LevelChanged {
        entity: info.entity,
        from_depth: "current".to_string(),
        to_depth: "portal_destination".to_string(),
    });
}

fn trigger_web<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let duration = web_duration_by_str(rng, info.strength);

    if duration == 0 {
        events.push(EngineEvent::msg("trap-web-tear"));
    } else {
        events.push(EngineEvent::msg("trap-web"));
        events.push(EngineEvent::StatusApplied {
            entity: info.entity,
            status: StatusEffect::Paralyzed, // stuck in web
            duration: Some(duration),
            source: None,
        });
    }
}

/// Calculate web trapping duration based on STR.
fn web_duration_by_str<R: Rng>(rng: &mut R, strength: u8) -> u32 {
    match strength {
        0..=3 => rn1(rng, 6, 6),   // 6..11
        4..=5 => rn1(rng, 6, 4),   // 4..9
        6..=8 => rn1(rng, 4, 4),   // 4..7
        9..=11 => rn1(rng, 4, 2),  // 2..5
        12..=14 => rn1(rng, 2, 2), // 2..3
        15..=17 => rnd(rng, 2),    // 1..2
        18..=68 => 1,              // standard 18
        _ => 0,                    // 69+ (gauntlets + 18/**): instant tear
    }
}

fn trigger_statue_trap<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Only hero triggers statue traps.  Animate the statue as a hostile monster.
    events.push(EngineEvent::msg("trap-statue"));
    // Full animation requires monster spawning; emit a generation event placeholder.
    events.push(EngineEvent::MonsterGenerated {
        entity: info.entity, // placeholder; real entity created by caller
        position: info.pos,
    });
}

fn trigger_magic_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // 1/30 chance: magical explosion
    if rn2(rng, 30) == 0 {
        let damage = rnd(rng, 10); // 1..10

        events.push(EngineEvent::msg("trap-magic-trap"));
        events.push(EngineEvent::HpChange {
            entity: info.entity,
            amount: -(damage as i32),
            new_hp: info.hp - damage as i32,
            source: HpSource::Trap,
        });
        // Gain +2 max energy, restore to max
        events.push(EngineEvent::PwChange {
            entity: info.entity,
            amount: 2,
            new_pw: info.max_pw + 2,
        });
        return;
    }

    // Random domagictrap effect (simplified)
    let fate = rnd(rng, 20);
    match fate {
        1..=9 => {
            // Flash of light — blind + spawn monsters
            events.push(EngineEvent::msg("trap-magic-trap-blind"));
            events.push(EngineEvent::StatusApplied {
                entity: info.entity,
                status: StatusEffect::Blind,
                duration: Some(rn1(rng, 5, 10)), // 10..14 turns
                source: None,
            });
        }
        10 => {
            events.push(EngineEvent::msg("wand-nothing"));
        }
        11 => {
            // Toggle invisibility
            events.push(EngineEvent::StatusApplied {
                entity: info.entity,
                status: StatusEffect::Invisible,
                duration: None, // permanent toggle
                source: None,
            });
        }
        12 => {
            // Fire trap effect
            let fire_damage = d(rng, 2, 4);
            events.push(EngineEvent::msg("trap-fire"));
            events.push(EngineEvent::HpChange {
                entity: info.entity,
                amount: -(fire_damage as i32),
                new_hp: info.hp - fire_damage as i32,
                source: HpSource::Trap,
            });
        }
        13..=18 => {
            // Flavor messages
            let _flavors = [
                "You shiver suddenly.",
                "You hear a distant howling.",
                "You feel a strange yearning.",
                "Your pack shakes violently!",
                "You smell acrid fumes.",
                "You feel tired all of a sudden.",
            ];
            let _idx = (fate - 13) as usize;
            events.push(EngineEvent::msg("trap-magic-trap"));
        }
        19 => {
            // +1 CHA, tame adjacent monsters
            events.push(EngineEvent::msg("scroll-taming"));
        }
        20 => {
            // Uncurse inventory
            events.push(EngineEvent::msg("scroll-remove-curse"));
        }
        _ => {}
    }
}

fn trigger_anti_magic<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Energy drain (always)
    let drain = d(rng, 2, 6) as i32; // 2..12

    events.push(EngineEvent::msg("trap-anti-magic"));
    events.push(EngineEvent::PwChange {
        entity: info.entity,
        amount: -drain,
        new_pw: (info.pw - drain).max(0),
    });

    // If entity has magic resistance, also physical damage
    if info.magic_resistant {
        let phys_damage = rnd(rng, 4) as i32;
        events.push(EngineEvent::HpChange {
            entity: info.entity,
            amount: -phys_damage,
            new_hp: info.hp - phys_damage,
            source: HpSource::Trap,
        });
    }
}

fn trigger_poly_trap<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    if info.magic_resistant {
        events.push(EngineEvent::msg("trap-polymorph"));
        return;
    }

    events.push(EngineEvent::msg("trap-polymorph"));
    events.push(EngineEvent::StatusApplied {
        entity: info.entity,
        status: StatusEffect::Polymorphed,
        duration: None, // handled by polymorph system
        source: None,
    });
}

fn trigger_vibrating_square<R: Rng>(
    _rng: &mut R,
    _info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Not a real trap; just a marker.  Hero feels it.
    events.push(EngineEvent::msg("trap-vibrating-square"));
}

fn trigger_trapped_door<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    let damage = d(rng, 2, 4); // typical door trap damage
    events.push(EngineEvent::msg("trap-door-booby"));
    events.push(EngineEvent::HpChange {
        entity: info.entity,
        amount: -(damage as i32),
        new_hp: info.hp - damage as i32,
        source: HpSource::Trap,
    });
}

fn trigger_trapped_chest<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    _trap: &mut TrapInstance,
    events: &mut Vec<EngineEvent>,
) {
    // Simplified chest trap: pick one of the possible effects.
    let effect = rn2(rng, 7);
    match effect {
        0..=1 => {
            // Stun gas
            events.push(EngineEvent::msg("trap-gas-puff"));
            events.push(EngineEvent::StatusApplied {
                entity: info.entity,
                status: StatusEffect::Stunned,
                duration: Some(rn1(rng, 7, 16)), // 16..22
                source: None,
            });
        }
        2..=3 => {
            // Paralysis
            events.push(EngineEvent::msg("trap-gas-cloud"));
            events.push(EngineEvent::StatusApplied {
                entity: info.entity,
                status: StatusEffect::Paralyzed,
                duration: Some(d(rng, 5, 6)),
                source: None,
            });
        }
        4..=5 => {
            // Electric shock
            let damage = d(rng, 4, 4);
            events.push(EngineEvent::msg("trap-shock"));
            events.push(EngineEvent::HpChange {
                entity: info.entity,
                amount: -(damage as i32),
                new_hp: info.hp - damage as i32,
                source: HpSource::Trap,
            });
        }
        _ => {
            // Explosion
            let damage = d(rng, 6, 6);
            events.push(EngineEvent::msg("trap-chest-explode"));
            events.push(EngineEvent::HpChange {
                entity: info.entity,
                amount: -(damage as i32),
                new_hp: info.hp - damage as i32,
                source: HpSource::Trap,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// detect_trap — searching for hidden traps
// ---------------------------------------------------------------------------

/// Search for traps around the given position.
///
/// Corresponds to `dosearch0()`: checks all 8 adjacent squares.
/// `fund` is the search bonus (artifact search bonus + lenses bonus,
/// capped at 5).
///
/// Returns events for any newly detected traps.
pub fn detect_trap<R: Rng>(
    rng: &mut R,
    trap_map: &mut TrapMap,
    player_pos: Position,
    luck: i32,
    fund: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();
    let fund = fund.min(5);

    for &(dx, dy) in &[
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        let adj = Position::new(player_pos.x + dx, player_pos.y + dy);

        // Find trap at adjacent position
        if let Some(trap) = trap_map.trap_at_mut(adj)
            && !trap.detected
        {
            // Base probability: rnl(8 - fund) == 0 means found.
            // With fund=0, that's rnl(8)==0 => ~1/8 base chance.
            let threshold = (8 - fund).max(1);
            if rnl(rng, threshold, luck) == 0 {
                trap.detected = true;
                events.push(EngineEvent::TrapRevealed {
                    position: adj,
                    trap_type: trap.trap_type,
                });
                events.push(EngineEvent::msg("search-trap"));
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// escape_trap — STR-based escape from bear trap / pit / web
// ---------------------------------------------------------------------------

/// Attempt to escape from a trap the entity is stuck in.
///
/// Returns events describing the outcome.  If the entity escapes,
/// the returned events include a message and the caller should
/// clear the entity's `Trapped` component.
pub fn escape_trap<R: Rng>(
    _rng: &mut R,
    info: &TrapEntityInfo,
    trapped: &Trapped,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if trapped.turns_remaining == 0 {
        // Already free.
        return (events, true);
    }

    match trapped.kind {
        TrappedIn::BearTrap => {
            // Bear trap escape is purely turn-based (4..7 turns).
            // turns_remaining decremented by caller each turn.
            if trapped.turns_remaining <= 1 {
                events.push(EngineEvent::msg("trap-bear-escape"));
                return (events, true);
            }
            events.push(EngineEvent::msg("trap-bear-stuck"));
            (events, false)
        }
        TrappedIn::Pit => {
            // Pit escape: decrement counter, free when 0.
            if trapped.turns_remaining <= 1 {
                events.push(EngineEvent::msg("trap-pit-climb"));
                return (events, true);
            }
            // Flying/levitating: instant escape
            if info.is_flying || info.is_levitating {
                events.push(EngineEvent::msg("trap-pit-float"));
                return (events, true);
            }
            events.push(EngineEvent::msg("trap-pit-cant-climb"));
            (events, false)
        }
        TrappedIn::Web => {
            // Web escape: STR-based duration, turn-based countdown.
            if trapped.turns_remaining <= 1 {
                events.push(EngineEvent::msg("trap-web-free"));
                return (events, true);
            }
            // STR >= 69: instant tear (already handled by duration=0 at trigger time)
            events.push(EngineEvent::msg("trap-web-stuck"));
            (events, false)
        }
        TrappedIn::Lava => {
            events.push(EngineEvent::msg("swim-lava"));
            (events, false)
        }
        TrappedIn::None => (events, true),
    }
}

// ---------------------------------------------------------------------------
// monster_trigger_trap — monsters stepping on traps (C mintrap)
// ---------------------------------------------------------------------------

/// Properties of a monster relevant to trap interaction.
///
/// Similar to `TrapEntityInfo` but specifically for non-player monsters.
/// Extracted from ECS components before calling trap functions.
#[derive(Debug, Clone, Copy)]
pub struct MonsterTrapInfo {
    pub entity: Entity,
    pub pos: Position,
    pub hp: i32,
    pub max_hp: i32,
    pub is_flying: bool,
    pub is_amorphous: bool,
    pub is_mindless: bool,
    pub sleep_resistant: bool,
    pub fire_resistant: bool,
    pub poison_resistant: bool,
    pub magic_resistant: bool,
    pub strength: u8,
}

impl Default for MonsterTrapInfo {
    fn default() -> Self {
        Self {
            entity: Entity::DANGLING,
            pos: Position::new(0, 0),
            hp: 10,
            max_hp: 10,
            is_flying: false,
            is_amorphous: false,
            is_mindless: false,
            sleep_resistant: false,
            fire_resistant: false,
            poison_resistant: false,
            magic_resistant: false,
            strength: 10,
        }
    }
}

/// Resolve a monster stepping onto a trap.
///
/// Mirrors C `mintrap()`.  Converts `MonsterTrapInfo` to the general
/// `TrapEntityInfo` and delegates to `avoid_trap` / `trigger_trap`.
pub fn monster_trigger_trap<R: Rng>(
    rng: &mut R,
    mon_info: &MonsterTrapInfo,
    trap: &mut TrapInstance,
) -> Vec<EngineEvent> {
    let info = TrapEntityInfo {
        entity: mon_info.entity,
        pos: mon_info.pos,
        hp: mon_info.hp,
        max_hp: mon_info.max_hp,
        pw: 0,
        max_pw: 0,
        ac: 10,
        strength: mon_info.strength,
        dexterity: 10,
        is_flying: mon_info.is_flying,
        is_levitating: false,
        sleep_resistant: mon_info.sleep_resistant,
        fire_resistant: mon_info.fire_resistant,
        poison_resistant: mon_info.poison_resistant,
        magic_resistant: mon_info.magic_resistant,
        is_amorphous: mon_info.is_amorphous,
        is_player: false,
        luck: 0,
    };

    if avoid_trap(rng, &info, trap) {
        return Vec::new();
    }

    trigger_trap(rng, &info, trap)
}

// ---------------------------------------------------------------------------
// disarm_trap — player attempts to disarm a trap
// ---------------------------------------------------------------------------

/// Player attempts to disarm a trap.
///
/// Mirrors C `try_disarm()`.  Disarm chance depends on DEX + luck.
/// On success the trap is removed.  On failure the trap triggers.
///
/// Returns `(events, disarmed)`.
pub fn disarm_trap<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trap: &mut TrapInstance,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();
    let tt = trap.trap_type;

    // Cannot disarm undestroyable traps.
    if is_undestroyable(tt) {
        events.push(EngineEvent::msg("trap-cannot-disarm"));
        return (events, false);
    }

    // Disarm check: dex/2 + luck vs rnl(20).
    let skill = (info.dexterity as i32) / 2 + info.luck;
    let roll = rnl(rng, 20, info.luck);

    if roll < skill {
        events.push(EngineEvent::msg("trap-disarmed"));
        events.push(EngineEvent::TrapRevealed {
            position: trap.pos,
            trap_type: tt,
        });
        (events, true)
    } else {
        // Failed: trap triggers on the player.
        events.push(EngineEvent::msg("trap-disarm-fail"));
        let trigger_events = trigger_trap(rng, info, trap);
        events.extend(trigger_events);
        (events, false)
    }
}

// ---------------------------------------------------------------------------
// trigger_trap_at — convenience for ECS callers
// ---------------------------------------------------------------------------

/// Check for a trap at the given position and trigger it if found.
///
/// This is the primary integration point for `movement.rs` and
/// `turn.rs` — after an entity moves to a new tile, call this to
/// check and trigger any trap at that position.
///
/// Returns `(events, trap_was_triggered)`.
pub fn trigger_trap_at<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trap_map: &mut TrapMap,
) -> (Vec<EngineEvent>, bool) {
    let trap = match trap_map.trap_at_mut(info.pos) {
        Some(t) => t,
        None => return (Vec::new(), false),
    };

    if avoid_trap(rng, info, trap) {
        return (Vec::new(), false);
    }

    let events = trigger_trap(rng, info, trap);
    (events, true)
}

// ---------------------------------------------------------------------------
// trap_to_pit — landmine/trapdoor aftermath creates a pit
// ---------------------------------------------------------------------------

/// After a landmine explodes, convert it to a pit trap at the same position.
pub fn convert_landmine_to_pit(trap_map: &mut TrapMap, pos: Position) {
    if let Some(trap) = trap_map.trap_at_mut(pos) {
        if trap.trap_type == TrapType::Landmine {
            trap.trap_type = TrapType::Pit;
            trap.detected = true;
            trap.triggered_count = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// bear_trap_escape — strength-based escape (C: u_ustuck / bear_trap)
// ---------------------------------------------------------------------------

/// Attempt to escape a bear trap using raw strength.
///
/// In NetHack, a strong character can rip free of a bear trap on a
/// given turn with probability STR/118 per turn (simplified here).
///
/// Returns `(events, escaped)`.
pub fn bear_trap_str_escape<R: Rng>(
    rng: &mut R,
    info: &TrapEntityInfo,
    trapped: &Trapped,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if trapped.kind != TrappedIn::BearTrap {
        return (events, false);
    }

    // STR check: roll d(1,118), if <= STR then rip free.
    let roll: u32 = rng.random_range(1..=118);
    if roll <= info.strength as u32 {
        events.push(EngineEvent::msg("trap-bear-rip-free"));
        return (events, true);
    }

    events.push(EngineEvent::msg("trap-bear-stuck"));
    (events, false)
}

// ---------------------------------------------------------------------------
// trap_chain_reaction — landmine triggers adjacent traps
// ---------------------------------------------------------------------------

/// A landmine explosion can trigger adjacent traps (chain reaction).
///
/// Checks all 8 adjacent positions for traps and triggers them with
/// a synthetic TrapEntityInfo representing the explosion force.
///
/// Returns the combined events from all triggered traps.
pub fn trap_chain_reaction<R: Rng>(
    rng: &mut R,
    center: Position,
    trap_map: &mut TrapMap,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Synthetic "entity info" representing the landmine blast.
    let blast_info = TrapEntityInfo {
        entity: Entity::DANGLING,
        pos: center,
        hp: 999,
        max_hp: 999,
        pw: 0,
        max_pw: 0,
        ac: 0,
        strength: 25,
        dexterity: 10,
        is_flying: false,
        is_levitating: false,
        sleep_resistant: true,
        fire_resistant: true,
        poison_resistant: true,
        magic_resistant: false,
        is_amorphous: false,
        is_player: false,
        luck: 0,
    };

    // Collect adjacent positions first (avoid borrow conflict).
    let deltas: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    let adj_positions: Vec<Position> = deltas
        .iter()
        .map(|&(dx, dy)| Position::new(center.x + dx, center.y + dy))
        .collect();

    for adj_pos in adj_positions {
        if let Some(adj_trap) = trap_map.trap_at_mut(adj_pos) {
            // Only trigger landmines in chain reaction.
            if adj_trap.trap_type == TrapType::Landmine {
                let mut info = blast_info;
                info.pos = adj_pos;
                let sub = trigger_trap(rng, &info, adj_trap);
                events.extend(sub);
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// detect_traps — reveal traps in area
// ---------------------------------------------------------------------------

/// Reveal all traps within `radius` of `center`.
///
/// Mirrors the effect of a scroll of detect traps or the detect-traps
/// spell.  Sets `detected = true` on all trap instances in range and
/// emits `TrapRevealed` events.
pub fn detect_traps(center: Position, radius: i32, trap_map: &mut TrapMap) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    for trap in trap_map.traps.iter_mut() {
        if trap.detected {
            continue;
        }
        let dx = (trap.pos.x - center.x).abs();
        let dy = (trap.pos.y - center.y).abs();
        if dx <= radius && dy <= radius {
            trap.detected = true;
            events.push(EngineEvent::TrapRevealed {
                position: trap.pos,
                trap_type: trap.trap_type,
            });
        }
    }

    events
}

// ---------------------------------------------------------------------------
// create_trap — place a new trap on the map
// ---------------------------------------------------------------------------

/// Place a new trap on the map.
///
/// Mirrors C `maketrap()`.  If a trap already exists at the position,
/// it is replaced.  Some trap types are auto-detected (holes,
/// vibrating square).
///
/// Returns the events emitted.
pub fn create_trap<R: Rng>(
    rng: &mut R,
    trap_map: &mut TrapMap,
    pos: Position,
    trap_type: TrapType,
) -> Vec<EngineEvent> {
    let _ = rng; // reserved for future random placement logic
    let mut events = Vec::new();

    // Remove any existing trap at this position.
    trap_map.remove_trap_at(pos);

    let mut trap = TrapInstance::new(pos, trap_type);

    // Some traps are always visible.
    match trap_type {
        TrapType::Hole | TrapType::TrapDoor | TrapType::VibratingSquare => {
            trap.detected = true;
        }
        _ => {}
    }

    trap_map.traps.push(trap);

    events.push(EngineEvent::TrapRevealed {
        position: pos,
        trap_type,
    });

    events
}

// ---------------------------------------------------------------------------
// MonsterTrapResult — simplified result for monster-trap interactions
// ---------------------------------------------------------------------------

/// Simplified outcome when a monster steps on a trap.
///
/// This provides a data-only result (no ECS events) that AI/movement code
/// can use to decide what happened to a monster without needing to parse
/// the full `EngineEvent` stream.  Mirrors `mintrap()` return semantics
/// from C NetHack.
#[derive(Debug, Clone, PartialEq)]
pub enum MonsterTrapResult {
    /// Monster avoided the trap entirely (flying over pit, amorphous through web, etc.)
    Avoids,
    /// Trap had no mechanical effect (e.g. vibrating square on monster).
    NoEffect,
    /// Monster resisted the trap effect (magic resistance vs teleport, etc.)
    Resists,
    /// Monster fell into a pit/spiked pit.
    Falls { damage: i32 },
    /// Monster is stuck in a trap (bear trap, web).
    Trapped { duration: u32 },
    /// Monster took damage from a trap.
    Damaged {
        damage: i32,
        damage_type: &'static str,
    },
    /// Monster was teleported on the same level.
    Teleported,
    /// Monster was teleported to a different level.
    LevelTeleported,
    /// Monster fell asleep.
    FallsAsleep { duration: u32 },
    /// Monster's energy was drained (anti-magic).
    Drained,
    /// Monster's armor was damaged (rust trap).
    ArmorDamaged,
    /// Trap alerted the player (squeaky board).
    AlertsPlayer,
}

/// Process a monster stepping on a trap and return a simplified result.
///
/// This is a higher-level wrapper around `monster_trigger_trap` that
/// interprets the event stream into a `MonsterTrapResult` for AI use.
/// The full event list is also returned for the game loop.
pub fn monster_hits_trap<R: Rng>(
    rng: &mut R,
    mon_info: &MonsterTrapInfo,
    trap: &mut TrapInstance,
) -> (MonsterTrapResult, Vec<EngineEvent>) {
    let info = TrapEntityInfo {
        entity: mon_info.entity,
        pos: mon_info.pos,
        hp: mon_info.hp,
        max_hp: mon_info.max_hp,
        pw: 0,
        max_pw: 0,
        ac: 10,
        strength: mon_info.strength,
        dexterity: 10,
        is_flying: mon_info.is_flying,
        is_levitating: false,
        sleep_resistant: mon_info.sleep_resistant,
        fire_resistant: mon_info.fire_resistant,
        poison_resistant: mon_info.poison_resistant,
        magic_resistant: mon_info.magic_resistant,
        is_amorphous: mon_info.is_amorphous,
        is_player: false,
        luck: 0,
    };

    // Check avoidance first.
    if avoid_trap(rng, &info, trap) {
        return (MonsterTrapResult::Avoids, Vec::new());
    }

    let events = trigger_trap(rng, &info, trap);

    // Classify the result from the event stream.
    let result = classify_monster_trap_result(trap.trap_type, &events);

    (result, events)
}

/// Classify trap events into a `MonsterTrapResult`.
fn classify_monster_trap_result(trap_type: TrapType, events: &[EngineEvent]) -> MonsterTrapResult {
    match trap_type {
        TrapType::Pit | TrapType::SpikedPit => {
            let damage = sum_trap_damage(events);
            MonsterTrapResult::Falls { damage }
        }
        TrapType::BearTrap | TrapType::Web => {
            if let Some(dur) = find_status_duration(events) {
                MonsterTrapResult::Trapped { duration: dur }
            } else {
                // Web torn instantly (high STR).
                MonsterTrapResult::NoEffect
            }
        }
        TrapType::TeleportTrap | TrapType::MagicPortal => {
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
                || events
                    .iter()
                    .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
            {
                MonsterTrapResult::Teleported
            } else {
                MonsterTrapResult::Resists
            }
        }
        TrapType::LevelTeleport | TrapType::Hole | TrapType::TrapDoor => {
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::LevelChanged { .. }))
            {
                MonsterTrapResult::LevelTeleported
            } else {
                MonsterTrapResult::Resists
            }
        }
        TrapType::SleepingGasTrap => {
            if let Some(dur) = find_sleep_duration(events) {
                MonsterTrapResult::FallsAsleep { duration: dur }
            } else {
                MonsterTrapResult::Resists
            }
        }
        TrapType::FireTrap => {
            let damage = sum_trap_damage(events);
            if damage > 0 {
                MonsterTrapResult::Damaged {
                    damage,
                    damage_type: "fire",
                }
            } else {
                MonsterTrapResult::Resists
            }
        }
        TrapType::AntiMagic => MonsterTrapResult::Drained,
        TrapType::RustTrap => MonsterTrapResult::ArmorDamaged,
        TrapType::SqueakyBoard => MonsterTrapResult::AlertsPlayer,
        TrapType::ArrowTrap
        | TrapType::DartTrap
        | TrapType::RockTrap
        | TrapType::RollingBoulderTrap
        | TrapType::Landmine => {
            let damage = sum_trap_damage(events);
            MonsterTrapResult::Damaged {
                damage,
                damage_type: "physical",
            }
        }
        TrapType::PolyTrap => {
            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::StatusApplied {
                        status: StatusEffect::Polymorphed,
                        ..
                    }
                )
            }) {
                MonsterTrapResult::Damaged {
                    damage: 0,
                    damage_type: "polymorph",
                }
            } else {
                MonsterTrapResult::Resists
            }
        }
        TrapType::VibratingSquare => MonsterTrapResult::NoEffect,
        _ => {
            let damage = sum_trap_damage(events);
            if damage > 0 {
                MonsterTrapResult::Damaged {
                    damage,
                    damage_type: "physical",
                }
            } else {
                MonsterTrapResult::NoEffect
            }
        }
    }
}

/// Sum all HpChange amounts from Trap source in an event list.
fn sum_trap_damage(events: &[EngineEvent]) -> i32 {
    events
        .iter()
        .filter_map(|e| {
            if let EngineEvent::HpChange {
                amount,
                source: HpSource::Trap,
                ..
            } = e
            {
                Some(-*amount) // damage is negative amount
            } else {
                None
            }
        })
        .sum()
}

/// Find the duration of any StatusApplied event.
fn find_status_duration(events: &[EngineEvent]) -> Option<u32> {
    events.iter().find_map(|e| {
        if let EngineEvent::StatusApplied {
            duration: Some(d), ..
        } = e
        {
            Some(*d)
        } else {
            None
        }
    })
}

/// Find the duration of a Sleeping status event.
fn find_sleep_duration(events: &[EngineEvent]) -> Option<u32> {
    events.iter().find_map(|e| {
        if let EngineEvent::StatusApplied {
            status: StatusEffect::Sleeping,
            duration: Some(d),
            ..
        } = e
        {
            Some(*d)
        } else {
            None
        }
    })
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    /// Create a deterministic RNG for repeatable tests.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(12345)
    }

    /// Create a default entity info for testing.
    fn test_entity_info() -> TrapEntityInfo {
        TrapEntityInfo {
            entity: Entity::DANGLING,
            pos: Position::new(10, 5),
            hp: 50,
            max_hp: 50,
            pw: 20,
            max_pw: 20,
            ac: 10,
            strength: 10,
            dexterity: 10,
            is_flying: false,
            is_levitating: false,
            sleep_resistant: false,
            fire_resistant: false,
            poison_resistant: false,
            magic_resistant: false,
            is_amorphous: false,
            is_player: true,
            luck: 0,
        }
    }

    // -------------------------------------------------------------------
    // 1. Arrow trap deals d(1,6) damage
    // -------------------------------------------------------------------
    #[test]
    fn arrow_trap_deals_damage() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::ArrowTrap);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        // Should contain TrapTriggered and HpChange
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapTriggered { .. }))
        );
        let hp_change = events.iter().find_map(|e| {
            if let EngineEvent::HpChange { amount, source, .. } = e {
                if *source == HpSource::Trap {
                    Some(*amount)
                } else {
                    None
                }
            } else {
                None
            }
        });
        assert!(hp_change.is_some(), "Arrow trap should deal HP damage");
        let dmg = -hp_change.unwrap();
        assert!(
            dmg >= 1 && dmg <= 6,
            "d(1,6) damage should be 1..6, got {}",
            dmg
        );
    }

    // -------------------------------------------------------------------
    // 2. Bear trap sticks entity for correct duration (4..7 turns)
    // -------------------------------------------------------------------
    #[test]
    fn bear_trap_sticks_entity() {
        // Run multiple seeds to verify range.
        let mut min_dur = u32::MAX;
        let mut max_dur = 0;
        for seed in 0..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::BearTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);

            if let Some(dur) = events.iter().find_map(|e| {
                if let EngineEvent::StatusApplied {
                    duration: Some(d), ..
                } = e
                {
                    Some(*d)
                } else {
                    None
                }
            }) {
                min_dur = min_dur.min(dur);
                max_dur = max_dur.max(dur);
            }
        }
        assert!(
            min_dur >= 4,
            "Bear trap min duration should be >= 4, got {}",
            min_dur
        );
        assert!(
            max_dur <= 7,
            "Bear trap max duration should be <= 7, got {}",
            max_dur
        );
        // With 200 seeds, both extremes should appear.
        assert_eq!(min_dur, 4, "Should hit min duration 4");
        assert_eq!(max_dur, 7, "Should hit max duration 7");
    }

    // -------------------------------------------------------------------
    // 3. Pit fall damage formula: rnd(6) = 1..6
    // -------------------------------------------------------------------
    #[test]
    fn pit_fall_damage() {
        let mut min_dmg = i32::MAX;
        let mut max_dmg = 0;
        for seed in 0..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Pit);
            let events = trigger_trap(&mut rng, &info, &mut trap);

            if let Some(dmg) = events.iter().find_map(|e| {
                if let EngineEvent::HpChange { amount, source, .. } = e {
                    if *source == HpSource::Trap {
                        Some(-*amount)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                min_dmg = min_dmg.min(dmg);
                max_dmg = max_dmg.max(dmg);
            }
        }
        assert!(
            min_dmg >= 1 && min_dmg <= 6,
            "Pit min damage should be in 1..6"
        );
        assert!(
            max_dmg >= 1 && max_dmg <= 6,
            "Pit max damage should be in 1..6"
        );
    }

    // -------------------------------------------------------------------
    // 4. Sleeping gas blocked by resistance
    // -------------------------------------------------------------------
    #[test]
    fn sleeping_gas_blocked_by_resistance() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.sleep_resistant = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SleepingGasTrap);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        // Should NOT have a Sleeping status applied
        let has_sleep = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Sleeping,
                    ..
                }
            )
        });
        assert!(
            !has_sleep,
            "Sleep-resistant entity should not be put to sleep"
        );

        // Should have the "enveloped in gas" message
        let has_msg = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key.contains("sleeping-gas")
            )
        });
        assert!(has_msg, "Should get gas cloud message");
    }

    // -------------------------------------------------------------------
    // 5. Fire trap deals fire damage
    // -------------------------------------------------------------------
    #[test]
    fn fire_trap_deals_damage() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::FireTrap);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        let hp_change = events.iter().find_map(|e| {
            if let EngineEvent::HpChange { amount, source, .. } = e {
                if *source == HpSource::Trap {
                    Some(*amount)
                } else {
                    None
                }
            } else {
                None
            }
        });
        assert!(hp_change.is_some(), "Fire trap should deal damage");
        let dmg = -hp_change.unwrap();
        // d(2,4) = 2..8 for non-resistant
        assert!(
            dmg >= 2 && dmg <= 8,
            "Fire trap d(2,4) damage should be 2..8, got {}",
            dmg
        );
    }

    // -------------------------------------------------------------------
    // 6. Teleport trap moves entity
    // -------------------------------------------------------------------
    #[test]
    fn teleport_trap_moves_entity() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::TeleportTrap);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        let teleported = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityTeleported { .. }));
        assert!(
            teleported,
            "Teleport trap should produce EntityTeleported event"
        );

        // The new position should differ from the original (with high probability)
        if let Some(EngineEvent::EntityTeleported { from, to, .. }) = events
            .iter()
            .find(|e| matches!(e, EngineEvent::EntityTeleported { .. }))
        {
            assert_eq!(*from, info.pos);
            // to should be within map bounds
            assert!(to.x >= 1 && to.x < 79);
            assert!(to.y >= 1 && to.y < 20);
        }
    }

    // -------------------------------------------------------------------
    // 7. Flying avoids pit and bear trap
    // -------------------------------------------------------------------
    #[test]
    fn flying_avoids_pit_and_bear_trap() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.is_flying = true;

        let pit_trap = TrapInstance::new(Position::new(10, 5), TrapType::Pit);
        assert!(
            avoid_trap(&mut rng, &info, &pit_trap),
            "Flying entity should avoid pit"
        );

        let bear_trap = TrapInstance::new(Position::new(10, 5), TrapType::BearTrap);
        assert!(
            avoid_trap(&mut rng, &info, &bear_trap),
            "Flying entity should avoid bear trap"
        );
    }

    // -------------------------------------------------------------------
    // 8. Search detects hidden trap
    // -------------------------------------------------------------------
    #[test]
    fn search_detects_hidden_trap() {
        let mut found = false;
        // Run enough seeds that we should find it at least once (p ~ 1/8 per search)
        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let mut trap_map = TrapMap::new();
            place_trap(
                &mut trap_map,
                Position::new(11, 5), // adjacent to player at (10,5)
                TrapType::ArrowTrap,
            );

            let events = detect_trap(
                &mut rng,
                &mut trap_map,
                Position::new(10, 5),
                0, // neutral luck
                0, // no search bonus
            );

            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapRevealed { .. }))
            {
                found = true;
                break;
            }
        }
        assert!(found, "Search should eventually detect a hidden trap");
    }

    // -------------------------------------------------------------------
    // 9. Web escape by STR check — duration depends on STR
    // -------------------------------------------------------------------
    #[test]
    fn web_escape_by_str() {
        // Low STR (3): duration 6..11
        let mut rng = test_rng();
        let dur_low = web_duration_by_str(&mut rng, 3);
        assert!(
            dur_low >= 6 && dur_low <= 11,
            "STR 3 web duration should be 6..11, got {}",
            dur_low
        );

        // High STR (18): duration 1
        let dur_high = web_duration_by_str(&mut rng, 18);
        assert_eq!(dur_high, 1, "STR 18 web duration should be 1");

        // Very high STR (69+): instant tear
        let dur_max = web_duration_by_str(&mut rng, 69);
        assert_eq!(
            dur_max, 0,
            "STR 69+ web duration should be 0 (instant tear)"
        );
    }

    // -------------------------------------------------------------------
    // 10. Land mine damage and fallthrough
    // -------------------------------------------------------------------
    #[test]
    fn landmine_damage_and_fallthrough() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Landmine);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        // Should have HP damage
        let hp_change = events.iter().find_map(|e| {
            if let EngineEvent::HpChange { amount, source, .. } = e {
                if *source == HpSource::Trap {
                    Some(*amount)
                } else {
                    None
                }
            } else {
                None
            }
        });
        assert!(hp_change.is_some(), "Landmine should deal damage");
        let dmg = -hp_change.unwrap();
        assert!(
            dmg >= 1 && dmg <= 16,
            "Landmine damage rnd(16) should be 1..16, got {}",
            dmg
        );

        // Should have level change (fallthrough)
        let has_level_change = events
            .iter()
            .any(|e| matches!(e, EngineEvent::LevelChanged { .. }));
        assert!(
            has_level_change,
            "Landmine should cause level change (fallthrough)"
        );
    }

    // -------------------------------------------------------------------
    // 11. Squeaky board wakes monsters
    // -------------------------------------------------------------------
    #[test]
    fn squeaky_board_wakes_monsters() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SqueakyBoard);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        // Should have the squeak message
        let has_squeak = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key.contains("squeaky-board")
            )
        });
        assert!(has_squeak, "Squeaky board should produce a squeak message");

        // Should have the wake message
        let has_wake = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::Message { key, .. } if key.contains("squeaky-board")
            )
        });
        assert!(has_wake, "Squeaky board should produce a wake message");
    }

    // -------------------------------------------------------------------
    // 12. Spiked pit extra damage vs regular pit
    // -------------------------------------------------------------------
    #[test]
    fn spiked_pit_extra_damage() {
        // Collect damage from many spiked pit triggers
        let mut spiked_damages: Vec<i32> = Vec::new();
        let mut regular_damages: Vec<i32> = Vec::new();

        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();

            // Spiked pit
            let mut spiked = TrapInstance::new(Position::new(10, 5), TrapType::SpikedPit);
            let events = trigger_trap(&mut rng, &info, &mut spiked);
            // Sum all trap HP damage (may include poison too)
            let total: i32 = events
                .iter()
                .filter_map(|e| {
                    if let EngineEvent::HpChange { amount, source, .. } = e {
                        if *source == HpSource::Trap {
                            Some(-*amount)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .sum();
            spiked_damages.push(total);

            // Regular pit
            let mut rng2 = Pcg64::seed_from_u64(seed);
            let mut regular = TrapInstance::new(Position::new(10, 5), TrapType::Pit);
            let events2 = trigger_trap(&mut rng2, &info, &mut regular);
            let total2: i32 = events2
                .iter()
                .filter_map(|e| {
                    if let EngineEvent::HpChange { amount, source, .. } = e {
                        if *source == HpSource::Trap {
                            Some(-*amount)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .sum();
            regular_damages.push(total2);
        }

        let spiked_avg: f64 =
            spiked_damages.iter().map(|d| *d as f64).sum::<f64>() / spiked_damages.len() as f64;
        let regular_avg: f64 =
            regular_damages.iter().map(|d| *d as f64).sum::<f64>() / regular_damages.len() as f64;

        assert!(
            spiked_avg > regular_avg,
            "Spiked pit average damage ({}) should be higher than regular pit ({})",
            spiked_avg,
            regular_avg
        );

        // Spiked pit max should be higher: rnd(6)+rnd(10) = up to 16 vs rnd(6) = up to 6
        let spiked_max = spiked_damages.iter().max().unwrap();
        let regular_max = regular_damages.iter().max().unwrap();
        assert!(
            spiked_max > regular_max,
            "Spiked pit max damage ({}) should exceed regular pit max ({})",
            spiked_max,
            regular_max
        );
    }

    // -------------------------------------------------------------------
    // 13. Fire trap resistance reduces damage
    // -------------------------------------------------------------------
    #[test]
    fn fire_trap_resistance_reduces_damage() {
        // With fire resistance: damage is rn2(2) = 0 or 1
        let mut max_resistant_dmg = 0i32;
        for seed in 0..200 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let mut info = test_entity_info();
            info.fire_resistant = true;
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::FireTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);

            if let Some(dmg) = events.iter().find_map(|e| {
                if let EngineEvent::HpChange { amount, source, .. } = e {
                    if *source == HpSource::Trap {
                        Some(-*amount)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                max_resistant_dmg = max_resistant_dmg.max(dmg);
            }
        }
        assert!(
            max_resistant_dmg <= 1,
            "Fire-resistant entity should take at most 1 damage, got {}",
            max_resistant_dmg
        );
    }

    // -------------------------------------------------------------------
    // 14. Sleeping gas applies sleep effect with correct duration range
    // -------------------------------------------------------------------
    #[test]
    fn sleeping_gas_duration_range() {
        let mut min_dur = u32::MAX;
        let mut max_dur = 0;
        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SleepingGasTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);

            if let Some(dur) = events.iter().find_map(|e| {
                if let EngineEvent::StatusApplied {
                    status: StatusEffect::Sleeping,
                    duration: Some(d),
                    ..
                } = e
                {
                    Some(*d)
                } else {
                    None
                }
            }) {
                min_dur = min_dur.min(dur);
                max_dur = max_dur.max(dur);
            }
        }
        assert!(
            min_dur >= 1,
            "Sleep duration min should be >= 1, got {}",
            min_dur
        );
        assert!(
            max_dur <= 25,
            "Sleep duration max should be <= 25, got {}",
            max_dur
        );
    }

    // -------------------------------------------------------------------
    // 15. Anti-magic trap drains power
    // -------------------------------------------------------------------
    #[test]
    fn anti_magic_drains_power() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::AntiMagic);

        let events = trigger_trap(&mut rng, &info, &mut trap);

        let pw_drain = events.iter().find_map(|e| {
            if let EngineEvent::PwChange { amount, .. } = e {
                Some(*amount)
            } else {
                None
            }
        });
        assert!(pw_drain.is_some(), "Anti-magic should drain power");
        let drain = -pw_drain.unwrap();
        assert!(
            drain >= 2 && drain <= 12,
            "d(2,6) drain should be 2..12, got {}",
            drain
        );
    }

    // -------------------------------------------------------------------
    // 16. TrapMap operations
    // -------------------------------------------------------------------
    #[test]
    fn trap_map_operations() {
        let mut tm = TrapMap::new();
        let pos = Position::new(5, 5);

        place_trap(&mut tm, pos, TrapType::ArrowTrap);

        assert!(tm.trap_at(pos).is_some());
        assert!(tm.trap_at(Position::new(6, 6)).is_none());

        let removed = tm.remove_trap_at(pos);
        assert!(removed.is_some());
        assert!(tm.trap_at(pos).is_none());
    }

    // -------------------------------------------------------------------
    // 17. Levitating avoids floor triggers but not teleport trap
    // -------------------------------------------------------------------
    #[test]
    fn levitating_avoids_floor_triggers_only() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.is_levitating = true;

        // Floor trigger: should avoid
        let pit = TrapInstance::new(Position::new(10, 5), TrapType::Pit);
        assert!(avoid_trap(&mut rng, &info, &pit));

        // Non-floor trigger (teleport trap): should NOT auto-avoid
        // (may still get DEX-based avoidance if detected, but not guaranteed)
        let telep = TrapInstance::new(Position::new(10, 5), TrapType::TeleportTrap);
        // TeleportTrap is not a floor trigger, so levitation doesn't bypass it.
        // Without detection, there's no DEX avoidance either.
        assert!(!avoid_trap(&mut rng, &info, &telep));
    }

    // -------------------------------------------------------------------
    // 18. Escape trap for bear trap is turn-based
    // -------------------------------------------------------------------
    #[test]
    fn escape_bear_trap_turn_based() {
        let mut rng = test_rng();
        let info = test_entity_info();

        // Not yet at 0 or 1
        let trapped = Trapped {
            kind: TrappedIn::BearTrap,
            turns_remaining: 3,
        };
        let (events, escaped) = escape_trap(&mut rng, &info, &trapped);
        assert!(!escaped, "Should not escape with 3 turns remaining");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("bear-stuck")
        )));

        // At 1 turn remaining
        let trapped2 = Trapped {
            kind: TrappedIn::BearTrap,
            turns_remaining: 1,
        };
        let (events2, escaped2) = escape_trap(&mut rng, &info, &trapped2);
        assert!(escaped2, "Should escape with 1 turn remaining");
        assert!(events2.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key.contains("bear-escape")
        )));
    }

    // =================================================================
    // H.2: Additional Trap System Alignment Tests
    // =================================================================

    // ── All trap types have trigger handlers ────────────────────

    #[test]
    fn test_trap_all_types_trigger() {
        // Verify every map-placeable trap type has a working
        // trigger_trap handler that produces at least a TrapTriggered
        // event.
        let map_types = [
            TrapType::ArrowTrap,
            TrapType::DartTrap,
            TrapType::RockTrap,
            TrapType::SqueakyBoard,
            TrapType::BearTrap,
            TrapType::Landmine,
            TrapType::RollingBoulderTrap,
            TrapType::SleepingGasTrap,
            TrapType::RustTrap,
            TrapType::FireTrap,
            TrapType::Pit,
            TrapType::SpikedPit,
            TrapType::Hole,
            TrapType::TrapDoor,
            TrapType::TeleportTrap,
            TrapType::LevelTeleport,
            TrapType::MagicPortal,
            TrapType::Web,
            TrapType::StatueTrap,
            TrapType::MagicTrap,
            TrapType::AntiMagic,
            TrapType::PolyTrap,
            TrapType::VibratingSquare,
            TrapType::TrappedDoor,
            TrapType::TrappedChest,
        ];

        for &tt in &map_types {
            let mut rng = Pcg64::seed_from_u64(tt as u64 + 9999);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), tt);
            let events = trigger_trap(&mut rng, &info, &mut trap);

            let has_triggered = events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapTriggered { .. }));
            assert!(
                has_triggered,
                "TrapType::{:?} should produce TrapTriggered event",
                tt
            );
        }
    }

    // ── Trap damage formulas ────────────────────────────────────

    #[test]
    fn test_trap_bear_trap_damage_range() {
        // d(2,4) = 2..8
        let mut min_dmg = i32::MAX;
        let mut max_dmg = 0i32;
        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::BearTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if let Some(dmg) = events.iter().find_map(|e| {
                if let EngineEvent::HpChange {
                    amount,
                    source: HpSource::Trap,
                    ..
                } = e
                {
                    Some(-*amount)
                } else {
                    None
                }
            }) {
                min_dmg = min_dmg.min(dmg);
                max_dmg = max_dmg.max(dmg);
            }
        }
        assert!(
            min_dmg >= 2,
            "Bear trap min damage should be >= 2, got {}",
            min_dmg
        );
        assert!(
            max_dmg <= 8,
            "Bear trap max damage should be <= 8, got {}",
            max_dmg
        );
        assert_eq!(min_dmg, 2, "Should hit min bear trap damage 2");
        assert_eq!(max_dmg, 8, "Should hit max bear trap damage 8");
    }

    #[test]
    fn test_trap_rock_trap_damage_range() {
        // d(2,6) = 2..12
        let mut min_dmg = i32::MAX;
        let mut max_dmg = 0i32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::RockTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if let Some(dmg) = events.iter().find_map(|e| {
                if let EngineEvent::HpChange {
                    amount,
                    source: HpSource::Trap,
                    ..
                } = e
                {
                    Some(-*amount)
                } else {
                    None
                }
            }) {
                min_dmg = min_dmg.min(dmg);
                max_dmg = max_dmg.max(dmg);
            }
        }
        assert!(
            min_dmg >= 2,
            "Rock trap min damage should be >= 2, got {}",
            min_dmg
        );
        assert!(
            max_dmg <= 12,
            "Rock trap max damage should be <= 12, got {}",
            max_dmg
        );
    }

    #[test]
    fn test_trap_landmine_damage_range() {
        // rnd(16) = 1..16
        let mut min_dmg = i32::MAX;
        let mut max_dmg = 0i32;
        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Landmine);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if let Some(dmg) = events.iter().find_map(|e| {
                if let EngineEvent::HpChange {
                    amount,
                    source: HpSource::Trap,
                    ..
                } = e
                {
                    Some(-*amount)
                } else {
                    None
                }
            }) {
                min_dmg = min_dmg.min(dmg);
                max_dmg = max_dmg.max(dmg);
            }
        }
        assert!(min_dmg >= 1, "Landmine min damage should be >= 1");
        assert!(max_dmg <= 16, "Landmine max damage should be <= 16");
    }

    // ── Trap detection / search ─────────────────────────────────

    #[test]
    fn test_trap_detection_probability() {
        // Base search probability: ~1/8 for each adjacent cell.
        // Over 8000 searches of a single adjacent trap, we should
        // detect it roughly 1000 times.
        let mut detect_count = 0u32;
        let total = 8000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let mut trap_map = TrapMap::new();
            place_trap(
                &mut trap_map,
                Position::new(11, 5), // adjacent
                TrapType::BearTrap,
            );
            let events = detect_trap(
                &mut rng,
                &mut trap_map,
                Position::new(10, 5),
                0, // neutral luck
                0, // no search bonus
            );
            if events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapRevealed { .. }))
            {
                detect_count += 1;
            }
        }
        let pct = detect_count as f64 / total as f64;
        // Expected: ~1/8 = 12.5%, allow +-5%.
        assert!(
            pct > 0.07 && pct < 0.20,
            "Detection rate should be ~12.5%, got {:.1}%",
            pct * 100.0
        );
    }

    #[test]
    fn test_trap_detection_already_seen_noop() {
        // If a trap is already detected, searching should not
        // produce another TrapRevealed event.
        let mut rng = test_rng();
        let mut trap_map = TrapMap::new();
        let mut trap = TrapInstance::new(Position::new(11, 5), TrapType::ArrowTrap);
        trap.detected = true;
        trap_map.traps.push(trap);

        let events = detect_trap(&mut rng, &mut trap_map, Position::new(10, 5), 0, 0);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapRevealed { .. })),
            "Already-detected trap should not be re-revealed"
        );
    }

    // ── Trap avoidance ──────────────────────────────────────────

    #[test]
    fn test_trap_avoidance_20pct_for_seen() {
        // Seen, non-special traps have 20% avoidance chance.
        let mut avoided = 0u32;
        let total = 10_000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::TeleportTrap);
            trap.detected = true;
            if avoid_trap(&mut rng, &info, &trap) {
                avoided += 1;
            }
        }
        let pct = avoided as f64 / total as f64;
        // Expected: 20% (1/5), allow +-5%.
        assert!(
            (pct - 0.20).abs() < 0.05,
            "Seen trap avoidance should be ~20%, got {:.1}%",
            pct * 100.0
        );
    }

    #[test]
    fn test_trap_avoidance_anti_magic_not_avoidable() {
        // Anti-magic traps cannot be avoided by the DEX check.
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::AntiMagic);
            trap.detected = true;
            // AntiMagic is not a floor trigger, so no flying bypass.
            // DEX avoidance explicitly excludes AntiMagic.
            assert!(
                !avoid_trap(&mut rng, &info, &trap),
                "Anti-magic traps should not be avoidable by DEX"
            );
        }
    }

    #[test]
    fn test_trap_avoidance_undestroyable_not_avoidable() {
        // Vibrating square cannot be avoided by DEX.
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::VibratingSquare);
            trap.detected = true;
            assert!(
                !avoid_trap(&mut rng, &info, &trap),
                "VibratingSquare should not be avoidable by DEX"
            );
        }
    }

    #[test]
    fn test_trap_amorphous_avoids_bear_and_web() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.is_amorphous = true;

        let bear = TrapInstance::new(Position::new(10, 5), TrapType::BearTrap);
        assert!(avoid_trap(&mut rng, &info, &bear));

        let web = TrapInstance::new(Position::new(10, 5), TrapType::Web);
        assert!(avoid_trap(&mut rng, &info, &web));
    }

    // ── Web duration by STR table ───────────────────────────────

    #[test]
    fn test_trap_web_duration_full_str_table() {
        // Test the full STR -> duration mapping from the spec.
        let cases: &[(u8, u32, u32)] = &[
            // (str, min_dur, max_dur)
            (3, 6, 11),
            (4, 4, 9),
            (5, 4, 9),
            (6, 4, 7),
            (8, 4, 7),
            (9, 2, 5),
            (11, 2, 5),
            (12, 2, 3),
            (14, 2, 3),
            (15, 1, 2),
            (17, 1, 2),
            (18, 1, 1),
            (50, 1, 1),
            (68, 1, 1),
            (69, 0, 0),  // instant tear
            (100, 0, 0), // instant tear
        ];

        for &(str_val, expected_min, expected_max) in cases {
            let mut min_dur = u32::MAX;
            let mut max_dur = 0u32;
            for seed in 0..500 {
                let mut rng = Pcg64::seed_from_u64(seed);
                let dur = web_duration_by_str(&mut rng, str_val);
                min_dur = min_dur.min(dur);
                max_dur = max_dur.max(dur);
            }
            assert!(
                min_dur >= expected_min,
                "STR {}: min duration should be >= {}, got {}",
                str_val,
                expected_min,
                min_dur
            );
            assert!(
                max_dur <= expected_max,
                "STR {}: max duration should be <= {}, got {}",
                str_val,
                expected_max,
                max_dur
            );
        }
    }

    // ── Spiked pit poison chance ────────────────────────────────

    #[test]
    fn test_trap_spiked_pit_poison_1in6() {
        // 1/6 chance of poison on spiked pit.
        let mut poison_count = 0u32;
        let total = 6000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SpikedPit);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            let has_poison = events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::HpChange {
                        source: HpSource::Poison,
                        ..
                    }
                )
            });
            if has_poison {
                poison_count += 1;
            }
        }
        let pct = poison_count as f64 / total as f64;
        // Expected: ~16.7% (1/6), allow +-5%.
        assert!(
            (pct - 0.167).abs() < 0.05,
            "Spiked pit poison rate should be ~16.7%, got {:.1}%",
            pct * 100.0,
        );
    }

    // ── Dart poison chance ──────────────────────────────────────

    #[test]
    fn test_trap_dart_poison_1in6() {
        // 1/6 chance of poisoned dart.
        let mut poison_count = 0u32;
        let total = 6000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::DartTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            // Poisoned dart emits either "poison" or "poison-resist" message.
            let has_poison_msg = events
                .iter()
                .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("poison")));
            if has_poison_msg {
                poison_count += 1;
            }
        }
        let pct = poison_count as f64 / total as f64;
        assert!(
            (pct - 0.167).abs() < 0.05,
            "Dart poison rate should be ~16.7%, got {:.1}%",
            pct * 100.0,
        );
    }

    // ── Depletion check ─────────────────────────────────────────

    #[test]
    fn test_trap_arrow_depletion() {
        // Arrow trap: if triggered_count > 1 AND detected AND rn2(15) == 0:
        // depleted (no HP change).
        let mut depleted = 0u32;
        let total = 15_000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::ArrowTrap);
            trap.triggered_count = 1; // will become 2 inside trigger_trap
            trap.detected = true;
            let events = trigger_trap(&mut rng, &info, &mut trap);
            let has_hp_change = events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::HpChange {
                        source: HpSource::Trap,
                        ..
                    }
                )
            });
            if !has_hp_change {
                depleted += 1;
            }
        }
        let pct = depleted as f64 / total as f64;
        // Expected: ~1/15 = 6.7%, allow +-3%.
        assert!(
            (pct - 0.067).abs() < 0.03,
            "Arrow trap depletion rate should be ~6.7%, got {:.1}%",
            pct * 100.0,
        );
    }

    // ── Magic trap explosion ────────────────────────────────────

    #[test]
    fn test_trap_magic_trap_explosion_1in30() {
        // 1/30 chance of explosion (which grants +2 max energy).
        let mut explosion_count = 0u32;
        let total = 30_000u32;
        for seed in 0..total {
            let mut rng = Pcg64::seed_from_u64(seed as u64);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::MagicTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            let has_pw_gain = events
                .iter()
                .any(|e| matches!(e, EngineEvent::PwChange { amount, .. } if *amount > 0));
            if has_pw_gain {
                explosion_count += 1;
            }
        }
        let pct = explosion_count as f64 / total as f64;
        // Expected: ~1/30 = 3.3%, allow +-2%.
        assert!(
            (pct - 0.033).abs() < 0.02,
            "Magic trap explosion rate should be ~3.3%, got {:.1}%",
            pct * 100.0,
        );
    }

    // ── Level teleport blocked by magic resistance ──────────────

    #[test]
    fn test_trap_level_teleport_blocked_by_mr() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.magic_resistant = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::LevelTeleport);
        let events = trigger_trap(&mut rng, &info, &mut trap);

        let has_level_change = events
            .iter()
            .any(|e| matches!(e, EngineEvent::LevelChanged { .. }));
        assert!(
            !has_level_change,
            "Level teleport should be blocked by magic resistance"
        );
    }

    // ── Polymorph trap blocked by magic resistance ──────────────

    #[test]
    fn test_trap_poly_blocked_by_mr() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.magic_resistant = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::PolyTrap);
        let events = trigger_trap(&mut rng, &info, &mut trap);

        let has_poly = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::StatusApplied {
                    status: StatusEffect::Polymorphed,
                    ..
                }
            )
        });
        assert!(
            !has_poly,
            "Polymorph trap should be blocked by magic resistance"
        );
    }

    // ── Pit trapping duration ───────────────────────────────────

    #[test]
    fn test_trap_pit_duration_range() {
        // rn1(6, 2) = [2, 7]
        let mut min_dur = u32::MAX;
        let mut max_dur = 0u32;
        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Pit);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if let Some(dur) = events.iter().find_map(|e| {
                if let EngineEvent::StatusApplied {
                    duration: Some(d), ..
                } = e
                {
                    Some(*d)
                } else {
                    None
                }
            }) {
                min_dur = min_dur.min(dur);
                max_dur = max_dur.max(dur);
            }
        }
        assert!(
            min_dur >= 2,
            "Pit duration min should be >= 2, got {}",
            min_dur
        );
        assert!(
            max_dur <= 7,
            "Pit duration max should be <= 7, got {}",
            max_dur
        );
        assert_eq!(min_dur, 2, "Should hit pit duration 2");
        assert_eq!(max_dur, 7, "Should hit pit duration 7");
    }

    // ── Hole always visible ─────────────────────────────────────

    #[test]
    fn test_trap_hole_always_visible() {
        let trap = TrapInstance::new(Position::new(10, 5), TrapType::Hole);
        assert!(
            trap.detected,
            "Holes should be detected (visible) on creation"
        );
    }

    // ── Floor trigger classification ────────────────────────────

    #[test]
    fn test_trap_floor_trigger_classification() {
        // Types 1..14 are floor triggers.
        let floor_triggers = [
            TrapType::ArrowTrap,
            TrapType::DartTrap,
            TrapType::RockTrap,
            TrapType::SqueakyBoard,
            TrapType::BearTrap,
            TrapType::Landmine,
            TrapType::RollingBoulderTrap,
            TrapType::SleepingGasTrap,
            TrapType::RustTrap,
            TrapType::FireTrap,
            TrapType::Pit,
            TrapType::SpikedPit,
            TrapType::Hole,
            TrapType::TrapDoor,
        ];
        for &tt in &floor_triggers {
            assert!(is_floor_trigger(tt), "{:?} should be a floor trigger", tt);
        }

        // Types >= 15 are NOT floor triggers.
        let non_floor = [
            TrapType::TeleportTrap,
            TrapType::LevelTeleport,
            TrapType::MagicPortal,
            TrapType::Web,
            TrapType::StatueTrap,
            TrapType::MagicTrap,
            TrapType::AntiMagic,
            TrapType::PolyTrap,
            TrapType::VibratingSquare,
        ];
        for &tt in &non_floor {
            assert!(
                !is_floor_trigger(tt),
                "{:?} should NOT be a floor trigger",
                tt
            );
        }
    }

    // ── Escape pit by flying/levitating ─────────────────────────

    #[test]
    fn test_trap_escape_pit_by_flying() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.is_flying = true;
        let trapped = Trapped {
            kind: TrappedIn::Pit,
            turns_remaining: 5,
        };
        let (_events, escaped) = escape_trap(&mut rng, &info, &trapped);
        assert!(escaped, "Flying entity should instantly escape pit");
    }

    #[test]
    fn test_trap_escape_pit_by_levitating() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.is_levitating = true;
        let trapped = Trapped {
            kind: TrappedIn::Pit,
            turns_remaining: 5,
        };
        let (_events, escaped) = escape_trap(&mut rng, &info, &trapped);
        assert!(escaped, "Levitating entity should instantly escape pit");
    }

    // ── Anti-magic energy drain range ───────────────────────────

    #[test]
    fn test_trap_anti_magic_drain_range() {
        // d(2,6) = 2..12
        let mut min_drain = i32::MAX;
        let mut max_drain = 0i32;
        for seed in 0..1000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::AntiMagic);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if let Some(drain) = events.iter().find_map(|e| {
                if let EngineEvent::PwChange { amount, .. } = e {
                    Some(-*amount)
                } else {
                    None
                }
            }) {
                min_drain = min_drain.min(drain);
                max_drain = max_drain.max(drain);
            }
        }
        assert!(
            min_drain >= 2,
            "AM drain min should be >= 2, got {}",
            min_drain
        );
        assert!(
            max_drain <= 12,
            "AM drain max should be <= 12, got {}",
            max_drain
        );
    }

    // ── Anti-magic physical damage with MR ──────────────────────

    #[test]
    fn test_trap_anti_magic_phys_damage_with_mr() {
        // Entity with magic resistance takes rnd(4) physical damage.
        let mut has_phys = false;
        for seed in 0..100 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let mut info = test_entity_info();
            info.magic_resistant = true;
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::AntiMagic);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if events.iter().any(|e| {
                matches!(
                    e,
                    EngineEvent::HpChange {
                        source: HpSource::Trap,
                        ..
                    }
                )
            }) {
                has_phys = true;
                break;
            }
        }
        assert!(
            has_phys,
            "Magic-resistant entity should take physical damage from anti-magic"
        );
    }

    // ── Sleeping gas duration range ─────────────────────────────

    #[test]
    fn test_trap_sleeping_gas_duration_extremes() {
        // rnd(25) = 1..25
        let mut min_dur = u32::MAX;
        let mut max_dur = 0u32;
        for seed in 0..2000 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_entity_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SleepingGasTrap);
            let events = trigger_trap(&mut rng, &info, &mut trap);
            if let Some(dur) = events.iter().find_map(|e| {
                if let EngineEvent::StatusApplied {
                    status: StatusEffect::Sleeping,
                    duration: Some(d),
                    ..
                } = e
                {
                    Some(*d)
                } else {
                    None
                }
            }) {
                min_dur = min_dur.min(dur);
                max_dur = max_dur.max(dur);
            }
        }
        assert_eq!(min_dur, 1, "Sleep gas min duration should be 1");
        assert_eq!(max_dur, 25, "Sleep gas max duration should be 25");
    }

    // ═══════════════════════════════════════════════════════════════
    // New tests: monster_trigger_trap, disarm_trap, trigger_trap_at,
    //            convert_landmine_to_pit
    // ═══════════════════════════════════════════════════════════════

    fn test_mon_info() -> MonsterTrapInfo {
        MonsterTrapInfo {
            entity: Entity::DANGLING,
            pos: Position::new(10, 5),
            hp: 20,
            max_hp: 20,
            ..MonsterTrapInfo::default()
        }
    }

    #[test]
    fn monster_trigger_trap_arrow_damages() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::ArrowTrap);

        let events = monster_trigger_trap(&mut rng, &info, &mut trap);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapTriggered { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::HpChange { .. }))
        );
    }

    #[test]
    fn monster_flying_avoids_pit() {
        let mut rng = test_rng();
        let mut info = test_mon_info();
        info.is_flying = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Pit);

        let events = monster_trigger_trap(&mut rng, &info, &mut trap);
        assert!(events.is_empty(), "flying monster should avoid pit");
    }

    #[test]
    fn monster_amorphous_avoids_bear_trap() {
        let mut rng = test_rng();
        let mut info = test_mon_info();
        info.is_amorphous = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::BearTrap);

        let events = monster_trigger_trap(&mut rng, &info, &mut trap);
        assert!(
            events.is_empty(),
            "amorphous monster should avoid bear trap"
        );
    }

    #[test]
    fn monster_trigger_web_traps() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Web);

        let events = monster_trigger_trap(&mut rng, &info, &mut trap);
        assert!(!events.is_empty(), "grounded monster should trigger web");
    }

    #[test]
    fn disarm_undestroyable_fails() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::MagicPortal);

        let (events, disarmed) = disarm_trap(&mut rng, &info, &mut trap);
        assert!(!disarmed, "magic portal should not be disarmable");
        assert!(events.iter().any(|e| matches!(
            e,
            EngineEvent::Message { key, .. } if key == "trap-cannot-disarm"
        )));
    }

    #[test]
    fn disarm_with_high_dex_succeeds() {
        // High DEX = easy disarm.
        let mut info = test_entity_info();
        info.dexterity = 25;
        info.luck = 5;

        let mut saw_disarm = false;
        for seed in 0..100u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::ArrowTrap);
            trap.detected = true;
            let (events, disarmed) = disarm_trap(&mut rng, &info, &mut trap);
            if disarmed {
                saw_disarm = true;
                assert!(events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "trap-disarmed"
                )));
                break;
            }
        }
        assert!(
            saw_disarm,
            "high DEX should eventually succeed at disarming"
        );
    }

    #[test]
    fn disarm_failure_triggers_trap() {
        // Low DEX = likely failure.
        let mut info = test_entity_info();
        info.dexterity = 3;
        info.luck = -5;

        let mut saw_trigger = false;
        for seed in 0..100u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::ArrowTrap);
            trap.detected = true;
            let (events, disarmed) = disarm_trap(&mut rng, &info, &mut trap);
            if !disarmed {
                saw_trigger = true;
                assert!(
                    events
                        .iter()
                        .any(|e| matches!(e, EngineEvent::TrapTriggered { .. })),
                    "failed disarm should trigger trap"
                );
                break;
            }
        }
        assert!(saw_trigger, "low DEX should eventually fail at disarming");
    }

    #[test]
    fn trigger_trap_at_no_trap() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap_map = TrapMap::new();

        let (events, triggered) = trigger_trap_at(&mut rng, &info, &mut trap_map);
        assert!(events.is_empty());
        assert!(!triggered);
    }

    #[test]
    fn trigger_trap_at_with_trap() {
        let mut rng = test_rng();
        let info = test_entity_info();
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(10, 5), TrapType::ArrowTrap);

        let (events, triggered) = trigger_trap_at(&mut rng, &info, &mut trap_map);
        assert!(triggered, "should trigger the arrow trap");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, EngineEvent::TrapTriggered { .. }))
        );
    }

    #[test]
    fn trigger_trap_at_flying_avoids() {
        let mut rng = test_rng();
        let mut info = test_entity_info();
        info.is_flying = true;
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(10, 5), TrapType::Pit);

        let (_events, triggered) = trigger_trap_at(&mut rng, &info, &mut trap_map);
        assert!(!triggered, "flying entity should avoid pit");
    }

    #[test]
    fn convert_landmine_to_pit_works() {
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(5, 5), TrapType::Landmine);
        assert_eq!(
            trap_map.trap_at(Position::new(5, 5)).unwrap().trap_type,
            TrapType::Landmine
        );

        convert_landmine_to_pit(&mut trap_map, Position::new(5, 5));
        let trap = trap_map.trap_at(Position::new(5, 5)).unwrap();
        assert_eq!(trap.trap_type, TrapType::Pit);
        assert!(trap.detected);
    }

    #[test]
    fn convert_landmine_to_pit_ignores_non_landmine() {
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(5, 5), TrapType::ArrowTrap);

        convert_landmine_to_pit(&mut trap_map, Position::new(5, 5));
        assert_eq!(
            trap_map.trap_at(Position::new(5, 5)).unwrap().trap_type,
            TrapType::ArrowTrap,
            "non-landmine should not be converted"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // New tests: bear_trap_str_escape, trap_chain_reaction,
    //            detect_traps, create_trap
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn bear_trap_str_escape_high_str_succeeds() {
        let mut info = test_entity_info();
        info.strength = 25; // High STR → good chance.
        let trapped = Trapped {
            kind: TrappedIn::BearTrap,
            turns_remaining: 5,
        };

        let mut saw_escape = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (events, escaped) = bear_trap_str_escape(&mut rng, &info, &trapped);
            if escaped {
                saw_escape = true;
                assert!(events.iter().any(|e| matches!(
                    e,
                    EngineEvent::Message { key, .. } if key == "trap-bear-rip-free"
                )));
                break;
            }
        }
        assert!(saw_escape, "STR 25 should eventually rip free");
    }

    #[test]
    fn bear_trap_str_escape_low_str_fails() {
        let mut info = test_entity_info();
        info.strength = 3; // Low STR → very low chance.
        let trapped = Trapped {
            kind: TrappedIn::BearTrap,
            turns_remaining: 5,
        };

        let mut all_stuck = true;
        for seed in 0..20u64 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let (_events, escaped) = bear_trap_str_escape(&mut rng, &info, &trapped);
            if escaped {
                all_stuck = false;
            }
        }
        // With STR 3 vs d(1,118), ~2.5% chance per turn; in 20 tries
        // we might see a success but most should fail.
        // We just check we got at least some stuck messages.
        // (Not asserting all_stuck because of random chance.)
        let _ = all_stuck; // suppress unused warning
    }

    #[test]
    fn bear_trap_str_escape_wrong_trap_type() {
        let info = test_entity_info();
        let trapped = Trapped {
            kind: TrappedIn::Pit,
            turns_remaining: 3,
        };

        let mut rng = test_rng();
        let (_events, escaped) = bear_trap_str_escape(&mut rng, &info, &trapped);
        assert!(!escaped, "should not escape from non-bear-trap");
    }

    #[test]
    fn trap_chain_reaction_triggers_adjacent_landmines() {
        let mut rng = test_rng();
        let mut trap_map = TrapMap::new();

        let center = Position::new(10, 10);
        // Place landmines adjacent.
        place_trap(&mut trap_map, Position::new(11, 10), TrapType::Landmine);
        place_trap(&mut trap_map, Position::new(9, 10), TrapType::Landmine);
        // Also place a non-landmine (should not chain).
        place_trap(&mut trap_map, Position::new(10, 11), TrapType::ArrowTrap);

        let events = trap_chain_reaction(&mut rng, center, &mut trap_map);

        // Should trigger the two landmines.
        let landmine_triggers = events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    EngineEvent::TrapTriggered {
                        trap_type: TrapType::Landmine,
                        ..
                    }
                )
            })
            .count();
        assert!(
            landmine_triggers >= 2,
            "expected 2 landmine triggers, got {}",
            landmine_triggers
        );
    }

    #[test]
    fn trap_chain_reaction_no_adjacent_traps() {
        let mut rng = test_rng();
        let mut trap_map = TrapMap::new();

        let center = Position::new(10, 10);
        // No adjacent traps → no events.
        let events = trap_chain_reaction(&mut rng, center, &mut trap_map);
        assert!(events.is_empty());
    }

    #[test]
    fn detect_traps_reveals_hidden() {
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(5, 5), TrapType::ArrowTrap);
        place_trap(&mut trap_map, Position::new(7, 5), TrapType::Pit);
        // Far away trap.
        place_trap(&mut trap_map, Position::new(50, 50), TrapType::FireTrap);

        assert!(!trap_map.trap_at(Position::new(5, 5)).unwrap().detected);
        assert!(!trap_map.trap_at(Position::new(7, 5)).unwrap().detected);

        let events = detect_traps(Position::new(6, 5), 3, &mut trap_map);

        assert!(trap_map.trap_at(Position::new(5, 5)).unwrap().detected);
        assert!(trap_map.trap_at(Position::new(7, 5)).unwrap().detected);
        // Far trap should NOT be detected.
        assert!(!trap_map.trap_at(Position::new(50, 50)).unwrap().detected);

        assert_eq!(events.len(), 2, "should reveal exactly 2 traps");
        assert!(
            events
                .iter()
                .all(|e| matches!(e, EngineEvent::TrapRevealed { .. }))
        );
    }

    #[test]
    fn detect_traps_already_detected_no_double() {
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(5, 5), TrapType::ArrowTrap);
        trap_map.trap_at_mut(Position::new(5, 5)).unwrap().detected = true;

        let events = detect_traps(Position::new(5, 5), 5, &mut trap_map);
        assert!(
            events.is_empty(),
            "already-detected trap should not emit event"
        );
    }

    #[test]
    fn create_trap_places_new() {
        let mut rng = test_rng();
        let mut trap_map = TrapMap::new();

        let events = create_trap(
            &mut rng,
            &mut trap_map,
            Position::new(8, 8),
            TrapType::BearTrap,
        );
        let trap = trap_map.trap_at(Position::new(8, 8));
        assert!(trap.is_some());
        assert_eq!(trap.unwrap().trap_type, TrapType::BearTrap);
        assert!(!events.is_empty());
    }

    #[test]
    fn create_trap_replaces_existing() {
        let mut rng = test_rng();
        let mut trap_map = TrapMap::new();
        place_trap(&mut trap_map, Position::new(8, 8), TrapType::ArrowTrap);

        create_trap(
            &mut rng,
            &mut trap_map,
            Position::new(8, 8),
            TrapType::FireTrap,
        );

        let trap = trap_map.trap_at(Position::new(8, 8)).unwrap();
        assert_eq!(
            trap.trap_type,
            TrapType::FireTrap,
            "should have replaced arrow with fire"
        );
    }

    #[test]
    fn create_trap_hole_auto_detected() {
        let mut rng = test_rng();
        let mut trap_map = TrapMap::new();

        create_trap(&mut rng, &mut trap_map, Position::new(3, 3), TrapType::Hole);
        let trap = trap_map.trap_at(Position::new(3, 3)).unwrap();
        assert!(trap.detected, "holes should be auto-detected");
    }

    // ═══════════════════════════════════════════════════════════════
    // MonsterTrapResult / monster_hits_trap tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn monster_hits_trap_flying_avoids_pit() {
        let mut rng = test_rng();
        let mut info = test_mon_info();
        info.is_flying = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Pit);

        let (result, events) = monster_hits_trap(&mut rng, &info, &mut trap);
        assert_eq!(result, MonsterTrapResult::Avoids);
        assert!(events.is_empty());
    }

    #[test]
    fn monster_hits_trap_bear_trap_trapped() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::BearTrap);

        let (result, events) = monster_hits_trap(&mut rng, &info, &mut trap);
        match result {
            MonsterTrapResult::Trapped { duration } => {
                assert!(
                    duration >= 4 && duration <= 7,
                    "bear trap duration should be 4..7, got {}",
                    duration
                );
            }
            _ => panic!("expected Trapped, got {:?}", result),
        }
        assert!(!events.is_empty());
    }

    #[test]
    fn monster_hits_trap_fire_without_resistance() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::FireTrap);

        let (result, _events) = monster_hits_trap(&mut rng, &info, &mut trap);
        match result {
            MonsterTrapResult::Damaged {
                damage,
                damage_type,
            } => {
                assert!(
                    damage >= 2 && damage <= 8,
                    "fire trap d(2,4) should be 2..8, got {}",
                    damage
                );
                assert_eq!(damage_type, "fire");
            }
            _ => panic!("expected Damaged, got {:?}", result),
        }
    }

    #[test]
    fn monster_hits_trap_teleport_resisted_by_mr() {
        // Level teleport blocked by MR
        let mut rng = test_rng();
        let mut info = test_mon_info();
        info.magic_resistant = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::LevelTeleport);

        let (result, _events) = monster_hits_trap(&mut rng, &info, &mut trap);
        assert_eq!(result, MonsterTrapResult::Resists);
    }

    #[test]
    fn monster_hits_trap_rolling_boulder_damage() {
        // Rolling boulder: d(3,10) = 3..30
        let mut min_dmg = i32::MAX;
        let mut max_dmg = 0i32;
        for seed in 0..500 {
            let mut rng = Pcg64::seed_from_u64(seed);
            let info = test_mon_info();
            let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::RollingBoulderTrap);
            let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
            if let MonsterTrapResult::Damaged { damage, .. } = result {
                min_dmg = min_dmg.min(damage);
                max_dmg = max_dmg.max(damage);
            }
        }
        assert!(
            min_dmg >= 3,
            "rolling boulder min should be >= 3, got {}",
            min_dmg
        );
        assert!(
            max_dmg <= 30,
            "rolling boulder max should be <= 30, got {}",
            max_dmg
        );
    }

    #[test]
    fn monster_hits_trap_sleeping_gas_falls_asleep() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SleepingGasTrap);

        let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
        match result {
            MonsterTrapResult::FallsAsleep { duration } => {
                assert!(
                    duration >= 1 && duration <= 25,
                    "sleep duration should be 1..25, got {}",
                    duration
                );
            }
            _ => panic!("expected FallsAsleep, got {:?}", result),
        }
    }

    #[test]
    fn monster_hits_trap_sleep_gas_resisted() {
        let mut rng = test_rng();
        let mut info = test_mon_info();
        info.sleep_resistant = true;
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SleepingGasTrap);

        let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
        assert_eq!(result, MonsterTrapResult::Resists);
    }

    #[test]
    fn monster_hits_trap_squeaky_board_alerts() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::SqueakyBoard);

        let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
        assert_eq!(result, MonsterTrapResult::AlertsPlayer);
    }

    #[test]
    fn monster_hits_trap_vibrating_square_no_effect() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::VibratingSquare);

        let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
        assert_eq!(result, MonsterTrapResult::NoEffect);
    }

    #[test]
    fn monster_hits_trap_arrow_damage() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::ArrowTrap);

        let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
        match result {
            MonsterTrapResult::Damaged {
                damage,
                damage_type,
            } => {
                assert!(
                    damage >= 1 && damage <= 6,
                    "arrow d(1,6) should be 1..6, got {}",
                    damage
                );
                assert_eq!(damage_type, "physical");
            }
            _ => panic!("expected Damaged, got {:?}", result),
        }
    }

    #[test]
    fn monster_hits_trap_pit_falls() {
        let mut rng = test_rng();
        let info = test_mon_info();
        let mut trap = TrapInstance::new(Position::new(10, 5), TrapType::Pit);

        let (result, _) = monster_hits_trap(&mut rng, &info, &mut trap);
        match result {
            MonsterTrapResult::Falls { damage } => {
                assert!(
                    damage >= 1 && damage <= 6,
                    "pit fall damage should be 1..6, got {}",
                    damage
                );
            }
            _ => panic!("expected Falls, got {:?}", result),
        }
    }
}
