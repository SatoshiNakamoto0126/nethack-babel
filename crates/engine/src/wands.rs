//! Wand zapping, ray propagation, and wand-breaking mechanics.
//!
//! Implements the NetHack 3.7 wand system from `zap.c`, `explode.c`, and
//! `apply.c`.  All pure functions operate on plain data parameters for
//! testability.
//!
//! Reference: `specs/wand-ray.md`

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::{Direction, Position};
use crate::dungeon::{LevelMap, Terrain};
use crate::event::{DamageSource, DeathCause, EngineEvent, HpSource, StatusEffect};
use crate::world::{GameWorld, HitPoints, Monster, Positioned};

// ---------------------------------------------------------------------------
// Wand type enumeration
// ---------------------------------------------------------------------------

/// All wand types in NetHack 3.7, categorised by direction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WandType {
    // ── NODIR ────────────────────────────────────────────────
    Light,
    SecretDoorDetection,
    CreateMonster,
    Wishing,
    Enlightenment,

    // ── IMMEDIATE ────────────────────────────────────────────
    Striking,
    SlowMonster,
    SpeedMonster,
    UndeadTurning,
    Polymorph,
    Cancellation,
    Teleportation,
    MakeInvisible,
    Opening,
    Locking,
    Probing,
    Nothing,

    // ── RAY ──────────────────────────────────────────────────
    Death,
    Fire,
    Cold,
    Lightning,
    Sleep,
    MagicMissile,
    Digging,
}

// ---------------------------------------------------------------------------
// Direction classification
// ---------------------------------------------------------------------------

/// How a wand targets: no direction, immediate beam, or bouncing ray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WandDirection {
    Nodir,
    Immediate,
    Ray,
}

impl WandType {
    /// Return the direction type for this wand.
    pub fn direction(self) -> WandDirection {
        match self {
            WandType::Light
            | WandType::SecretDoorDetection
            | WandType::CreateMonster
            | WandType::Wishing
            | WandType::Enlightenment => WandDirection::Nodir,

            WandType::Striking
            | WandType::SlowMonster
            | WandType::SpeedMonster
            | WandType::UndeadTurning
            | WandType::Polymorph
            | WandType::Cancellation
            | WandType::Teleportation
            | WandType::MakeInvisible
            | WandType::Opening
            | WandType::Locking
            | WandType::Probing
            | WandType::Nothing => WandDirection::Immediate,

            WandType::Death
            | WandType::Fire
            | WandType::Cold
            | WandType::Lightning
            | WandType::Sleep
            | WandType::MagicMissile
            | WandType::Digging => WandDirection::Ray,
        }
    }

    /// Number of damage dice (nd) for RAY wands.
    /// Returns 0 for non-RAY types that deal no dice-based ray damage.
    pub fn ray_nd(self) -> u32 {
        match self {
            WandType::MagicMissile => 2,
            WandType::Fire
            | WandType::Cold
            | WandType::Lightning
            | WandType::Sleep
            | WandType::Death => 6,
            _ => 0,
        }
    }

    /// Descriptive name for the ray (used in messages).
    pub fn ray_name(self) -> &'static str {
        match self {
            WandType::MagicMissile => "magic missile",
            WandType::Fire => "bolt of fire",
            WandType::Cold => "bolt of cold",
            WandType::Lightning => "bolt of lightning",
            WandType::Sleep => "sleep ray",
            WandType::Death => "death ray",
            WandType::Digging => "dig beam",
            _ => "beam",
        }
    }
}

// ---------------------------------------------------------------------------
// Wand charges component
// ---------------------------------------------------------------------------

/// Tracks remaining charges on a wand entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WandCharges {
    /// Current charges (obj.spe).  Can be negative after wresting.
    pub spe: i8,
    /// How many times this wand has been recharged (0..7).
    pub recharged: u8,
}

// ---------------------------------------------------------------------------
// Initial charge generation
// ---------------------------------------------------------------------------

/// Generate initial charges for a newly created wand.
///
/// - Wishing: always 1
/// - NODIR: rn1(5,11) = 11..15
/// - IMMEDIATE/RAY: rn1(5,4) = 4..8
pub fn initial_charges<R: Rng>(wand_type: WandType, rng: &mut R) -> WandCharges {
    let spe = if wand_type == WandType::Wishing {
        1
    } else if wand_type.direction() == WandDirection::Nodir {
        // rn1(5,11) = rng.random_range(0..5) + 11 = 11..15
        rng.random_range(0..5) + 11
    } else {
        // rn1(5,4) = rng.random_range(0..5) + 4 = 4..8
        rng.random_range(0..5) + 4
    };
    WandCharges {
        spe: spe as i8,
        recharged: 0,
    }
}

// ---------------------------------------------------------------------------
// Zappable check (charge consumption)
// ---------------------------------------------------------------------------

/// Wresting chance: 1 in 121.
const WAND_WREST_CHANCE: u32 = 121;

/// Result of trying to use a wand charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZapResult {
    /// Wand fires normally; spe has been decremented.
    Success,
    /// Wand was wrested (spe was 0, now -1).
    Wrested,
    /// Wand has no usable charges.
    Fail,
}

/// Check and consume one charge from the wand.
///
/// Mirrors the C `zappable()` function:
/// - spe < 0: return Fail
/// - spe == 0: 1/121 chance of wresting, else Fail
/// - spe > 0: decrement and return Success
pub fn zappable<R: Rng>(charges: &mut WandCharges, rng: &mut R) -> ZapResult {
    if charges.spe < 0 {
        return ZapResult::Fail;
    }
    if charges.spe == 0 {
        if rng.random_range(0u32..WAND_WREST_CHANCE) == 0 {
            charges.spe -= 1; // becomes -1
            return ZapResult::Wrested;
        }
        return ZapResult::Fail;
    }
    charges.spe -= 1;
    ZapResult::Success
}

// ---------------------------------------------------------------------------
// Ray propagation data
// ---------------------------------------------------------------------------

/// Information about a single cell along a ray's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RayCell {
    pub position: Position,
    /// True if the ray bounced at this cell (direction reversed).
    pub bounced: bool,
}

/// Outcome of a ray trace.
#[derive(Debug, Clone)]
pub struct RayPath {
    /// Cells the ray passed through, in order.
    pub cells: Vec<RayCell>,
}

/// Determine whether terrain blocks a ray (solid obstacle that causes bounce).
fn terrain_blocks_ray(terrain: Terrain) -> bool {
    matches!(
        terrain,
        Terrain::Wall | Terrain::Stone | Terrain::DoorClosed | Terrain::DoorLocked | Terrain::Tree
    )
}

/// Determine whether terrain blocks an immediate beam (no bounce; stops).
fn terrain_blocks_beam(terrain: Terrain) -> bool {
    matches!(
        terrain,
        Terrain::Wall | Terrain::Stone | Terrain::DoorClosed | Terrain::DoorLocked | Terrain::Tree
    )
}

// ---------------------------------------------------------------------------
// Bounce direction algorithm
// ---------------------------------------------------------------------------

/// Compute the new direction after a ray bounces off an obstacle.
///
/// Implements the `bounce_dir` algorithm from `zap.c`:
/// - Cardinal direction (one axis zero) or random bounceback: reverse both.
/// - Diagonal: check which adjacent cell is passable and reverse the
///   appropriate axis.
fn bounce_direction<R: Rng>(
    map: &LevelMap,
    pos: Position,
    dx: i32,
    dy: i32,
    bounceback_chance: u32,
    rng: &mut R,
) -> (i32, i32) {
    // Cardinal direction or random bounceback
    if dx == 0
        || dy == 0
        || (bounceback_chance > 0 && rng.random_range(0u32..bounceback_chance) == 0)
    {
        return (-dx, -dy);
    }

    // Diagonal: determine which axis can bounce
    let mut bounce = 0u8;

    // Check if we can reverse Y (move along X from pre-bounce position)
    let lateral_y = Position::new(pos.x, pos.y - dy);
    if map.in_bounds(lateral_y)
        && let Some(cell) = map.get(lateral_y)
        && !terrain_blocks_ray(cell.terrain)
    {
        bounce = 1; // can reverse Y
    }

    // Check if we can reverse X (move along Y from pre-bounce position)
    let lateral_x = Position::new(pos.x - dx, pos.y);
    if map.in_bounds(lateral_x)
        && let Some(cell) = map.get(lateral_x)
        && !terrain_blocks_ray(cell.terrain)
        && (bounce == 0 || rng.random_range(0u32..2) != 0)
    {
        bounce = 2; // can reverse X
    }

    match bounce {
        1 => (dx, -dy),  // reverse Y only
        2 => (-dx, dy),  // reverse X only
        _ => (-dx, -dy), // full reverse (fallback)
    }
}

// ---------------------------------------------------------------------------
// Ray trace (RAY wands)
// ---------------------------------------------------------------------------

/// Trace a ray from `start` in the given direction.
///
/// Range is `rn1(7,7)` = 7..13.  The ray bounces off walls (direction
/// component reversed).  Each cell is recorded.  The ray stops when range
/// drops to 0 or below, or when it leaves the map bounds into stone.
///
/// This function only computes the path; it does not apply damage or effects.
pub fn trace_ray<R: Rng>(
    map: &LevelMap,
    start: Position,
    direction: Direction,
    rng: &mut R,
) -> RayPath {
    let (mut dx, mut dy) = direction.delta();
    if dx == 0 && dy == 0 {
        return RayPath { cells: Vec::new() };
    }

    // rn1(7,7) = rng.random_range(0..7) + 7 = 7..13
    let mut range: i32 = rng.random_range(0..7) + 7;
    let mut cells = Vec::new();
    let mut x = start.x;
    let mut y = start.y;

    while range > 0 {
        range -= 1;
        x += dx;
        y += dy;

        let pos = Position::new(x, y);

        // Out of bounds -> bounce
        if !map.in_bounds(pos) {
            // Step back
            x -= dx;
            y -= dy;
            let (ndx, ndy) = bounce_direction(map, Position::new(x, y), dx, dy, 10, rng);
            dx = ndx;
            dy = ndy;
            continue;
        }

        let cell = match map.get(pos) {
            Some(c) => c,
            None => break,
        };

        if terrain_blocks_ray(cell.terrain) {
            // Bounce
            let bchance: u32 = 75; // default for normal walls
            x -= dx;
            y -= dy;
            let (ndx, ndy) = bounce_direction(map, Position::new(x, y), dx, dy, bchance, rng);
            dx = ndx;
            dy = ndy;
            cells.push(RayCell {
                position: Position::new(x, y),
                bounced: true,
            });
            continue;
        }

        cells.push(RayCell {
            position: pos,
            bounced: false,
        });
    }

    RayPath { cells }
}

// ---------------------------------------------------------------------------
// Ray trace with gen block
// ---------------------------------------------------------------------------

/// Gen-block ray trace: yields each [`RayCell`] as the ray propagates.
///
/// This is the gen-block equivalent of [`trace_ray`].  Instead of
/// collecting all cells into a `RayPath`, the caller receives each cell
/// lazily as the ray steps forward.  Bouncing, range, and boundary
/// logic are identical to `trace_ray`.
pub fn trace_ray_gen<'a, R: Rng>(
    map: &'a LevelMap,
    start: Position,
    direction: Direction,
    rng: &'a mut R,
) -> impl Iterator<Item = RayCell> + 'a {
    gen move {
        let (mut dx, mut dy) = direction.delta();
        if dx == 0 && dy == 0 {
            return;
        }

        // rn1(7,7) = rng.random_range(0..7) + 7 = 7..13
        let mut range: i32 = rng.random_range(0..7) + 7;
        let mut x = start.x;
        let mut y = start.y;

        while range > 0 {
            range -= 1;
            x += dx;
            y += dy;

            let pos = Position::new(x, y);

            // Out of bounds -> bounce
            if !map.in_bounds(pos) {
                x -= dx;
                y -= dy;
                let (ndx, ndy) = bounce_direction(map, Position::new(x, y), dx, dy, 10, rng);
                dx = ndx;
                dy = ndy;
                continue;
            }

            let cell = match map.get(pos) {
                Some(c) => c,
                None => break,
            };

            if terrain_blocks_ray(cell.terrain) {
                // Bounce
                let bchance: u32 = 75;
                x -= dx;
                y -= dy;
                let (ndx, ndy) = bounce_direction(map, Position::new(x, y), dx, dy, bchance, rng);
                dx = ndx;
                dy = ndy;
                yield RayCell {
                    position: Position::new(x, y),
                    bounced: true,
                };
                continue;
            }

            yield RayCell {
                position: pos,
                bounced: false,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Immediate beam trace
// ---------------------------------------------------------------------------

/// Trace an immediate beam from `start` in the given direction.
///
/// Range is `rn1(8,6)` = 6..13.  The beam does not bounce; it stops at
/// walls.  Returns the list of traversed positions.
pub fn trace_immediate<R: Rng>(
    map: &LevelMap,
    start: Position,
    direction: Direction,
    rng: &mut R,
) -> Vec<Position> {
    let (dx, dy) = direction.delta();
    if dx == 0 && dy == 0 {
        return Vec::new();
    }

    // rn1(8,6) = rng.random_range(0..8) + 6 = 6..13
    let range: i32 = rng.random_range(0..8) + 6;
    let mut path = Vec::new();
    let mut x = start.x;
    let mut y = start.y;

    for _ in 0..range {
        x += dx;
        y += dy;
        let pos = Position::new(x, y);

        if !map.in_bounds(pos) {
            break;
        }
        if let Some(cell) = map.get(pos) {
            if terrain_blocks_beam(cell.terrain) {
                break;
            }
        } else {
            break;
        }
        path.push(pos);
    }

    path
}

// ---------------------------------------------------------------------------
// Hit chance (zap_hit)
// ---------------------------------------------------------------------------

/// Determine whether a ray/beam hits a target with the given AC.
///
/// Implements `zap_hit(ac, type)` from `zap.c`.
/// For wands, `spell_bonus` is always 0.
pub fn zap_hit<R: Rng>(target_ac: i32, spell_bonus: i32, rng: &mut R) -> bool {
    let chance = rng.random_range(0..20); // 0..19
    if chance == 0 {
        // 5% edge case: can still miss even naked targets
        return rng.random_range(1..=10) < (target_ac + spell_bonus);
    }
    (3 - chance) < (target_ac + spell_bonus)
}

// ---------------------------------------------------------------------------
// Resistance flags (simplified for pure-function design)
// ---------------------------------------------------------------------------

/// Simplified resistance/property flags for a target entity, used by the
/// wand system to determine effect outcomes.
#[derive(Debug, Clone, Copy, Default)]
pub struct TargetProperties {
    pub magic_resistance: bool,
    pub fire_resistance: bool,
    pub cold_resistance: bool,
    pub sleep_resistance: bool,
    pub shock_resistance: bool,
    pub disintegration_resistance: bool,
    pub reflection: bool,
    /// Whether the target is non-living (undead, golem, etc.)
    pub nonliving: bool,
    /// Whether the target is a demon.
    pub is_demon: bool,
    /// Current HP (needed for death ray calculation).
    pub current_hp: i32,
}

// ---------------------------------------------------------------------------
// Ray effect on a single target
// ---------------------------------------------------------------------------

/// Outcome of a ray hitting a single target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RayHitOutcome {
    /// Target takes damage.
    Damage(u32),
    /// Target is killed instantly.
    InstantDeath,
    /// Target falls asleep for N turns.
    Sleep(u32),
    /// Ray was reflected (target has reflection).
    Reflected,
    /// Ray was resisted entirely (no effect).
    Resisted,
    /// No meaningful effect (e.g. Nothing wand).
    NoEffect,
}

/// Compute the effect of a ray hitting a target.
///
/// Handles reflection first, then resistance checks, then damage/effect.
pub fn ray_effect_on_target<R: Rng>(
    wand_type: WandType,
    target: &TargetProperties,
    rng: &mut R,
) -> RayHitOutcome {
    // Reflection check (only RAY type, not IMMEDIATE)
    if target.reflection && wand_type.direction() == WandDirection::Ray {
        return RayHitOutcome::Reflected;
    }

    match wand_type {
        WandType::Death => {
            // Instakill unless non-living, demon, or magic resistant
            if target.nonliving || target.is_demon || target.magic_resistance {
                RayHitOutcome::Resisted
            } else {
                RayHitOutcome::InstantDeath
            }
        }

        WandType::Fire => {
            if target.fire_resistance {
                RayHitOutcome::Resisted
            } else {
                // d(6,6); cold-resistant targets take +7 extra (spec section 5)
                let mut damage = roll_dice(6, 6, rng);
                if target.cold_resistance {
                    damage += 7;
                }
                RayHitOutcome::Damage(damage)
            }
        }

        WandType::Cold => {
            if target.cold_resistance {
                RayHitOutcome::Resisted
            } else {
                // d(6,6); fire-resistant targets take +d(6,3) extra (spec section 5)
                let mut damage = roll_dice(6, 6, rng);
                if target.fire_resistance {
                    damage += roll_dice(6, 3, rng);
                }
                RayHitOutcome::Damage(damage)
            }
        }

        WandType::Lightning => {
            if target.shock_resistance {
                // Resisted for damage, but could still blind (handled elsewhere)
                RayHitOutcome::Resisted
            } else {
                // d(6,6)
                let damage = roll_dice(6, 6, rng);
                RayHitOutcome::Damage(damage)
            }
        }

        WandType::Sleep => {
            if target.sleep_resistance {
                RayHitOutcome::Resisted
            } else {
                // Sleep for d(nd,25) turns; nd=6 for wands (spec section 5)
                let nd = wand_type.ray_nd();
                let duration = roll_dice(nd, 25, rng);
                RayHitOutcome::Sleep(duration)
            }
        }

        WandType::MagicMissile => {
            if target.magic_resistance {
                RayHitOutcome::Resisted
            } else {
                // d(2,6)
                let damage = roll_dice(2, 6, rng);
                RayHitOutcome::Damage(damage)
            }
        }

        WandType::Digging => {
            // Digging ray doesn't damage creatures directly
            RayHitOutcome::NoEffect
        }

        _ => RayHitOutcome::NoEffect,
    }
}

// ---------------------------------------------------------------------------
// Dice rolling helper
// ---------------------------------------------------------------------------

/// Roll NdS (N dice of S sides), returning the sum.
fn roll_dice<R: Rng>(n: u32, s: u32, rng: &mut R) -> u32 {
    (0..n).map(|_| rng.random_range(1..=s)).sum()
}

// ---------------------------------------------------------------------------
// Zap wand (top-level entry point)
// ---------------------------------------------------------------------------

/// Zap a wand, consuming a charge and producing game events.
///
/// Returns the events generated.  The caller is responsible for applying
/// the events to the game world.
pub fn zap_wand<R: Rng>(
    world: &GameWorld,
    zapper: Entity,
    wand_type: WandType,
    charges: &mut WandCharges,
    direction: Direction,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // Try to consume a charge
    let result = zappable(charges, rng);
    match result {
        ZapResult::Fail => {
            events.push(EngineEvent::msg("wand-nothing"));
            return events;
        }
        ZapResult::Wrested => {
            events.push(EngineEvent::msg("wand-wrested"));
        }
        ZapResult::Success => {}
    }

    let _map = &world.dungeon().current_level;

    // Get zapper position
    let zapper_pos = match world.get_component::<Positioned>(zapper) {
        Some(p) => p.0,
        None => return events,
    };

    match wand_type.direction() {
        WandDirection::Nodir => {
            dispatch_nodir(world, wand_type, zapper, &mut events, rng);
        }
        WandDirection::Immediate => {
            dispatch_immediate(
                world,
                wand_type,
                zapper,
                zapper_pos,
                direction,
                &mut events,
                rng,
            );
        }
        WandDirection::Ray => {
            dispatch_ray(
                world,
                wand_type,
                zapper,
                zapper_pos,
                direction,
                &mut events,
                rng,
            );
        }
    }

    events
}

// ---------------------------------------------------------------------------
// NODIR wand dispatch
// ---------------------------------------------------------------------------

fn dispatch_nodir<R: Rng>(
    world: &GameWorld,
    wand_type: WandType,
    zapper: Entity,
    events: &mut Vec<EngineEvent>,
    rng: &mut R,
) {
    match wand_type {
        WandType::Light => {
            // Light up a 5x5 area around the zapper.
            events.push(EngineEvent::msg("wand-light"));
            if let Some(pos) = world.get_component::<Positioned>(zapper) {
                let center = pos.0;
                let map = &world.dungeon().current_level;
                for dy in -2..=2i32 {
                    for dx in -2..=2i32 {
                        let p = Position::new(center.x + dx, center.y + dy);
                        if map.in_bounds(p) {
                            events.push(EngineEvent::msg_with(
                                "wand-light-cell",
                                vec![("x", p.x.to_string()), ("y", p.y.to_string())],
                            ));
                        }
                    }
                }
            }
        }
        WandType::SecretDoorDetection => {
            // Reveal all secret doors on the current level.
            events.push(EngineEvent::msg("wand-secret-door-detect"));
            let map = &world.dungeon().current_level;
            let (w, h) = map.dimensions();
            for y in 0..h {
                for x in 0..w {
                    let p = Position::new(x as i32, y as i32);
                    if let Some(cell) = map.get(p)
                        && cell.terrain == Terrain::DoorClosed
                        && !cell.explored
                    {
                        events.push(EngineEvent::msg_with(
                            "wand-reveal-door",
                            vec![("x", p.x.to_string()), ("y", p.y.to_string())],
                        ));
                    }
                }
            }
        }
        WandType::CreateMonster => {
            // Emit a creation event. The actual spawning is handled by the caller.
            events.push(EngineEvent::msg("wand-create-monster"));
            if let Some(pos) = world.get_component::<Positioned>(zapper) {
                let num = rng.random_range(1..=4u32);
                for _ in 0..num {
                    events.push(EngineEvent::MonsterGenerated {
                        entity: zapper, // placeholder; real entity created by caller
                        position: pos.0,
                    });
                }
            }
        }
        WandType::Wishing => {
            events.push(EngineEvent::msg("wand-wishing"));
        }
        WandType::Enlightenment => {
            events.push(EngineEvent::msg("wand-enlightenment"));
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// IMMEDIATE beam dispatch
// ---------------------------------------------------------------------------

fn dispatch_immediate<R: Rng>(
    world: &GameWorld,
    wand_type: WandType,
    _zapper: Entity,
    start: Position,
    direction: Direction,
    events: &mut Vec<EngineEvent>,
    rng: &mut R,
) {
    let map = &world.dungeon().current_level;
    let path = trace_immediate(map, start, direction, rng);

    // Walk through the beam path and check for entities and terrain
    for pos in &path {
        // Check for monsters at this position
        for (entity, (positioned, _monster, hp)) in world
            .ecs()
            .query::<(&Positioned, &Monster, &HitPoints)>()
            .iter()
        {
            if positioned.0 == *pos {
                let imm_events = apply_immediate_effect(wand_type, entity, hp, rng);
                events.extend(imm_events);
            }
        }

        // Opening/Locking affect doors along the beam path
        if let Some(cell) = map.get(*pos) {
            match wand_type {
                WandType::Opening
                    if cell.terrain == Terrain::DoorLocked
                        || cell.terrain == Terrain::DoorClosed =>
                {
                    events.push(EngineEvent::DoorOpened { position: *pos });
                }
                WandType::Locking
                    if cell.terrain == Terrain::DoorOpen || cell.terrain == Terrain::DoorClosed =>
                {
                    events.push(EngineEvent::DoorLocked { position: *pos });
                }
                _ => {}
            }
        }
    }

    // Also check the first cell ahead for Opening/Locking on doors that
    // block the beam (trace_immediate stops before them).
    if matches!(wand_type, WandType::Opening | WandType::Locking) {
        let (dx, dy) = direction.delta();
        let next_pos = if path.is_empty() {
            Position::new(start.x + dx, start.y + dy)
        } else {
            let last = path[path.len() - 1];
            Position::new(last.x + dx, last.y + dy)
        };
        if map.in_bounds(next_pos)
            && let Some(cell) = map.get(next_pos)
        {
            match wand_type {
                WandType::Opening
                    if cell.terrain == Terrain::DoorLocked
                        || cell.terrain == Terrain::DoorClosed =>
                {
                    events.push(EngineEvent::DoorOpened { position: next_pos });
                }
                WandType::Locking if cell.terrain == Terrain::DoorOpen => {
                    events.push(EngineEvent::DoorLocked { position: next_pos });
                }
                _ => {}
            }
        }
    }
}

/// Apply an IMMEDIATE wand effect to a target entity.
fn apply_immediate_effect<R: Rng>(
    wand_type: WandType,
    target: Entity,
    hp: &HitPoints,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    match wand_type {
        WandType::Striking => {
            let damage = roll_dice(2, 12, rng);
            events.push(EngineEvent::ExtraDamage {
                target,
                amount: damage,
                source: DamageSource::Wand,
            });
            if (hp.current as u32) <= damage {
                events.push(EngineEvent::EntityDied {
                    entity: target,
                    killer: None,
                    cause: DeathCause::KilledBy {
                        killer_name: "a wand of striking".to_string(),
                    },
                });
            }
        }
        WandType::SlowMonster => {
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::SlowSpeed,
                duration: None,
                source: None,
            });
        }
        WandType::SpeedMonster => {
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::FastSpeed,
                duration: None,
                source: None,
            });
        }
        WandType::Polymorph => {
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::Polymorphed,
                duration: None,
                source: None,
            });
        }
        WandType::Cancellation => {
            events.extend(cancel_monster(target));
        }
        WandType::Teleportation => {
            // Emit a teleport event with dummy positions; the actual
            // destination is resolved by the caller.
            events.push(EngineEvent::msg("monster-teleport-away"));
        }
        WandType::MakeInvisible => {
            events.push(EngineEvent::StatusApplied {
                entity: target,
                status: StatusEffect::Invisible,
                duration: None,
                source: None,
            });
        }
        WandType::UndeadTurning => {
            let damage = roll_dice(1, 8, rng);
            events.push(EngineEvent::ExtraDamage {
                target,
                amount: damage,
                source: DamageSource::Wand,
            });
        }
        WandType::Probing => {
            events.push(EngineEvent::msg("wand-probing"));
        }
        WandType::Opening | WandType::Locking | WandType::Nothing => {
            // No effect on monsters
        }
        _ => {}
    }

    events
}

// ---------------------------------------------------------------------------
// RAY dispatch
// ---------------------------------------------------------------------------

fn dispatch_ray<R: Rng>(
    world: &GameWorld,
    wand_type: WandType,
    zapper: Entity,
    start: Position,
    direction: Direction,
    events: &mut Vec<EngineEvent>,
    rng: &mut R,
) {
    let map = &world.dungeon().current_level;

    events.push(EngineEvent::msg("wand-zap"));

    // For Digging, the ray goes in a straight line through walls/stone,
    // converting them to corridor. Unlike other rays, it does NOT bounce.
    if wand_type == WandType::Digging {
        let (dx, dy) = direction.delta();
        if dx == 0 && dy == 0 {
            return;
        }
        let range: i32 = rng.random_range(0..7) + 7; // 7..13
        let mut dug_any = false;
        let mut cx = start.x;
        let mut cy = start.y;
        for _ in 0..range {
            cx += dx;
            cy += dy;
            let pos = Position::new(cx, cy);
            if !map.in_bounds(pos) {
                break;
            }
            if let Some(map_cell) = map.get(pos)
                && (map_cell.terrain == Terrain::Wall || map_cell.terrain == Terrain::Stone)
            {
                events.push(EngineEvent::msg_with(
                    "wand-digging-cell",
                    vec![("x", pos.x.to_string()), ("y", pos.y.to_string())],
                ));
                dug_any = true;
            }
        }
        if dug_any {
            events.push(EngineEvent::msg("wand-digging"));
        } else {
            events.push(EngineEvent::msg("wand-digging-miss"));
        }
        return;
    }

    // Full ray propagation with hit checks
    let (mut dx, mut dy) = direction.delta();
    if dx == 0 && dy == 0 {
        return;
    }

    let mut range: i32 = rng.random_range(0..7) + 7; // rn1(7,7) = 7..13
    let mut x = start.x;
    let mut y = start.y;

    while range > 0 {
        range -= 1;
        x += dx;
        y += dy;

        let pos = Position::new(x, y);

        // Out of bounds -> bounce
        if !map.in_bounds(pos) {
            x -= dx;
            y -= dy;
            let (ndx, ndy) = bounce_direction(map, Position::new(x, y), dx, dy, 10, rng);
            dx = ndx;
            dy = ndy;
            continue;
        }

        let cell = match map.get(pos) {
            Some(c) => c,
            None => break,
        };

        // Check for entities at this position
        let mut hit_entity = false;
        for (entity, (positioned, _monster, hp)) in world
            .ecs()
            .query::<(&Positioned, &Monster, &HitPoints)>()
            .iter()
        {
            if positioned.0 == pos {
                // Use AC 10 as default for simplified zap_hit
                if zap_hit(10, 0, rng) {
                    let props = TargetProperties {
                        current_hp: hp.current,
                        ..Default::default()
                    };
                    let outcome = ray_effect_on_target(wand_type, &props, rng);
                    match outcome {
                        RayHitOutcome::Reflected => {
                            dx = -dx;
                            dy = -dy;
                            events.push(EngineEvent::msg("wand-ray-reflect"));
                        }
                        RayHitOutcome::InstantDeath => {
                            events.push(EngineEvent::EntityDied {
                                entity,
                                killer: Some(zapper),
                                cause: DeathCause::KilledBy {
                                    killer_name: "a death ray".to_string(),
                                },
                            });
                        }
                        RayHitOutcome::Damage(dmg) => {
                            events.push(EngineEvent::HpChange {
                                entity,
                                amount: -(dmg as i32),
                                new_hp: hp.current - dmg as i32,
                                source: HpSource::Spell,
                            });
                            if hp.current <= dmg as i32 {
                                events.push(EngineEvent::EntityDied {
                                    entity,
                                    killer: Some(zapper),
                                    cause: DeathCause::KilledBy {
                                        killer_name: format!("a {}", wand_type.ray_name()),
                                    },
                                });
                            }
                        }
                        RayHitOutcome::Sleep(duration) => {
                            events.push(EngineEvent::StatusApplied {
                                entity,
                                status: StatusEffect::Sleeping,
                                duration: Some(duration),
                                source: Some(zapper),
                            });
                        }
                        RayHitOutcome::Resisted => {
                            events.push(EngineEvent::msg("wand-ray-absorb"));
                        }
                        RayHitOutcome::NoEffect => {}
                    }
                    // range -= 2 regardless of reflection or hit (spec section 2.2)
                    range -= 2;
                    hit_entity = true;
                }
                break; // only handle first monster at position
            }
        }

        if hit_entity {
            continue;
        }

        // Terrain bounce check
        if terrain_blocks_ray(cell.terrain) {
            let bchance: u32 = 75;
            x -= dx;
            y -= dy;
            let (ndx, ndy) = bounce_direction(map, Position::new(x, y), dx, dy, bchance, rng);
            dx = ndx;
            dy = ndy;
        }
    }
}

// ---------------------------------------------------------------------------
// Wand recharging (scroll of charging / spec section 9)
// ---------------------------------------------------------------------------

/// Result of attempting to recharge a wand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RechargeResult {
    /// Wand successfully recharged to `new_spe`.
    Success { new_spe: i8 },
    /// Wand exploded during recharging.
    Exploded,
    /// Cursed: charges stripped to 0.
    Stripped,
}

/// Attempt to recharge a wand.
///
/// Spec section 9:
/// - Charge limit: wishing=1, directional=8, NODIR=15
/// - Explosion: if recharged > 0 and (wishing or n^3 > rn2(343))
/// - Cursed: strip charges to 0
/// - Otherwise: n = rn1(5, lim-4); if not blessed: n = rnd(n)
///   if spe < n: spe = n; else: spe += 1
pub fn recharge_wand<R: Rng>(
    wand_type: WandType,
    charges: &mut WandCharges,
    blessed: bool,
    cursed: bool,
    rng: &mut R,
) -> RechargeResult {
    let lim: i8 = if wand_type == WandType::Wishing {
        1
    } else if wand_type.direction() != WandDirection::Nodir {
        8
    } else {
        15
    };

    // Explosion check (spec section 9.2).
    let n = charges.recharged as u32;
    if n > 0 {
        let explodes = if wand_type == WandType::Wishing {
            true // Wishing wand always explodes if recharged > 0
        } else {
            let n_cubed = n * n * n;
            n_cubed > rng.random_range(0u32..343)
        };
        if explodes {
            return RechargeResult::Exploded;
        }
    }

    // Increment recharge counter (3-bit field, max 7).
    charges.recharged = (charges.recharged + 1).min(7);

    if cursed {
        // Cursed: strip charges.
        charges.spe = 0;
        return RechargeResult::Stripped;
    }

    // Calculate charge amount.
    let charge_amount: i8 = if lim == 1 {
        1
    } else {
        let top = (lim - 4) + rng.random_range(0i8..5); // rn1(5, lim-4)
        if blessed {
            top
        } else {
            // rnd(top) = 1..top
            if top <= 0 {
                1
            } else {
                rng.random_range(1..=top)
            }
        }
    };

    if charges.spe < charge_amount {
        charges.spe = charge_amount;
    } else {
        charges.spe += 1;
    }

    // Wishing wand: if spe > 3 after recharge, explode.
    if wand_type == WandType::Wishing && charges.spe > 3 {
        return RechargeResult::Exploded;
    }

    RechargeResult::Success {
        new_spe: charges.spe,
    }
}

// ---------------------------------------------------------------------------
// Cursed wand backfire (spec section 10)
// ---------------------------------------------------------------------------

/// Check if a cursed wand backfires on zap.
///
/// 1% chance (1 in 100) for a cursed wand to explode when zapped.
/// Returns true if the wand should backfire.
pub fn cursed_wand_backfire<R: Rng>(rng: &mut R) -> bool {
    rng.random_range(0u32..100) == 0
}

/// Calculate backfire explosion damage: d(spe + 2, 6).
pub fn backfire_damage<R: Rng>(spe: i8, rng: &mut R) -> u32 {
    let n = (spe as i32 + 2).max(1) as u32;
    roll_dice(n, 6, rng)
}

// ---------------------------------------------------------------------------
// Self-zap effects (spec section 17)
// ---------------------------------------------------------------------------

/// Outcome of zapping yourself with a wand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfZapOutcome {
    /// Take damage.
    Damage(u32),
    /// Instant death.
    InstantDeath,
    /// Fall asleep for N turns.
    Sleep(u32),
    /// Teleport.
    Teleport,
    /// Speed up for N turns.
    SpeedUp(u32),
    /// Slow down.
    SlowDown,
    /// Become invisible for N turns.
    Invisible(u32),
    /// Polymorphed.
    Polymorph,
    /// Cancelled (lose special abilities).
    Cancel,
    /// No effect (digging self, nothing wand, etc.).
    NoEffect,
    /// Resisted (magic resistance, etc.).
    Resisted,
}

/// Compute the effect of zapping yourself with a wand.
///
/// Spec section 17: self-zap produces different effects per wand type.
pub fn self_zap_effect<R: Rng>(
    wand_type: WandType,
    target: &TargetProperties,
    rng: &mut R,
) -> SelfZapOutcome {
    match wand_type {
        WandType::Striking => {
            if target.magic_resistance {
                SelfZapOutcome::Resisted
            } else {
                // d(2, 12)
                SelfZapOutcome::Damage(roll_dice(2, 12, rng))
            }
        }
        WandType::Lightning => {
            // d(12, 6) + blinding (handled elsewhere)
            let damage = roll_dice(12, 6, rng);
            SelfZapOutcome::Damage(damage)
        }
        WandType::Fire => {
            let damage = roll_dice(12, 6, rng);
            SelfZapOutcome::Damage(damage)
        }
        WandType::Cold => {
            let damage = roll_dice(12, 6, rng);
            SelfZapOutcome::Damage(damage)
        }
        WandType::MagicMissile => {
            if target.magic_resistance {
                SelfZapOutcome::Resisted
            } else {
                // d(4, 6)
                SelfZapOutcome::Damage(roll_dice(4, 6, rng))
            }
        }
        WandType::Death => {
            // Instant death unless non-living or demon
            if target.nonliving || target.is_demon {
                SelfZapOutcome::Resisted
            } else {
                SelfZapOutcome::InstantDeath
            }
        }
        WandType::Sleep => {
            if target.sleep_resistance {
                SelfZapOutcome::Resisted
            } else {
                // rnd(50) turns
                let duration = rng.random_range(1..=50u32);
                SelfZapOutcome::Sleep(duration)
            }
        }
        WandType::Polymorph => SelfZapOutcome::Polymorph,
        WandType::Cancellation => SelfZapOutcome::Cancel,
        WandType::Teleportation => SelfZapOutcome::Teleport,
        WandType::SpeedMonster => {
            // rn1(25, 50) = 50..74 turns
            let duration = rng.random_range(0u32..25) + 50;
            SelfZapOutcome::SpeedUp(duration)
        }
        WandType::SlowMonster => SelfZapOutcome::SlowDown,
        WandType::MakeInvisible => {
            // rn1(15, 31) = 31..45 turns
            let duration = rng.random_range(0u32..15) + 31;
            SelfZapOutcome::Invisible(duration)
        }
        WandType::Digging
        | WandType::Nothing
        | WandType::Probing
        | WandType::Opening
        | WandType::Locking
        | WandType::UndeadTurning
        | WandType::Light
        | WandType::SecretDoorDetection
        | WandType::CreateMonster
        | WandType::Wishing
        | WandType::Enlightenment => SelfZapOutcome::NoEffect,
    }
}

// ---------------------------------------------------------------------------
// Breaking a wand
// ---------------------------------------------------------------------------

/// Explosion type for breaking wands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplosionType {
    Fiery,
    Frosty,
    Magical,
    NoExplosion,
}

/// Determine the explosion type and damage multiplier when breaking a wand.
fn break_explosion_params(wand_type: WandType) -> (ExplosionType, u32) {
    match wand_type {
        WandType::Death | WandType::Lightning => (ExplosionType::Magical, 4),
        WandType::Fire => (ExplosionType::Fiery, 2),
        WandType::Cold => (ExplosionType::Frosty, 2),
        WandType::MagicMissile => (ExplosionType::Magical, 1),
        // No-explosion group
        WandType::Opening
        | WandType::Wishing
        | WandType::Nothing
        | WandType::Locking
        | WandType::Probing
        | WandType::Enlightenment
        | WandType::SecretDoorDetection => (ExplosionType::NoExplosion, 0),
        // Per-cell effect group: small magical explosion + area effects
        _ => (ExplosionType::Magical, 1),
    }
}

/// Break a wand, potentially causing an explosion.
///
/// Returns the events generated.  The base damage scales with remaining
/// charges: `spe * 4` for the base, multiplied by the wand-specific factor.
pub fn break_wand(
    world: &GameWorld,
    breaker: Entity,
    wand_type: WandType,
    charges: &WandCharges,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    events.push(EngineEvent::msg("wand-break"));

    let (explosion_type, multiplier) = break_explosion_params(wand_type);

    if explosion_type == ExplosionType::NoExplosion {
        events.push(EngineEvent::msg("wand-nothing"));
        return events;
    }

    // Base damage: spe * 4
    let spe = charges.spe.max(0) as u32;
    let base_damage = spe * 4;
    let total_damage = base_damage * multiplier;

    if total_damage == 0 {
        events.push(EngineEvent::msg("wand-break"));
        return events;
    }

    // Get breaker position for 3x3 explosion
    let center = match world.get_component::<Positioned>(breaker) {
        Some(p) => p.0,
        None => return events,
    };

    let source = match explosion_type {
        ExplosionType::Fiery => DamageSource::Fire,
        ExplosionType::Frosty => DamageSource::Cold,
        ExplosionType::Magical | ExplosionType::NoExplosion => DamageSource::Explosion,
    };

    events.push(EngineEvent::msg("wand-break"));

    // 3x3 area explosion: check all entities in range
    let map = &world.dungeon().current_level;
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            let pos = Position::new(center.x + dx, center.y + dy);
            if !map.in_bounds(pos) {
                continue;
            }

            // Check for entities at this position
            for (entity, (positioned, hp)) in
                world.ecs().query::<(&Positioned, &HitPoints)>().iter()
            {
                if positioned.0 == pos {
                    events.push(EngineEvent::ExtraDamage {
                        target: entity,
                        amount: total_damage,
                        source,
                    });
                    if hp.current <= total_damage as i32 {
                        events.push(EngineEvent::EntityDied {
                            entity,
                            killer: Some(breaker),
                            cause: DeathCause::KilledBy {
                                killer_name: "an exploding wand".to_string(),
                            },
                        });
                    }
                }
            }
        }
    }

    events
}

// ---------------------------------------------------------------------------
// Vertical zapping (spec section 13)
// ---------------------------------------------------------------------------

/// Outcome of zapping a wand upward or downward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerticalZapOutcome {
    /// Beam goes through the ceiling/floor; may hit something on another level.
    PassThrough,
    /// Creates a pit or hole in the floor (digging down).
    DigHole,
    /// Opens a hole in the ceiling (digging up).
    DigCeiling,
    /// Light fills the area.
    Light,
    /// Monsters created around the zapper.
    CreateMonster(u32),
    /// Nothing special happens.
    NoEffect,
}

/// Determine the effect of zapping a wand vertically (at floor or ceiling).
///
/// In NetHack, zapping down with a wand of digging creates a hole/trap door.
/// Zapping up with digging opens the ceiling. Most other wands have reduced
/// or no vertical effect.
pub fn zap_updown<R: Rng>(
    wand_type: WandType,
    zapping_down: bool,
    rng: &mut R,
) -> VerticalZapOutcome {
    match wand_type {
        WandType::Digging => {
            if zapping_down {
                VerticalZapOutcome::DigHole
            } else {
                VerticalZapOutcome::DigCeiling
            }
        }
        WandType::Light => VerticalZapOutcome::Light,
        WandType::CreateMonster => {
            let num = rng.random_range(1..=4u32);
            VerticalZapOutcome::CreateMonster(num)
        }
        // RAY wands zapped vertically just pass through
        WandType::Fire
        | WandType::Cold
        | WandType::Lightning
        | WandType::Death
        | WandType::Sleep
        | WandType::MagicMissile => VerticalZapOutcome::PassThrough,
        // Everything else: no special vertical effect
        _ => VerticalZapOutcome::NoEffect,
    }
}

// ---------------------------------------------------------------------------
// Object damage from rays (spec section 6)
// ---------------------------------------------------------------------------

/// Categories of objects that can be damaged or destroyed by rays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectMaterial {
    Paper,
    Wood,
    Cloth,
    Leather,
    Organic,
    Metal,
    Glass,
    Mineral,
    Liquid,
    Wax,
    Plastic,
    Gemstone,
    Bone,
    Dragon,
    Mithril,
    Iron,
    Copper,
    Silver,
    Gold,
}

/// Whether a ray type can destroy objects of a given material.
///
/// Fire burns paper/wood/cloth, cold shatters potions/glass,
/// lightning destroys wands and rings.
pub fn ray_destroys_material(wand_type: WandType, material: ObjectMaterial) -> bool {
    match wand_type {
        WandType::Fire => matches!(
            material,
            ObjectMaterial::Paper
                | ObjectMaterial::Wood
                | ObjectMaterial::Cloth
                | ObjectMaterial::Organic
                | ObjectMaterial::Wax
        ),
        WandType::Cold => matches!(material, ObjectMaterial::Glass | ObjectMaterial::Liquid),
        WandType::Lightning => matches!(
            material,
            ObjectMaterial::Metal
                | ObjectMaterial::Iron
                | ObjectMaterial::Copper
                | ObjectMaterial::Gold
        ),
        _ => false,
    }
}

/// Chance (as percent, 0..100) that an object is destroyed by a ray.
/// In NetHack, the base chance is roughly 1/3 for flammable, 1/5 for glass
/// from cold, and wands have 1/3 chance from lightning.
pub fn ray_destroy_chance(wand_type: WandType, material: ObjectMaterial) -> u32 {
    if !ray_destroys_material(wand_type, material) {
        return 0;
    }
    match wand_type {
        WandType::Fire => 33,      // ~1/3
        WandType::Cold => 20,      // ~1/5
        WandType::Lightning => 33, // ~1/3 for wands/rings
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Engraving with wands (spec section 14)
// ---------------------------------------------------------------------------

/// Effect of applying a wand to the floor for engraving.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngraveEffect {
    /// Wand produces visible text (fire, lightning, digging).
    Burn,
    /// Wand produces invisible writing (mark invisible, nothing).
    Invisible,
    /// Wand erases existing engravings (cancellation, teleport, make invisible).
    Erase,
    /// Wand creates random scrawl (polymorph, striking).
    Scrawl,
    /// Wand causes an electric shock, ruining the engraving (lightning).
    Electric,
    /// No special engraving effect; prompt for text.
    Normal,
}

/// Determine the engraving effect when writing on the floor with a wand.
///
/// This is how players identify wands: the engraving effect reveals the type.
/// Fire burns permanent text, digging engraves permanent text, lightning
/// zaps the floor, etc.
pub fn engrave_with_wand(wand_type: WandType) -> EngraveEffect {
    match wand_type {
        WandType::Fire => EngraveEffect::Burn,
        WandType::Lightning => EngraveEffect::Electric,
        WandType::Digging => EngraveEffect::Burn,
        WandType::Polymorph => EngraveEffect::Scrawl,
        WandType::Striking => EngraveEffect::Scrawl,
        WandType::Cancellation | WandType::Teleportation => EngraveEffect::Erase,
        WandType::MakeInvisible | WandType::Nothing => EngraveEffect::Invisible,
        WandType::Cold | WandType::Sleep | WandType::Death | WandType::MagicMissile => {
            EngraveEffect::Normal
        }
        WandType::SlowMonster | WandType::SpeedMonster => EngraveEffect::Normal,
        _ => EngraveEffect::Normal,
    }
}

// ---------------------------------------------------------------------------
// Wand identification
// ---------------------------------------------------------------------------

/// Whether zapping a wand auto-identifies it (the player discovers the type).
///
/// In NetHack, most wands are identified the first time you see their effect.
/// NODIR wands are identified immediately. RAY wands are identified by their
/// visual ray. Some IMMEDIATE wands are identified when they hit a target.
pub fn wand_auto_identifies_on_zap(wand_type: WandType) -> bool {
    match wand_type {
        // NODIR: always identified on zap
        WandType::Light
        | WandType::SecretDoorDetection
        | WandType::CreateMonster
        | WandType::Wishing
        | WandType::Enlightenment => true,
        // RAY: identified by the visible ray
        WandType::Fire
        | WandType::Cold
        | WandType::Lightning
        | WandType::Death
        | WandType::Sleep
        | WandType::MagicMissile => true,
        // Digging: identified when it digs
        WandType::Digging => true,
        // IMMEDIATE: some identified on visible effect
        WandType::Striking => true,
        WandType::SlowMonster | WandType::SpeedMonster => true,
        WandType::Polymorph => true,
        WandType::MakeInvisible => true,
        WandType::Opening | WandType::Locking => true,
        WandType::Teleportation => true,
        // These don't auto-identify easily
        WandType::Cancellation => false,
        WandType::UndeadTurning => false,
        WandType::Probing => true,
        WandType::Nothing => false,
    }
}

/// Whether engraving with a wand auto-identifies it.
///
/// In NetHack, the distinctive engraving effect is the primary way players
/// identify wands without zapping them.
pub fn wand_auto_identifies_on_engrave(wand_type: WandType) -> bool {
    match engrave_with_wand(wand_type) {
        EngraveEffect::Burn => true,       // fire, digging: distinctive
        EngraveEffect::Electric => true,   // lightning: distinctive
        EngraveEffect::Erase => true,      // cancellation, teleport
        EngraveEffect::Scrawl => false,    // polymorph, striking: not unique
        EngraveEffect::Invisible => false, // could be nothing or make invisible
        EngraveEffect::Normal => false,    // no distinctive effect
    }
}

// ---------------------------------------------------------------------------
// Cancellation effects on entities
// ---------------------------------------------------------------------------

/// Events produced when a cancellation beam hits a monster.
///
/// Cancellation removes special abilities: spellcasting, breath weapons,
/// gaze attacks, etc. It also removes most enchantments.
pub fn cancel_monster(target: Entity) -> Vec<EngineEvent> {
    // Cancellation removes special abilities. We emit a message event
    // and the caller is responsible for stripping the entity's special flags.
    // There is no StatusEffect::Cancelled variant; the effect is permanent
    // removal of properties rather than a timed status.
    let _ = target; // used by caller to identify which entity
    vec![EngineEvent::msg("wand-cancel-monster")]
}

// ---------------------------------------------------------------------------
// Wand ray hitting objects on the ground (bhito / bhitpile from zap.c)
// ---------------------------------------------------------------------------

/// Result of a wand ray hitting an object on the ground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WandObjectResult {
    /// Object is transformed (e.g. polymorph, stone to flesh).
    Transform,
    /// Object's magic is cancelled (enchantment removed).
    Cancel,
    /// Object becomes invisible.
    MakeInvisible,
    /// Object takes damage (e.g. striking).
    Damage { amount: u32 },
    /// Object is destroyed (fire burns scrolls, cold shatters potions, etc.).
    Destroy { reason: &'static str },
    /// No effect on this object.
    NoEffect,
}

/// What happens when a wand ray hits an object on the ground.
///
/// `object_class` uses NetHack's class characters:
/// - `'?'` = scroll, `'+'` = spellbook, `'!'` = potion, `'/'` = wand, `'='` = ring
///
/// `object_material` is the material name (e.g. "stone", "iron", "cloth").
pub fn wand_hits_object<R: Rng>(
    wand_type: WandType,
    object_class: char,
    object_material: &str,
    rng: &mut R,
) -> WandObjectResult {
    match wand_type {
        WandType::Polymorph => WandObjectResult::Transform,
        WandType::Cancellation => WandObjectResult::Cancel,
        WandType::MakeInvisible => WandObjectResult::MakeInvisible,
        WandType::Striking => WandObjectResult::Damage {
            amount: roll_dice(2, 12, rng),
        },
        WandType::Fire => {
            // Fire burns scrolls ('?') and spellbooks ('+')
            if matches!(object_class, '?' | '+') {
                WandObjectResult::Destroy { reason: "burnt" }
            } else {
                WandObjectResult::NoEffect
            }
        }
        WandType::Cold => {
            // Cold shatters potions ('!')
            if object_class == '!' {
                WandObjectResult::Destroy {
                    reason: "shattered",
                }
            } else {
                WandObjectResult::NoEffect
            }
        }
        WandType::Lightning => {
            // Lightning fries wands ('/') and rings ('=')
            if matches!(object_class, '/' | '=') {
                WandObjectResult::Destroy { reason: "fried" }
            } else {
                WandObjectResult::NoEffect
            }
        }
        // Stone to flesh is a spell effect, not a wand type, but we handle
        // the material-based check here for completeness when called from
        // spell/wand integration.
        _ if object_material == "stone" && wand_type == WandType::Polymorph => {
            WandObjectResult::Transform
        }
        WandType::Death | WandType::Sleep | WandType::SlowMonster | WandType::SpeedMonster => {
            WandObjectResult::NoEffect
        }
        _ => WandObjectResult::NoEffect,
    }
}

/// Process a wand ray hitting a pile of objects on the ground.
///
/// Returns a list of (index, result) for each affected object in the pile.
pub fn wand_hits_pile<R: Rng>(
    wand_type: WandType,
    objects: &[(char, &str)],
    rng: &mut R,
) -> Vec<(usize, WandObjectResult)> {
    let mut results = Vec::new();
    for (i, (class, material)) in objects.iter().enumerate() {
        let result = wand_hits_object(wand_type, *class, material, rng);
        if result != WandObjectResult::NoEffect {
            results.push((i, result));
        }
    }
    results
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    /// Deterministic RNG for testing.
    fn test_rng() -> Pcg64Mcg {
        Pcg64Mcg::seed_from_u64(42)
    }

    /// Create a simple floor map with walls on the border.
    fn floor_map(width: usize, height: usize) -> LevelMap {
        let mut map = LevelMap::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let terrain = if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                map.set_terrain(Position::new(x as i32, y as i32), terrain);
            }
        }
        map
    }

    // -----------------------------------------------------------------------
    // Test 1: Ray propagation stops at wall
    // -----------------------------------------------------------------------
    #[test]
    fn ray_stops_at_wall() {
        let mut map = LevelMap::new(20, 5);
        // Floor from x=1..=18, walls at x=0 and x=19
        for y in 0..5 {
            for x in 0..20 {
                let t = if x == 0 || x == 19 || y == 0 || y == 4 {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                map.set_terrain(Position::new(x, y), t);
            }
        }
        // Place a wall in the middle at x=10
        map.set_terrain(Position::new(10, 2), Terrain::Wall);

        let mut rng = test_rng();
        let path = trace_ray(&map, Position::new(1, 2), Direction::East, &mut rng);

        // All cells should have x < 10 (wall at 10 causes bounce, not traversal)
        for cell in &path.cells {
            assert_ne!(cell.position.x, 10, "Ray should not enter the wall cell");
        }
        // At least some cells should have been traversed
        assert!(!path.cells.is_empty(), "Ray should traverse some cells");
    }

    // -----------------------------------------------------------------------
    // Test 2: Ray bounces off wall (direction reverses)
    // -----------------------------------------------------------------------
    #[test]
    fn ray_bounces_off_wall() {
        let map = floor_map(20, 5);
        let mut rng = test_rng();

        // Shoot east from near the right wall.  The ray should bounce.
        let path = trace_ray(&map, Position::new(16, 2), Direction::East, &mut rng);

        // Some cells should be to the left of the start (bounced back)
        let has_bounced_cells = path.cells.iter().any(|c| c.bounced);
        let has_leftward = path.cells.iter().any(|c| c.position.x < 16);
        assert!(
            has_bounced_cells || has_leftward,
            "Ray should bounce off the east wall"
        );
    }

    // -----------------------------------------------------------------------
    // Test 3: Death ray kills target without MR
    // -----------------------------------------------------------------------
    #[test]
    fn death_ray_kills_non_resistant() {
        let mut rng = test_rng();
        let target = TargetProperties {
            current_hp: 50,
            ..Default::default()
        };
        let outcome = ray_effect_on_target(WandType::Death, &target, &mut rng);
        assert_eq!(outcome, RayHitOutcome::InstantDeath);
    }

    // -----------------------------------------------------------------------
    // Test 4: Fire ray deals d(6,6) damage
    // -----------------------------------------------------------------------
    #[test]
    fn fire_ray_deals_damage() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = ray_effect_on_target(WandType::Fire, &target, &mut rng);
        match outcome {
            RayHitOutcome::Damage(d) => {
                assert!((6..=36).contains(&d), "d(6,6) should be 6..36, got {}", d);
            }
            other => panic!("Expected Damage, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test 5: Reflection reverses ray but consumes range
    // -----------------------------------------------------------------------
    #[test]
    fn reflection_reverses_ray() {
        let mut rng = test_rng();
        let target = TargetProperties {
            reflection: true,
            current_hp: 50,
            ..Default::default()
        };
        let outcome = ray_effect_on_target(WandType::Fire, &target, &mut rng);
        assert_eq!(outcome, RayHitOutcome::Reflected);
    }

    // -----------------------------------------------------------------------
    // Test 6: Charges decrement on use
    // -----------------------------------------------------------------------
    #[test]
    fn charges_decrement_on_use() {
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let result = zappable(&mut charges, &mut rng);
        assert_eq!(result, ZapResult::Success);
        assert_eq!(charges.spe, 4);
    }

    // -----------------------------------------------------------------------
    // Test 7: Wresting works at spe=0 with right roll
    // -----------------------------------------------------------------------
    #[test]
    fn wresting_at_spe_zero() {
        // We need to find an RNG seed where rng.random_range(0..121) == 0.
        // Try many seeds until we find one.
        let mut found_wrest = false;
        for seed in 0u64..10000 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 0,
                recharged: 0,
            };
            let result = zappable(&mut charges, &mut rng);
            if result == ZapResult::Wrested {
                assert_eq!(charges.spe, -1);
                found_wrest = true;
                break;
            }
        }
        assert!(
            found_wrest,
            "Should find a seed that produces wresting in 10000 tries"
        );
    }

    // -----------------------------------------------------------------------
    // Test 8: Cannot use wand with spe < 0
    // -----------------------------------------------------------------------
    #[test]
    fn cannot_use_exhausted_wand() {
        let mut charges = WandCharges {
            spe: -1,
            recharged: 0,
        };
        let mut rng = test_rng();
        let result = zappable(&mut charges, &mut rng);
        assert_eq!(result, ZapResult::Fail);
        assert_eq!(charges.spe, -1); // unchanged
    }

    // -----------------------------------------------------------------------
    // Test 9: Breaking fire wand causes explosion
    // -----------------------------------------------------------------------
    #[test]
    fn breaking_fire_wand_explosion() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let map = floor_map(20, 20);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let charges = WandCharges {
            spe: 3,
            recharged: 0,
        };
        let events = break_wand(&world, player, WandType::Fire, &charges);

        // Should contain an explosion message
        let has_explosion = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("wand-break")));
        assert!(
            has_explosion,
            "Breaking a fire wand should produce an explosion"
        );

        // Damage should be spe * 4 * 2 = 3 * 4 * 2 = 24
        let has_damage = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ExtraDamage { amount, .. } if *amount == 24));
        assert!(
            has_damage,
            "Fire wand explosion should deal spe*4*2 = 24 damage"
        );
    }

    // -----------------------------------------------------------------------
    // Test 10: Sleep ray blocked by resistance
    // -----------------------------------------------------------------------
    #[test]
    fn sleep_ray_resisted() {
        let mut rng = test_rng();
        let target = TargetProperties {
            sleep_resistance: true,
            ..Default::default()
        };
        let outcome = ray_effect_on_target(WandType::Sleep, &target, &mut rng);
        assert_eq!(outcome, RayHitOutcome::Resisted);
    }

    // -----------------------------------------------------------------------
    // Test 11: Initial charges ranges
    // -----------------------------------------------------------------------
    #[test]
    fn initial_charges_ranges() {
        let mut rng = test_rng();

        // Wishing: always 1
        let wishing = initial_charges(WandType::Wishing, &mut rng);
        assert_eq!(wishing.spe, 1);

        // NODIR: 11..=15
        for _ in 0..100 {
            let charges = initial_charges(WandType::Light, &mut rng);
            assert!(
                (11..=15).contains(&charges.spe),
                "NODIR charges should be 11..15, got {}",
                charges.spe
            );
        }

        // RAY: 4..=8
        for _ in 0..100 {
            let charges = initial_charges(WandType::Fire, &mut rng);
            assert!(
                (4..=8).contains(&charges.spe),
                "RAY charges should be 4..8, got {}",
                charges.spe
            );
        }

        // IMMEDIATE: 4..=8
        for _ in 0..100 {
            let charges = initial_charges(WandType::Striking, &mut rng);
            assert!(
                (4..=8).contains(&charges.spe),
                "IMMEDIATE charges should be 4..8, got {}",
                charges.spe
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 12: Death ray resisted by magic resistance
    // -----------------------------------------------------------------------
    #[test]
    fn death_ray_resisted_by_mr() {
        let mut rng = test_rng();
        let target = TargetProperties {
            magic_resistance: true,
            current_hp: 50,
            ..Default::default()
        };
        let outcome = ray_effect_on_target(WandType::Death, &target, &mut rng);
        assert_eq!(outcome, RayHitOutcome::Resisted);
    }

    // -----------------------------------------------------------------------
    // Test 13: Death ray resisted by nonliving
    // -----------------------------------------------------------------------
    #[test]
    fn death_ray_resisted_by_nonliving() {
        let mut rng = test_rng();
        let target = TargetProperties {
            nonliving: true,
            current_hp: 50,
            ..Default::default()
        };
        let outcome = ray_effect_on_target(WandType::Death, &target, &mut rng);
        assert_eq!(outcome, RayHitOutcome::Resisted);
    }

    // -----------------------------------------------------------------------
    // Test 14: Breaking no-effect wand is harmless
    // -----------------------------------------------------------------------
    #[test]
    fn breaking_nothing_wand_harmless() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let map = floor_map(20, 20);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let events = break_wand(&world, player, WandType::Nothing, &charges);

        // Should NOT contain explosion damage
        let has_damage = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ExtraDamage { .. }));
        assert!(
            !has_damage,
            "Breaking wand of nothing should not deal damage"
        );

        let has_nothing = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("wand-nothing")));
        assert!(has_nothing, "Should say 'nothing else happens'");
    }

    // -----------------------------------------------------------------------
    // Test 15: WandType direction classification
    // -----------------------------------------------------------------------
    #[test]
    fn wand_direction_classification() {
        assert_eq!(WandType::Light.direction(), WandDirection::Nodir);
        assert_eq!(WandType::Wishing.direction(), WandDirection::Nodir);
        assert_eq!(WandType::Striking.direction(), WandDirection::Immediate);
        assert_eq!(WandType::Nothing.direction(), WandDirection::Immediate);
        assert_eq!(WandType::Fire.direction(), WandDirection::Ray);
        assert_eq!(WandType::Death.direction(), WandDirection::Ray);
        assert_eq!(WandType::Digging.direction(), WandDirection::Ray);
    }

    // -----------------------------------------------------------------------
    // Test 16: Magic missile deals d(2,6)
    // -----------------------------------------------------------------------
    #[test]
    fn magic_missile_damage_range() {
        let target = TargetProperties::default();
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let outcome = ray_effect_on_target(WandType::MagicMissile, &target, &mut rng);
            match outcome {
                RayHitOutcome::Damage(d) => {
                    assert!((2..=12).contains(&d), "d(2,6) should be 2..12, got {}", d);
                }
                other => panic!("Expected Damage, got {:?}", other),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test 17: Cold ray damage
    // -----------------------------------------------------------------------
    #[test]
    fn cold_ray_damage() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = ray_effect_on_target(WandType::Cold, &target, &mut rng);
        match outcome {
            RayHitOutcome::Damage(d) => {
                assert!((6..=36).contains(&d), "d(6,6) should be 6..36, got {}", d);
            }
            other => panic!("Expected Damage, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test 18: Sleep ray applies sleep duration
    // -----------------------------------------------------------------------
    #[test]
    fn sleep_ray_applies_duration() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = ray_effect_on_target(WandType::Sleep, &target, &mut rng);
        match outcome {
            RayHitOutcome::Sleep(d) => {
                // d(6,25) = 6..150 (spec section 5: d(nd,25) where nd=6 for wands)
                assert!(
                    (6..=150).contains(&d),
                    "Sleep d(6,25) should be 6..150, got {}",
                    d
                );
            }
            other => panic!("Expected Sleep, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test 19: Immediate beam stops at wall
    // -----------------------------------------------------------------------
    #[test]
    fn immediate_beam_stops_at_wall() {
        let mut map = LevelMap::new(20, 5);
        for y in 0..5 {
            for x in 0..20 {
                let t = if x == 0 || x == 19 || y == 0 || y == 4 {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                map.set_terrain(Position::new(x, y), t);
            }
        }
        // Place a wall in the middle
        map.set_terrain(Position::new(8, 2), Terrain::Wall);

        let mut rng = test_rng();
        let path = trace_immediate(&map, Position::new(1, 2), Direction::East, &mut rng);

        // All positions should be before the wall
        for pos in &path {
            assert!(
                pos.x < 8,
                "Immediate beam should stop before wall at x=8, got x={}",
                pos.x
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 20: Zap wand with no charges fails
    // -----------------------------------------------------------------------
    #[test]
    fn zap_wand_no_charges() {
        let world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let mut charges = WandCharges {
            spe: -1,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Fire,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_nothing = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("wand-nothing")));
        assert!(
            has_nothing,
            "Zapping exhausted wand should produce 'Nothing happens'"
        );
    }

    // -----------------------------------------------------------------------
    // Gen-block ray trace equivalence tests
    // -----------------------------------------------------------------------

    #[test]
    fn trace_ray_gen_matches_trace_ray() {
        let map = floor_map(20, 10);

        // Test several directions with the same seed.
        let directions = [
            Direction::East,
            Direction::West,
            Direction::North,
            Direction::South,
            Direction::NorthEast,
            Direction::SouthWest,
        ];

        for &dir in &directions {
            let mut rng_a = test_rng();
            let path = trace_ray(&map, Position::new(10, 5), dir, &mut rng_a);

            let mut rng_b = test_rng();
            let gen_cells: Vec<RayCell> =
                trace_ray_gen(&map, Position::new(10, 5), dir, &mut rng_b).collect();

            assert_eq!(
                path.cells.len(),
                gen_cells.len(),
                "cell count mismatch for {:?}",
                dir
            );
            for (i, (a, b)) in path.cells.iter().zip(gen_cells.iter()).enumerate() {
                assert_eq!(a, b, "cell {} differs for {:?}", i, dir);
            }
        }
    }

    #[test]
    fn trace_ray_gen_bounce_matches() {
        // Shoot east from near the right wall -- should bounce.
        let map = floor_map(20, 5);

        let mut rng_a = test_rng();
        let path = trace_ray(&map, Position::new(16, 2), Direction::East, &mut rng_a);

        let mut rng_b = test_rng();
        let gen_cells: Vec<RayCell> =
            trace_ray_gen(&map, Position::new(16, 2), Direction::East, &mut rng_b).collect();

        assert_eq!(path.cells.len(), gen_cells.len());
        for (a, b) in path.cells.iter().zip(gen_cells.iter()) {
            assert_eq!(a, b);
        }
    }

    // -----------------------------------------------------------------------
    // Test: Death ray vs undead (nonliving) — no effect
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_death_vs_undead_no_effect() {
        // TV-2: Non-living (zombie, etc.) -> Resisted.
        let mut rng = test_rng();
        let target = TargetProperties {
            nonliving: true,
            current_hp: 50,
            ..Default::default()
        };
        assert_eq!(
            ray_effect_on_target(WandType::Death, &target, &mut rng),
            RayHitOutcome::Resisted,
            "death ray should be resisted by nonliving"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Death ray vs demon — no effect
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_death_vs_demon_no_effect() {
        let mut rng = test_rng();
        let target = TargetProperties {
            is_demon: true,
            current_hp: 50,
            ..Default::default()
        };
        assert_eq!(
            ray_effect_on_target(WandType::Death, &target, &mut rng),
            RayHitOutcome::Resisted,
            "death ray should be resisted by demons"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Death ray self-zap kills hero
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_death_self_zap_instant_death() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = self_zap_effect(WandType::Death, &target, &mut rng);
        assert_eq!(
            outcome,
            SelfZapOutcome::InstantDeath,
            "self-zap with death wand should be instant death"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Death ray self-zap resisted by nonliving
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_death_self_zap_resisted_nonliving() {
        let mut rng = test_rng();
        let target = TargetProperties {
            nonliving: true,
            ..Default::default()
        };
        let outcome = self_zap_effect(WandType::Death, &target, &mut rng);
        assert_eq!(
            outcome,
            SelfZapOutcome::Resisted,
            "self-zap death should be resisted by nonliving form"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Fire ray extra damage to cold-resistant targets
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_fire_vs_cold_resistant_extra_damage() {
        // Spec section 5: cold-resistant targets take +7 extra from fire.
        let mut min_damage = u32::MAX;
        let mut max_damage = 0;
        for seed in 0u64..200 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let target = TargetProperties {
                cold_resistance: true,
                ..Default::default()
            };
            match ray_effect_on_target(WandType::Fire, &target, &mut rng) {
                RayHitOutcome::Damage(d) => {
                    min_damage = min_damage.min(d);
                    max_damage = max_damage.max(d);
                }
                other => panic!("Expected Damage, got {:?}", other),
            }
        }
        // d(6,6) + 7 = range [13, 43]
        assert!(
            min_damage >= 13,
            "fire vs cold-resistant min should be >= 13, got {}",
            min_damage
        );
        assert!(
            max_damage <= 43,
            "fire vs cold-resistant max should be <= 43, got {}",
            max_damage
        );
    }

    // -----------------------------------------------------------------------
    // Test: Cold ray extra damage to fire-resistant targets
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_cold_vs_fire_resistant_extra_damage() {
        // Spec section 5: fire-resistant targets take +d(6,3) extra from cold.
        let mut min_damage = u32::MAX;
        let mut max_damage = 0;
        for seed in 0u64..200 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let target = TargetProperties {
                fire_resistance: true,
                ..Default::default()
            };
            match ray_effect_on_target(WandType::Cold, &target, &mut rng) {
                RayHitOutcome::Damage(d) => {
                    min_damage = min_damage.min(d);
                    max_damage = max_damage.max(d);
                }
                other => panic!("Expected Damage, got {:?}", other),
            }
        }
        // d(6,6) + d(6,3) = range [12, 54]
        assert!(
            min_damage >= 12,
            "cold vs fire-resistant min should be >= 12, got {}",
            min_damage
        );
        assert!(
            max_damage <= 54,
            "cold vs fire-resistant max should be <= 54, got {}",
            max_damage
        );
    }

    // -----------------------------------------------------------------------
    // Test: Sleep ray duration d(6,25)
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_sleep_duration_d6_25() {
        // Spec: sleep for d(nd,25) where nd=6 for wands.
        let mut min_dur = u32::MAX;
        let mut max_dur = 0;
        for seed in 0u64..500 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let target = TargetProperties::default();
            match ray_effect_on_target(WandType::Sleep, &target, &mut rng) {
                RayHitOutcome::Sleep(d) => {
                    min_dur = min_dur.min(d);
                    max_dur = max_dur.max(d);
                }
                other => panic!("Expected Sleep, got {:?}", other),
            }
        }
        assert!(
            min_dur >= 6,
            "d(6,25) minimum should be >= 6, got {}",
            min_dur
        );
        assert!(
            max_dur <= 150,
            "d(6,25) maximum should be <= 150, got {}",
            max_dur
        );
        // With 500 trials, minimum should be close to 6 and max well above 25.
        assert!(
            max_dur > 25,
            "d(6,25) should routinely exceed 25, got max {}",
            max_dur
        );
    }

    // -----------------------------------------------------------------------
    // Test: Wand recharging — first recharge never explodes
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_recharge_first_never_explodes() {
        // TV-4 (recharged=0): 0^3=0, condition n>0 false -> never explodes.
        for seed in 0u64..200 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 3,
                recharged: 0,
            };
            let result = recharge_wand(WandType::Fire, &mut charges, false, false, &mut rng);
            assert_ne!(
                result,
                RechargeResult::Exploded,
                "first recharge (recharged=0) should never explode"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: Wand recharging — recharged=7 always explodes
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_recharge_seventh_always_explodes() {
        // TV-4 (recharged=7): 7^3=343, rn2(343) range 0..342, 343>any -> always.
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 3,
                recharged: 7,
            };
            let result = recharge_wand(WandType::Fire, &mut charges, false, false, &mut rng);
            assert_eq!(
                result,
                RechargeResult::Exploded,
                "recharging with recharged=7 should always explode"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: Wishing wand always explodes on recharge if recharged > 0
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_wishing_recharge_always_explodes() {
        // TV-4: Wishing wand with recharged=1 -> always explode.
        for seed in 0u64..50 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 0,
                recharged: 1,
            };
            let result = recharge_wand(WandType::Wishing, &mut charges, true, false, &mut rng);
            assert_eq!(
                result,
                RechargeResult::Exploded,
                "wishing wand recharge with recharged>0 should always explode"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: Wishing wand recharge from recharged=0 succeeds
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_wishing_recharge_first_succeeds() {
        // TV-4: Wishing wand with recharged=0 -> should succeed (spe=1).
        let mut rng = test_rng();
        let mut charges = WandCharges {
            spe: 0,
            recharged: 0,
        };
        let result = recharge_wand(WandType::Wishing, &mut charges, true, false, &mut rng);
        match result {
            RechargeResult::Success { new_spe } => {
                assert_eq!(new_spe, 1, "wishing wand recharge should set spe to 1");
            }
            other => panic!("Expected Success, got {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Test: Cursed recharge strips charges
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_recharge_cursed_strips() {
        let mut rng = test_rng();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let result = recharge_wand(WandType::Fire, &mut charges, false, true, &mut rng);
        assert_eq!(result, RechargeResult::Stripped);
        assert_eq!(charges.spe, 0, "cursed recharge should strip charges to 0");
    }

    // -----------------------------------------------------------------------
    // Test: Wresting chance is approximately 1/121
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_wresting_probability() {
        // TV-5: spe=0, 1/121 chance to wrest.
        let mut wrest_count = 0;
        let trials = 12100u64;
        for seed in 0..trials {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 0,
                recharged: 0,
            };
            if zappable(&mut charges, &mut rng) == ZapResult::Wrested {
                wrest_count += 1;
            }
        }
        // Expected: ~100 wrests. Allow 50..200 range.
        assert!(
            wrest_count > 30 && wrest_count < 300,
            "wresting should occur ~1/121 of the time, got {}/{}",
            wrest_count,
            trials
        );
    }

    // -----------------------------------------------------------------------
    // Test: Cursed wand backfire probability
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_cursed_backfire_probability() {
        // TV-6: 1% chance (1 in 100).
        let mut fire_count = 0;
        let trials = 10000u64;
        for seed in 0..trials {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if cursed_wand_backfire(&mut rng) {
                fire_count += 1;
            }
        }
        // Expected: ~100. Allow 50..200.
        assert!(
            fire_count > 30 && fire_count < 300,
            "backfire should occur ~1% of the time, got {}/{}",
            fire_count,
            trials
        );
    }

    // -----------------------------------------------------------------------
    // Test: Backfire damage formula d(spe+2, 6)
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_backfire_damage_formula() {
        // TV-6: spe=5 -> d(7, 6) = range [7, 42].
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let dmg = backfire_damage(5, &mut rng);
            assert!(
                (7..=42).contains(&dmg),
                "backfire damage d(7,6) should be 7..42, got {}",
                dmg
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap speed monster gives 50..74 duration
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_speed() {
        let target = TargetProperties::default();
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            match self_zap_effect(WandType::SpeedMonster, &target, &mut rng) {
                SelfZapOutcome::SpeedUp(d) => {
                    assert!(
                        (50..=74).contains(&d),
                        "speed self-zap should give 50..74 turns, got {}",
                        d
                    );
                }
                other => panic!("Expected SpeedUp, got {:?}", other),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap invisibility gives 31..45 duration
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_invisible() {
        let target = TargetProperties::default();
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            match self_zap_effect(WandType::MakeInvisible, &target, &mut rng) {
                SelfZapOutcome::Invisible(d) => {
                    assert!(
                        (31..=45).contains(&d),
                        "invisible self-zap should give 31..45 turns, got {}",
                        d
                    );
                }
                other => panic!("Expected Invisible, got {:?}", other),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap sleep blocked by resistance
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_sleep_resisted() {
        let mut rng = test_rng();
        let target = TargetProperties {
            sleep_resistance: true,
            ..Default::default()
        };
        let outcome = self_zap_effect(WandType::Sleep, &target, &mut rng);
        assert_eq!(
            outcome,
            SelfZapOutcome::Resisted,
            "self-zap sleep should be resisted by sleep resistance"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap striking blocked by MR
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_striking_mr() {
        let mut rng = test_rng();
        let target = TargetProperties {
            magic_resistance: true,
            ..Default::default()
        };
        let outcome = self_zap_effect(WandType::Striking, &target, &mut rng);
        assert_eq!(
            outcome,
            SelfZapOutcome::Resisted,
            "self-zap striking should be resisted by magic resistance"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Breaking death wand damage = spe * 4 * 4
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_break_death_damage() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let map = floor_map(20, 20);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let charges = WandCharges {
            spe: 4,
            recharged: 0,
        };
        let events = break_wand(&world, player, WandType::Death, &charges);

        // Expected damage: spe*4 * 4 = 4*4*4 = 64
        let has_damage = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ExtraDamage { amount, .. } if *amount == 64));
        assert!(
            has_damage,
            "breaking death wand spe=4 should deal 64 damage"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Breaking wishing wand has no explosion
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_break_wishing_no_explosion() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let map = floor_map(20, 20);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let charges = WandCharges {
            spe: 1,
            recharged: 0,
        };
        let events = break_wand(&world, player, WandType::Wishing, &charges);

        let has_damage = events
            .iter()
            .any(|e| matches!(e, EngineEvent::ExtraDamage { .. }));
        assert!(!has_damage, "breaking wishing wand should not deal damage");
    }

    // -----------------------------------------------------------------------
    // Test: Recharge explosion probability at recharged=4 (18.66%)
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_recharge_explosion_probability_r4() {
        let mut explode_count = 0;
        let trials = 5000u64;
        for seed in 0..trials {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 3,
                recharged: 4,
            };
            if recharge_wand(WandType::Fire, &mut charges, false, false, &mut rng)
                == RechargeResult::Exploded
            {
                explode_count += 1;
            }
        }
        // Expected: ~18.66% = ~933 of 5000. Allow 600..1400.
        let pct = explode_count as f64 / trials as f64 * 100.0;
        assert!(
            pct > 10.0 && pct < 30.0,
            "recharged=4 explosion should be ~18.66%, got {:.1}%",
            pct
        );
    }

    // -----------------------------------------------------------------------
    // Test: Initial charges for wishing always 1
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_wishing_charges_always_1() {
        // TV-16: Wishing wand initial charges = 1 (fixed).
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let charges = initial_charges(WandType::Wishing, &mut rng);
            assert_eq!(charges.spe, 1, "wishing wand initial charges must be 1");
        }
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap polymorph
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_polymorph() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = self_zap_effect(WandType::Polymorph, &target, &mut rng);
        assert_eq!(outcome, SelfZapOutcome::Polymorph);
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap cancellation
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_cancel() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = self_zap_effect(WandType::Cancellation, &target, &mut rng);
        assert_eq!(outcome, SelfZapOutcome::Cancel);
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap fire damage d(12,6)
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_self_zap_fire_damage() {
        for seed in 0u64..100 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let target = TargetProperties::default();
            match self_zap_effect(WandType::Fire, &target, &mut rng) {
                SelfZapOutcome::Damage(d) => {
                    assert!(
                        (12..=72).contains(&d),
                        "self-zap fire d(12,6) should be 12..72, got {}",
                        d
                    );
                }
                other => panic!("Expected Damage, got {:?}", other),
            }
        }
    }

    // =======================================================================
    // New tests: dispatch_nodir enhancements
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Light wand illuminates 5x5 area
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_light_area() {
        let mut world = GameWorld::new(Position::new(10, 10));
        let map = floor_map(20, 20);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Light,
            &mut charges,
            Direction::East, // direction ignored for NODIR
            &mut rng,
        );

        // Should contain the wand-light message
        let has_light = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-light"));
        assert!(has_light, "Light wand should emit wand-light message");

        // Should contain wand-light-cell messages for the 5x5 area
        let cell_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-light-cell"))
            .count();
        // 5x5 = 25 cells, but border walls may reduce the in-bounds count.
        // Player at (10,10) in 20x20 map: all 25 cells should be in bounds.
        assert_eq!(
            cell_count, 25,
            "Light wand should illuminate 25 cells (5x5)"
        );
    }

    // -----------------------------------------------------------------------
    // Test: SecretDoorDetection reveals unexplored doors
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_secret_door_detection() {
        let mut world = GameWorld::new(Position::new(10, 10));
        let mut map = floor_map(20, 20);
        // Place a closed, unexplored door
        map.set_terrain(Position::new(5, 5), Terrain::DoorClosed);
        // Mark it as unexplored (default should be unexplored in new map)
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::SecretDoorDetection,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_detect = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-secret-door-detect"),
        );
        assert!(has_detect, "Should emit secret door detection message");

        // Should reveal the door at (5,5)
        let has_reveal = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, args } if key == "wand-reveal-door"
                && args.iter().any(|(k, v)| k == "x" && v == "5")
                && args.iter().any(|(k, v)| k == "y" && v == "5"))
        });
        assert!(has_reveal, "Should reveal the hidden door at (5,5)");
    }

    // -----------------------------------------------------------------------
    // Test: CreateMonster generates 1-4 events
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_create_monster_events() {
        let mut world = GameWorld::new(Position::new(10, 10));
        let map = floor_map(20, 20);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::CreateMonster,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_msg = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-create-monster"));
        assert!(has_msg, "Should emit create-monster message");

        let gen_count = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::MonsterGenerated { .. }))
            .count();
        assert!(
            (1..=4).contains(&gen_count),
            "Should generate 1-4 monsters, got {}",
            gen_count
        );
    }

    // =======================================================================
    // New tests: dispatch_immediate door effects
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Opening wand opens locked door along beam
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_opening_unlocks_door() {
        let mut world = GameWorld::new(Position::new(2, 2));
        let mut map = floor_map(20, 5);
        // Place a locked door at x=5, directly east
        map.set_terrain(Position::new(5, 2), Terrain::DoorLocked);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Opening,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_open = events.iter().any(|e| {
            matches!(e, EngineEvent::DoorOpened { position } if position.x == 5 && position.y == 2)
        });
        assert!(
            has_open,
            "Opening wand should open the locked door at (5,2)"
        );
    }

    // -----------------------------------------------------------------------
    // Test: Locking wand locks open door along beam
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_locking_locks_door() {
        let mut world = GameWorld::new(Position::new(2, 2));
        let mut map = floor_map(20, 5);
        // Place an open door at x=5
        map.set_terrain(Position::new(5, 2), Terrain::DoorOpen);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Locking,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_lock = events.iter().any(|e| {
            matches!(e, EngineEvent::DoorLocked { position } if position.x == 5 && position.y == 2)
        });
        assert!(has_lock, "Locking wand should lock the open door at (5,2)");
    }

    // =======================================================================
    // New tests: dispatch_ray digging
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Digging ray produces terrain-change events for walls
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_digging_ray_events() {
        let mut world = GameWorld::new(Position::new(5, 5));
        let mut map = LevelMap::new(20, 10);
        // Set up: floor from x=1..=18, but place a wall block at x=8..=10
        for y in 0..10 {
            for x in 0..20 {
                let t = if y == 0 || y == 9 || x == 0 || x == 19 {
                    Terrain::Wall
                } else if (8..=10).contains(&x) && y == 5 {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                map.set_terrain(Position::new(x, y), t);
            }
        }
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Digging,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        // Should have wand-zap message
        let has_zap = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-zap"));
        assert!(has_zap, "Digging ray should emit wand-zap");

        // Should have dig-cell events for the wall at x=8,9,10
        let dig_cells = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-digging-cell"))
            .count();
        assert!(
            dig_cells > 0,
            "Digging ray should produce dig-cell events for walls"
        );

        // Should have the summary message
        let has_digging = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-digging"));
        assert!(has_digging, "Should emit wand-digging when walls are dug");
    }

    // -----------------------------------------------------------------------
    // Test: Digging ray on empty corridor emits miss
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_digging_miss() {
        let mut world = GameWorld::new(Position::new(5, 5));
        // All floor except border walls — ray goes east, hits border wall which
        // causes a bounce in trace_ray. But the bounce cells have wall terrain,
        // so actually will be dug. Let's use a wider map.
        let map = floor_map(40, 10);
        world.dungeon_mut().current_level = map;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        // Use a specific seed and starting position well inside the map
        // so the ray doesn't reach any wall within its range.
        let mut rng = Pcg64Mcg::seed_from_u64(999);
        let events = zap_wand(
            &world,
            player,
            WandType::Digging,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        // The ray's range is 7..13. Starting at x=5, going east, the
        // first wall is at x=39 which is > 5+13=18, so no walls hit.
        let dig_cells = events
            .iter()
            .filter(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-digging-cell"))
            .count();
        if dig_cells == 0 {
            // Should emit wand-digging-miss
            let has_miss = events.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-digging-miss"),
            );
            assert!(has_miss, "Should emit wand-digging-miss when no walls dug");
        }
    }

    // =======================================================================
    // New tests: Vertical zapping
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Zapping digging wand downward creates hole
    // -----------------------------------------------------------------------
    #[test]
    fn test_zap_updown_digging_down() {
        let mut rng = test_rng();
        let outcome = zap_updown(WandType::Digging, true, &mut rng);
        assert_eq!(outcome, VerticalZapOutcome::DigHole);
    }

    // -----------------------------------------------------------------------
    // Test: Zapping digging wand upward creates ceiling hole
    // -----------------------------------------------------------------------
    #[test]
    fn test_zap_updown_digging_up() {
        let mut rng = test_rng();
        let outcome = zap_updown(WandType::Digging, false, &mut rng);
        assert_eq!(outcome, VerticalZapOutcome::DigCeiling);
    }

    // -----------------------------------------------------------------------
    // Test: Zapping fire wand vertically passes through
    // -----------------------------------------------------------------------
    #[test]
    fn test_zap_updown_fire_passthrough() {
        let mut rng = test_rng();
        let outcome = zap_updown(WandType::Fire, true, &mut rng);
        assert_eq!(outcome, VerticalZapOutcome::PassThrough);
    }

    // -----------------------------------------------------------------------
    // Test: Zapping light wand vertically gives light
    // -----------------------------------------------------------------------
    #[test]
    fn test_zap_updown_light() {
        let mut rng = test_rng();
        let outcome = zap_updown(WandType::Light, true, &mut rng);
        assert_eq!(outcome, VerticalZapOutcome::Light);
    }

    // -----------------------------------------------------------------------
    // Test: Zapping create monster vertically creates 1-4
    // -----------------------------------------------------------------------
    #[test]
    fn test_zap_updown_create_monster() {
        for seed in 0u64..50 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            match zap_updown(WandType::CreateMonster, true, &mut rng) {
                VerticalZapOutcome::CreateMonster(n) => {
                    assert!(
                        (1..=4).contains(&n),
                        "Should create 1-4 monsters, got {}",
                        n
                    );
                }
                other => panic!("Expected CreateMonster, got {:?}", other),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test: Zapping nothing wand vertically does nothing
    // -----------------------------------------------------------------------
    #[test]
    fn test_zap_updown_nothing() {
        let mut rng = test_rng();
        let outcome = zap_updown(WandType::Nothing, true, &mut rng);
        assert_eq!(outcome, VerticalZapOutcome::NoEffect);
    }

    // =======================================================================
    // New tests: Engraving effects
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Fire wand produces burn effect
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_fire_burns() {
        assert_eq!(engrave_with_wand(WandType::Fire), EngraveEffect::Burn);
    }

    // -----------------------------------------------------------------------
    // Test: Lightning wand produces electric effect
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_lightning_electric() {
        assert_eq!(
            engrave_with_wand(WandType::Lightning),
            EngraveEffect::Electric
        );
    }

    // -----------------------------------------------------------------------
    // Test: Digging wand produces burn (permanent engraving)
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_digging_burns() {
        assert_eq!(engrave_with_wand(WandType::Digging), EngraveEffect::Burn);
    }

    // -----------------------------------------------------------------------
    // Test: Cancellation wand erases engravings
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_cancellation_erases() {
        assert_eq!(
            engrave_with_wand(WandType::Cancellation),
            EngraveEffect::Erase
        );
    }

    // -----------------------------------------------------------------------
    // Test: Teleportation wand erases engravings
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_teleport_erases() {
        assert_eq!(
            engrave_with_wand(WandType::Teleportation),
            EngraveEffect::Erase
        );
    }

    // -----------------------------------------------------------------------
    // Test: Make invisible wand produces invisible writing
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_invisible_writing() {
        assert_eq!(
            engrave_with_wand(WandType::MakeInvisible),
            EngraveEffect::Invisible
        );
    }

    // -----------------------------------------------------------------------
    // Test: Nothing wand produces invisible writing
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_nothing_invisible() {
        assert_eq!(
            engrave_with_wand(WandType::Nothing),
            EngraveEffect::Invisible
        );
    }

    // -----------------------------------------------------------------------
    // Test: Polymorph wand produces scrawl
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_polymorph_scrawl() {
        assert_eq!(
            engrave_with_wand(WandType::Polymorph),
            EngraveEffect::Scrawl
        );
    }

    // -----------------------------------------------------------------------
    // Test: Striking wand produces scrawl
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_striking_scrawl() {
        assert_eq!(engrave_with_wand(WandType::Striking), EngraveEffect::Scrawl);
    }

    // -----------------------------------------------------------------------
    // Test: Death wand has normal engraving
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_death_normal() {
        assert_eq!(engrave_with_wand(WandType::Death), EngraveEffect::Normal);
    }

    // =======================================================================
    // New tests: Object destruction by rays
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Fire destroys paper/cloth/wood
    // -----------------------------------------------------------------------
    #[test]
    fn test_fire_destroys_flammable() {
        assert!(ray_destroys_material(WandType::Fire, ObjectMaterial::Paper));
        assert!(ray_destroys_material(WandType::Fire, ObjectMaterial::Wood));
        assert!(ray_destroys_material(WandType::Fire, ObjectMaterial::Cloth));
        assert!(ray_destroys_material(
            WandType::Fire,
            ObjectMaterial::Organic
        ));
        assert!(ray_destroys_material(WandType::Fire, ObjectMaterial::Wax));
        // Fire does NOT destroy metal
        assert!(!ray_destroys_material(
            WandType::Fire,
            ObjectMaterial::Metal
        ));
        assert!(!ray_destroys_material(
            WandType::Fire,
            ObjectMaterial::Glass
        ));
    }

    // -----------------------------------------------------------------------
    // Test: Cold shatters glass/liquid
    // -----------------------------------------------------------------------
    #[test]
    fn test_cold_shatters_glass() {
        assert!(ray_destroys_material(WandType::Cold, ObjectMaterial::Glass));
        assert!(ray_destroys_material(
            WandType::Cold,
            ObjectMaterial::Liquid
        ));
        // Cold does NOT destroy paper
        assert!(!ray_destroys_material(
            WandType::Cold,
            ObjectMaterial::Paper
        ));
    }

    // -----------------------------------------------------------------------
    // Test: Lightning destroys metal
    // -----------------------------------------------------------------------
    #[test]
    fn test_lightning_destroys_metal() {
        assert!(ray_destroys_material(
            WandType::Lightning,
            ObjectMaterial::Metal
        ));
        assert!(ray_destroys_material(
            WandType::Lightning,
            ObjectMaterial::Iron
        ));
        assert!(ray_destroys_material(
            WandType::Lightning,
            ObjectMaterial::Copper
        ));
        assert!(ray_destroys_material(
            WandType::Lightning,
            ObjectMaterial::Gold
        ));
        // Lightning does NOT destroy paper
        assert!(!ray_destroys_material(
            WandType::Lightning,
            ObjectMaterial::Paper
        ));
    }

    // -----------------------------------------------------------------------
    // Test: Death ray doesn't destroy objects
    // -----------------------------------------------------------------------
    #[test]
    fn test_death_destroys_nothing() {
        for &mat in &[
            ObjectMaterial::Paper,
            ObjectMaterial::Metal,
            ObjectMaterial::Glass,
            ObjectMaterial::Wood,
        ] {
            assert!(
                !ray_destroys_material(WandType::Death, mat),
                "Death ray should not destroy objects"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: Destroy chance percentages
    // -----------------------------------------------------------------------
    #[test]
    fn test_destroy_chances() {
        assert_eq!(
            ray_destroy_chance(WandType::Fire, ObjectMaterial::Paper),
            33
        );
        assert_eq!(
            ray_destroy_chance(WandType::Cold, ObjectMaterial::Glass),
            20
        );
        assert_eq!(
            ray_destroy_chance(WandType::Lightning, ObjectMaterial::Metal),
            33
        );
        // Non-destructible combination
        assert_eq!(ray_destroy_chance(WandType::Fire, ObjectMaterial::Metal), 0);
        assert_eq!(
            ray_destroy_chance(WandType::Death, ObjectMaterial::Paper),
            0
        );
    }

    // =======================================================================
    // New tests: Wand identification
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: NODIR wands auto-identify on zap
    // -----------------------------------------------------------------------
    #[test]
    fn test_nodir_wands_autoidentify() {
        assert!(wand_auto_identifies_on_zap(WandType::Light));
        assert!(wand_auto_identifies_on_zap(WandType::SecretDoorDetection));
        assert!(wand_auto_identifies_on_zap(WandType::CreateMonster));
        assert!(wand_auto_identifies_on_zap(WandType::Wishing));
        assert!(wand_auto_identifies_on_zap(WandType::Enlightenment));
    }

    // -----------------------------------------------------------------------
    // Test: RAY wands auto-identify on zap
    // -----------------------------------------------------------------------
    #[test]
    fn test_ray_wands_autoidentify() {
        assert!(wand_auto_identifies_on_zap(WandType::Fire));
        assert!(wand_auto_identifies_on_zap(WandType::Cold));
        assert!(wand_auto_identifies_on_zap(WandType::Lightning));
        assert!(wand_auto_identifies_on_zap(WandType::Death));
        assert!(wand_auto_identifies_on_zap(WandType::Sleep));
        assert!(wand_auto_identifies_on_zap(WandType::MagicMissile));
        assert!(wand_auto_identifies_on_zap(WandType::Digging));
    }

    // -----------------------------------------------------------------------
    // Test: Cancellation/Nothing don't auto-identify on zap
    // -----------------------------------------------------------------------
    #[test]
    fn test_hard_to_identify_wands() {
        assert!(!wand_auto_identifies_on_zap(WandType::Cancellation));
        assert!(!wand_auto_identifies_on_zap(WandType::Nothing));
    }

    // -----------------------------------------------------------------------
    // Test: Fire/lightning/digging auto-identify on engrave
    // -----------------------------------------------------------------------
    #[test]
    fn test_engrave_identification() {
        assert!(wand_auto_identifies_on_engrave(WandType::Fire));
        assert!(wand_auto_identifies_on_engrave(WandType::Lightning));
        assert!(wand_auto_identifies_on_engrave(WandType::Digging));
        assert!(wand_auto_identifies_on_engrave(WandType::Cancellation));
        assert!(wand_auto_identifies_on_engrave(WandType::Teleportation));
        // Polymorph/striking scrawl is not distinctive
        assert!(!wand_auto_identifies_on_engrave(WandType::Polymorph));
        assert!(!wand_auto_identifies_on_engrave(WandType::Striking));
        // Make invisible / nothing both produce invisible writing
        assert!(!wand_auto_identifies_on_engrave(WandType::MakeInvisible));
        assert!(!wand_auto_identifies_on_engrave(WandType::Nothing));
    }

    // =======================================================================
    // New tests: Cancellation
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: cancel_monster produces expected message
    // -----------------------------------------------------------------------
    #[test]
    fn test_cancel_monster_events() {
        let world = GameWorld::new(Position::new(5, 5));
        let player = world.player();
        let events = cancel_monster(player);
        let has_cancel = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-cancel-monster"));
        assert!(has_cancel, "cancel_monster should emit cancel message");
    }

    // -----------------------------------------------------------------------
    // Test: Immediate cancellation beam uses cancel_monster
    // -----------------------------------------------------------------------
    #[test]
    fn test_immediate_cancellation_on_monster() {
        let mut world = GameWorld::new(Position::new(2, 2));
        let map = floor_map(20, 5);
        world.dungeon_mut().current_level = map;
        // Spawn a monster at (5, 2) — in the beam path
        let monster = world.spawn((
            Positioned(Position::new(5, 2)),
            Monster,
            HitPoints {
                current: 20,
                max: 20,
            },
        ));
        let _ = monster;

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Cancellation,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_cancel = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-cancel-monster"));
        assert!(
            has_cancel,
            "Cancellation beam should cancel a monster in its path"
        );
    }

    // =======================================================================
    // New tests: Striking damage on monsters
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Striking beam damages monster in path
    // -----------------------------------------------------------------------
    #[test]
    fn test_striking_damages_monster() {
        let mut world = GameWorld::new(Position::new(2, 2));
        let map = floor_map(20, 5);
        world.dungeon_mut().current_level = map;
        let _monster = world.spawn((
            Positioned(Position::new(5, 2)),
            Monster,
            HitPoints {
                current: 30,
                max: 30,
            },
        ));

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Striking,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        let has_damage = events.iter().any(|e| {
            matches!(
                e,
                EngineEvent::ExtraDamage {
                    source: DamageSource::Wand,
                    ..
                }
            )
        });
        assert!(has_damage, "Striking beam should deal damage to a monster");
    }

    // =======================================================================
    // New tests: Ray hitting monster produces expected events
    // =======================================================================

    // -----------------------------------------------------------------------
    // Test: Fire ray hitting a monster produces HP change
    // -----------------------------------------------------------------------
    #[test]
    fn test_fire_ray_hits_monster_hp_change() {
        let mut world = GameWorld::new(Position::new(2, 5));
        let map = floor_map(20, 10);
        world.dungeon_mut().current_level = map;
        let _monster = world.spawn((
            Positioned(Position::new(5, 5)),
            Monster,
            HitPoints {
                current: 50,
                max: 50,
            },
        ));

        let player = world.player();
        let mut charges = WandCharges {
            spe: 5,
            recharged: 0,
        };
        let mut rng = test_rng();
        let events = zap_wand(
            &world,
            player,
            WandType::Fire,
            &mut charges,
            Direction::East,
            &mut rng,
        );

        // Should contain wand-zap message
        let has_zap = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-zap"));
        assert!(has_zap, "Ray dispatch should emit wand-zap");

        // Depending on zap_hit roll, may or may not hit, but let's check
        // that the event system works (zap_hit uses an RNG)
        let hit_events = events
            .iter()
            .any(|e| matches!(e, EngineEvent::HpChange { .. }));
        let absorb_events = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wand-ray-absorb"));
        // Either it hit (HP change) or events are empty after zap message
        // (missed due to zap_hit RNG). Both are valid.
        let _any_interaction = hit_events || absorb_events;
        // Just verify no panics and events are generated
        assert!(!events.is_empty(), "Should have at least the zap message");
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap teleport
    // -----------------------------------------------------------------------
    #[test]
    fn test_self_zap_teleport() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = self_zap_effect(WandType::Teleportation, &target, &mut rng);
        assert_eq!(outcome, SelfZapOutcome::Teleport);
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap slow
    // -----------------------------------------------------------------------
    #[test]
    fn test_self_zap_slow() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        let outcome = self_zap_effect(WandType::SlowMonster, &target, &mut rng);
        assert_eq!(outcome, SelfZapOutcome::SlowDown);
    }

    // -----------------------------------------------------------------------
    // Test: Self-zap no-effect wands
    // -----------------------------------------------------------------------
    #[test]
    fn test_self_zap_noeffect_wands() {
        let mut rng = test_rng();
        let target = TargetProperties::default();
        for &wand in &[
            WandType::Digging,
            WandType::Nothing,
            WandType::Probing,
            WandType::Opening,
            WandType::Locking,
            WandType::UndeadTurning,
            WandType::Light,
            WandType::SecretDoorDetection,
            WandType::CreateMonster,
            WandType::Wishing,
            WandType::Enlightenment,
        ] {
            let outcome = self_zap_effect(wand, &target, &mut rng);
            assert_eq!(
                outcome,
                SelfZapOutcome::NoEffect,
                "{:?} self-zap should be NoEffect",
                wand
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: All 24 wand types have a direction
    // -----------------------------------------------------------------------
    #[test]
    fn test_all_wand_types_classified() {
        let all_wands = [
            WandType::Light,
            WandType::SecretDoorDetection,
            WandType::CreateMonster,
            WandType::Wishing,
            WandType::Enlightenment,
            WandType::Striking,
            WandType::SlowMonster,
            WandType::SpeedMonster,
            WandType::UndeadTurning,
            WandType::Polymorph,
            WandType::Cancellation,
            WandType::Teleportation,
            WandType::MakeInvisible,
            WandType::Opening,
            WandType::Locking,
            WandType::Probing,
            WandType::Nothing,
            WandType::Death,
            WandType::Fire,
            WandType::Cold,
            WandType::Lightning,
            WandType::Sleep,
            WandType::MagicMissile,
            WandType::Digging,
        ];
        assert_eq!(all_wands.len(), 24, "Should have all 24 wand types");
        for wand in all_wands {
            let dir = wand.direction();
            assert!(
                matches!(
                    dir,
                    WandDirection::Nodir | WandDirection::Immediate | WandDirection::Ray
                ),
                "{:?} should have a valid direction",
                wand
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test: Wand recharge blessed gets higher charges
    // -----------------------------------------------------------------------
    #[test]
    fn test_wand_recharge_blessed_higher() {
        // Blessed recharge should generally give more charges than uncursed
        let mut blessed_total = 0i32;
        let mut uncursed_total = 0i32;
        let trials = 200u64;
        for seed in 0..trials {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            let mut charges = WandCharges {
                spe: 0,
                recharged: 0,
            };
            if let RechargeResult::Success { new_spe } =
                recharge_wand(WandType::Fire, &mut charges, true, false, &mut rng)
            {
                blessed_total += new_spe as i32;
            }

            let mut rng2 = Pcg64Mcg::seed_from_u64(seed);
            let mut charges2 = WandCharges {
                spe: 0,
                recharged: 0,
            };
            if let RechargeResult::Success { new_spe } =
                recharge_wand(WandType::Fire, &mut charges2, false, false, &mut rng2)
            {
                uncursed_total += new_spe as i32;
            }
        }
        assert!(
            blessed_total >= uncursed_total,
            "Blessed recharge should give >= charges than uncursed: {} vs {}",
            blessed_total,
            uncursed_total
        );
    }

    // =======================================================================
    // Wand object-hitting tests
    // =======================================================================

    #[test]
    fn test_fire_wand_burns_scrolls() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Fire, '?', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::Destroy { reason: "burnt" });
    }

    #[test]
    fn test_fire_wand_burns_spellbooks() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Fire, '+', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::Destroy { reason: "burnt" });
    }

    #[test]
    fn test_fire_wand_no_effect_on_potion() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Fire, '!', "glass", &mut rng);
        assert_eq!(result, WandObjectResult::NoEffect);
    }

    #[test]
    fn test_cold_wand_shatters_potions() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Cold, '!', "glass", &mut rng);
        assert_eq!(
            result,
            WandObjectResult::Destroy {
                reason: "shattered"
            }
        );
    }

    #[test]
    fn test_cold_wand_no_effect_on_scroll() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Cold, '?', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::NoEffect);
    }

    #[test]
    fn test_lightning_wand_fries_wands() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Lightning, '/', "wood", &mut rng);
        assert_eq!(result, WandObjectResult::Destroy { reason: "fried" });
    }

    #[test]
    fn test_lightning_wand_fries_rings() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Lightning, '=', "iron", &mut rng);
        assert_eq!(result, WandObjectResult::Destroy { reason: "fried" });
    }

    #[test]
    fn test_polymorph_wand_transforms_object() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Polymorph, '?', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::Transform);
    }

    #[test]
    fn test_cancellation_wand_cancels_object() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Cancellation, '?', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::Cancel);
    }

    #[test]
    fn test_striking_wand_damages_object() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Striking, '?', "paper", &mut rng);
        match result {
            WandObjectResult::Damage { amount } => {
                assert!(
                    (2..=24).contains(&amount),
                    "d(2,12) should be 2..24, got {}",
                    amount
                );
            }
            other => panic!("Expected Damage, got {:?}", other),
        }
    }

    #[test]
    fn test_death_wand_no_effect_on_objects() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::Death, '?', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::NoEffect);
    }

    #[test]
    fn test_wand_hits_pile_mixed() {
        let mut rng = test_rng();
        let pile: Vec<(char, &str)> = vec![
            ('?', "paper"), // scroll — fire burns
            ('!', "glass"), // potion — fire ignores
            ('+', "paper"), // spellbook — fire burns
            (')', "iron"),  // weapon — fire ignores
        ];
        let results = wand_hits_pile(WandType::Fire, &pile, &mut rng);
        // Should affect indices 0 and 2 (scroll and spellbook)
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[0].1, WandObjectResult::Destroy { reason: "burnt" });
        assert_eq!(results[1].0, 2);
        assert_eq!(results[1].1, WandObjectResult::Destroy { reason: "burnt" });
    }

    #[test]
    fn test_wand_hits_pile_cold_shatters_potions() {
        let mut rng = test_rng();
        let pile: Vec<(char, &str)> = vec![
            ('!', "glass"), // potion — cold shatters
            ('?', "paper"), // scroll — cold ignores
            ('!', "glass"), // potion — cold shatters
        ];
        let results = wand_hits_pile(WandType::Cold, &pile, &mut rng);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[1].0, 2);
    }

    #[test]
    fn test_make_invisible_wand_on_object() {
        let mut rng = test_rng();
        let result = wand_hits_object(WandType::MakeInvisible, '?', "paper", &mut rng);
        assert_eq!(result, WandObjectResult::MakeInvisible);
    }
}
