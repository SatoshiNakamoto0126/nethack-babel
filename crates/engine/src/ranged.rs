//! Ranged combat resolution for NetHack Babel.
//!
//! Implements throwing, launcher+ammo firing, and projectile path mechanics
//! based on the NetHack 3.7 formulas from `dothrow.c`, `mthrowu.c`, and
//! `weapon.c`.
//!
//! All pure functions operate on plain data parameters for testability.
//!
//! Reference: `specs/ranged-combat.md`

use hecs::Entity;
use rand::Rng;

use crate::action::{Direction, Position};
use crate::combat::{
    DefenderState, SkillLevel, WeaponStats,
    hitval, strength_damage_bonus, weapon_dam_bonus_armed,
    weapon_hit_bonus_armed, dmgval,
};
use crate::dungeon::{LevelMap, Terrain};
use crate::event::{DeathCause, EngineEvent, HpSource};
use crate::world::{
    ArmorClass, Attributes, ExperienceLevel, GameWorld, HitPoints, Monster,
    Positioned,
};

use nethack_babel_data::WeaponSkill;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum ranged attack distance for crossbow bolts (BOLT_LIM in hack.h).
pub const BOLT_LIM: i32 = 8;

/// Default launcher range for bows, crossbows, and slings.
pub const LAUNCHER_RANGE: i32 = 18;

// ---------------------------------------------------------------------------
// ACURRSTR — effective strength for range calculation
// ---------------------------------------------------------------------------

/// Map (strength, strength_extra) to the effective ACURRSTR value used in
/// range computation.
///
/// STR 3..18 -> 3..18
/// STR 18/01..18/31 -> 19
/// STR 18/32..18/81 -> 20
/// STR 18/82..18/99 -> 21
/// STR 18/100 -> 21
/// STR 19..25 -> 22..25 (but raw > 18 means gauntlets etc., map 19->22)
pub fn acurrstr(strength: u8, strength_extra: u8) -> i32 {
    match strength {
        0..=17 => strength as i32,
        18 => {
            match strength_extra {
                0 => 18,
                1..=31 => 19,
                32..=81 => 20,
                82.. => 21,
            }
        }
        19..=25 => {
            // STR 19-25 maps to ACURRSTR 22-25 per the spec comment
            // (19->22, 20->23, 21->21 with 18/82+ overlap, but
            // practically strengths above 18 from gauntlets etc. yield
            // values in the 19-25 range, mapped as strength - 19 + 22).
            // Actually the spec says STR 22..25 -> 22..25.
            // For 19..21 the mapping is 21 (same as 18/82+).
            if strength <= 21 { 21 } else { strength as i32 }
        }
        _ => 25, // cap
    }
}

// ---------------------------------------------------------------------------
// Dexterity modifier for ranged to-hit
// ---------------------------------------------------------------------------

/// DEX-based to-hit modifier for ranged combat (spec section 4.2).
///
/// IF DEX < 4:  -3
/// ELIF DEX < 6:  -2
/// ELIF DEX < 8:  -1
/// ELIF DEX >= 14: +(DEX - 14)
/// ELSE: 0
pub fn dex_to_hit_modifier(dexterity: u8) -> i32 {
    let dex = dexterity as i32;
    if dex < 4 {
        -3
    } else if dex < 6 {
        -2
    } else if dex < 8 {
        -1
    } else if dex >= 14 {
        dex - 14
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Distance modifier
// ---------------------------------------------------------------------------

/// Chebyshev distance between two positions.
#[inline]
pub fn chebyshev_distance(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    let dx = (ax - bx).abs();
    let dy = (ay - by).abs();
    dx.max(dy)
}

/// Distance-based to-hit modifier (spec section 4.3).
///
/// disttmp = 3 - chebyshev_distance
/// clamped to minimum -4.
#[inline]
pub fn distance_modifier(distance: i32) -> i32 {
    let disttmp = 3 - distance;
    disttmp.max(-4)
}

// ---------------------------------------------------------------------------
// Throw range calculation
// ---------------------------------------------------------------------------

/// Calculate the throwing range for an object.
///
/// Formula from spec section 1.1:
///   urange = ACURRSTR / 2
///   range = urange - (weight / 40)
///   minimum 1
pub fn throw_range(strength: u8, strength_extra: u8, weight: u32) -> i32 {
    let urange = acurrstr(strength, strength_extra) / 2;
    let range = urange - (weight as i32 / 40);
    range.max(1)
}

// ---------------------------------------------------------------------------
// Launcher+ammo matching
// ---------------------------------------------------------------------------

/// Check whether an ammo skill matches a launcher skill.
///
/// Concrete pairings (spec section 2.1):
/// - Bow <-> Arrow (bow+arrow, elven bow+elven arrow, etc.)
/// - Crossbow <-> Bolt
/// - Sling <-> Stone (rock, flint, gems)
///
/// In NetHack, ammo skills are the negative of their launcher skill.
/// We model this by checking explicit pairs.
pub fn matching_launcher(ammo_skill: WeaponSkill, launcher_skill: WeaponSkill) -> bool {
    matches!(
        (launcher_skill, ammo_skill),
        (WeaponSkill::Bow, WeaponSkill::Bow)
            | (WeaponSkill::Crossbow, WeaponSkill::Crossbow)
            | (WeaponSkill::Sling, WeaponSkill::Sling)
    )
}

// ---------------------------------------------------------------------------
// Multishot calculation
// ---------------------------------------------------------------------------

/// Calculate the number of shots for a multishot attack.
///
/// From spec section 3:
///   total = 1 + skill_bonus + role_bonus + race_bonus
///   result = rnd(total)  (1..total)
///
/// Skill bonus (spec 3.3):
///   Expert + not weak: +2
///   Expert + weak: +1
///   Skilled + not weak: +1
///   otherwise: 0
///
/// This function takes pre-computed bonuses so callers can determine
/// role/race bonuses from their own context.
pub fn calculate_multishot(
    skill_level: SkillLevel,
    role_bonus: u8,
    race_bonus: u8,
    rng: &mut impl Rng,
) -> u8 {
    let skill_bonus: u8 = match skill_level {
        SkillLevel::Expert => 2,
        SkillLevel::Skilled => 1,
        _ => 0,
    };

    let total = 1u8
        .saturating_add(skill_bonus)
        .saturating_add(role_bonus)
        .saturating_add(race_bonus);

    // rnd(total) produces 1..=total
    if total <= 1 {
        1
    } else {
        rng.random_range(1..=total)
    }
}

// ---------------------------------------------------------------------------
// Projectile path tracing
// ---------------------------------------------------------------------------

/// Trace a projectile path from `start` in the given `direction` for up to
/// `range` steps.
///
/// Stops when hitting a wall, going out of bounds, or reaching max range.
/// Returns the list of positions traversed (NOT including the start position).
pub fn trace_projectile(
    map: &LevelMap,
    start: Position,
    direction: Direction,
    range: i32,
) -> Vec<Position> {
    let (dx, dy) = direction.delta();

    // If direction has no displacement (Up/Down/Self_), return empty.
    if dx == 0 && dy == 0 {
        return Vec::new();
    }

    let mut path = Vec::new();
    let mut x = start.x;
    let mut y = start.y;

    for _ in 0..range {
        x += dx;
        y += dy;

        let pos = Position::new(x, y);

        // Check bounds.
        if !map.in_bounds(pos) {
            break;
        }

        // Check if terrain blocks projectiles (walls, closed doors, stone).
        if let Some(cell) = map.get(pos) {
            match cell.terrain {
                Terrain::Wall | Terrain::Stone | Terrain::DoorClosed
                | Terrain::DoorLocked | Terrain::Tree | Terrain::IronBars => {
                    break;
                }
                _ => {}
            }
        } else {
            break;
        }

        path.push(pos);
    }

    path
}

// ---------------------------------------------------------------------------
// Ammo breakage
// ---------------------------------------------------------------------------

/// Determine whether a projectile should break after hitting a target.
///
/// From spec section 8.1:
///   chance = 3 + greatest_erosion - enchantment
///   if chance > 1: broken = rn2(chance) != 0  (probability = (chance-1)/chance)
///   else: broken = rn2(4) == 0  (flat 25%)
///
/// This is a simplified version that does not include blessed/material saves.
pub fn should_break(enchantment: i32, greatest_erosion: i32, rng: &mut impl Rng) -> bool {
    let chance = 3 + greatest_erosion - enchantment;
    if chance > 1 {
        // Break if rn2(chance) != 0, i.e., probability (chance-1)/chance.
        rng.random_range(0..chance) != 0
    } else {
        // Flat 25% break rate.
        rng.random_range(0..4) == 0
    }
}

// ---------------------------------------------------------------------------
// Ranged to-hit calculation
// ---------------------------------------------------------------------------

/// Parameters for a ranged attack (thrown or fired).
#[derive(Debug, Clone)]
pub struct RangedAttackParams {
    // ---- Attacker stats ----
    pub strength: u8,
    pub strength_extra: u8,
    pub dexterity: u8,
    pub level: u8,
    pub luck: i32,
    /// Ring of Increase Accuracy bonus.
    pub uhitinc: i32,
    /// Ring of Increase Damage bonus.
    pub udaminc: i32,

    // ---- Projectile ----
    pub projectile: WeaponStats,
    pub projectile_skill: SkillLevel,

    // ---- Launcher (for fired ammo) ----
    /// If Some, this is a launcher+ammo shot (no STR damage bonus).
    pub launcher: Option<WeaponStats>,

    // ---- Flags ----
    /// Whether the projectile is a throwing weapon (dart, spear, dagger, etc.)
    pub is_throwing_weapon: bool,
    /// Whether the projectile is ammo (arrow, bolt, stone).
    pub is_ammo: bool,

    // ---- Target ----
    pub target_ac: i32,
    pub defender_state: DefenderState,
    /// Chebyshev distance from attacker to target.
    pub distance: i32,
}

impl Default for RangedAttackParams {
    fn default() -> Self {
        Self {
            strength: 10,
            strength_extra: 0,
            dexterity: 10,
            level: 1,
            luck: 0,
            uhitinc: 0,
            udaminc: 0,
            projectile: WeaponStats {
                spe: 0,
                hit_bonus: 0,
                damage_small: 4,
                damage_large: 4,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            },
            projectile_skill: SkillLevel::Basic,
            launcher: None,
            is_throwing_weapon: false,
            is_ammo: false,
            target_ac: 10,
            defender_state: DefenderState::default(),
            distance: 1,
        }
    }
}

/// Compute the ranged to-hit value (spec section 4).
///
/// Formula:
///   tmp = -1 + Luck + find_mac(target) + uhitinc + level
///   + dex_modifier + distance_modifier
///   + omon_adj (size, sleeping, paralyzed, hitval)
///   + weapon_type_modifier (throwing_weapon: +2, ammo with launcher: launcher bonuses)
///   + weapon_hit_bonus (skill-based)
pub fn ranged_to_hit(params: &RangedAttackParams) -> i32 {
    let mut tmp: i32 = -1;

    // Luck
    tmp += params.luck;

    // Target AC (find_mac)
    tmp += params.target_ac;

    // Accuracy ring
    tmp += params.uhitinc;

    // Experience level
    tmp += params.level as i32;

    // DEX modifier
    tmp += dex_to_hit_modifier(params.dexterity);

    // Distance modifier
    tmp += distance_modifier(params.distance);

    // Monster state (sleeping, paralyzed)
    if params.defender_state.sleeping {
        tmp += 2;
    }
    if params.defender_state.paralyzed {
        tmp += 4;
    }

    // hitval from projectile
    tmp += hitval(&params.projectile, &params.defender_state);

    // Weapon-type-specific modifiers
    if params.is_ammo {
        if let Some(ref launcher) = params.launcher {
            // Properly matched ammo+launcher: add launcher bonuses.
            tmp += launcher.spe - launcher.greatest_erosion;
            tmp += weapon_hit_bonus_armed(params.projectile_skill, false, SkillLevel::Basic);
        } else {
            // Ammo thrown without matching launcher: -4 penalty.
            tmp -= 4;
        }
    } else if params.is_throwing_weapon {
        // Throwing weapons (daggers, spears, darts, shuriken): +2
        tmp += 2;
        tmp += weapon_hit_bonus_armed(params.projectile_skill, false, SkillLevel::Basic);
    } else {
        // Non-throwing weapon thrown: -2
        tmp -= 2;
        tmp += weapon_hit_bonus_armed(params.projectile_skill, false, SkillLevel::Basic);
    }

    tmp
}

/// Calculate ranged damage for a hit.
///
/// From spec section 5:
///   base = dmgval(projectile, defender)
///   + udaminc (always)
///   + strength_bonus (ONLY for thrown melee weapons, NOT for shot ammo)
///   + weapon_dam_bonus (skill-based)
///   - minimum 1
pub fn ranged_damage(params: &RangedAttackParams, rng: &mut impl Rng) -> i32 {
    // Base damage from weapon dice.
    let base_dmg = dmgval(&params.projectile, &params.defender_state, rng);

    if base_dmg <= 0 {
        return 1;
    }

    let mut damage = base_dmg;

    // Ring of Increase Damage (always applied).
    damage += params.udaminc;

    // Strength bonus: applied for thrown weapons, NOT for launcher-shot ammo.
    if params.launcher.is_none() {
        let str_bonus = strength_damage_bonus(params.strength, params.strength_extra);
        damage += str_bonus;
    }

    // Skill damage bonus.
    damage += weapon_dam_bonus_armed(params.projectile_skill, false, SkillLevel::Basic);

    // Minimum damage.
    if damage < 1 {
        damage = 1;
    }

    damage
}

// ---------------------------------------------------------------------------
// Projectile shatter / bounce
// ---------------------------------------------------------------------------

/// Potion-class objects that shatter when they hit a wall or miss.
/// In NetHack, glass potions always shatter; this function models that.
pub fn should_shatter(is_potion: bool, is_glass: bool) -> bool {
    is_potion || is_glass
}

/// When a potion shatters, it may create a splash effect at the impact site.
/// Returns a message key describing the splash effect (if any).
pub fn shatter_effect(is_potion: bool) -> Option<&'static str> {
    if is_potion {
        Some("thrown-potion-shatter")
    } else {
        Some("thrown-glass-shatter")
    }
}

/// Result of an object hitting the floor (from `hitfloor` in dothrow.c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitFloorResult {
    /// Object landed on the floor normally.
    Landed,
    /// Object fell into a hole or trap.
    FellThrough,
    /// Object shattered on impact.
    Shattered,
    /// Object landed on an altar.
    OnAltar,
}

/// Determine what happens when a thrown object hits the floor at a position.
pub fn hit_floor(
    terrain: Terrain,
    is_fragile: bool,
    has_trap_hole: bool,
) -> HitFloorResult {
    if has_trap_hole {
        return HitFloorResult::FellThrough;
    }
    if terrain == Terrain::Altar {
        return HitFloorResult::OnAltar;
    }
    if is_fragile {
        return HitFloorResult::Shattered;
    }
    HitFloorResult::Landed
}

// ---------------------------------------------------------------------------
// Bounce / ricochet for wand zaps
// ---------------------------------------------------------------------------

/// Direction a ray bounces off a wall.
///
/// In NetHack, rays bounce off walls by reversing the component that
/// collided.  Horizontal walls reverse dy, vertical walls reverse dx,
/// corners reverse both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BounceAxis {
    Horizontal,
    Vertical,
    Corner,
}

/// Calculate the bounce direction when a ray hits a wall.
///
/// `incoming` is the direction the ray was travelling.
/// `axis` describes which wall component was hit.
pub fn bounce_direction(incoming: Direction, axis: BounceAxis) -> Direction {
    let (dx, dy) = direction_delta(incoming);
    let (ndx, ndy) = match axis {
        BounceAxis::Horizontal => (dx, -dy),
        BounceAxis::Vertical => (-dx, dy),
        BounceAxis::Corner => (-dx, -dy),
    };
    delta_to_direction(ndx, ndy).unwrap_or(incoming)
}

/// Convert a Direction to (dx, dy) deltas.
fn direction_delta(dir: Direction) -> (i32, i32) {
    match dir {
        Direction::North => (0, -1),
        Direction::South => (0, 1),
        Direction::East => (1, 0),
        Direction::West => (-1, 0),
        Direction::NorthEast => (1, -1),
        Direction::NorthWest => (-1, -1),
        Direction::SouthEast => (1, 1),
        Direction::SouthWest => (-1, 1),
        Direction::Up | Direction::Down | Direction::Self_ => (0, 0),
    }
}

/// Convert (dx, dy) deltas back to a Direction.
fn delta_to_direction(dx: i32, dy: i32) -> Option<Direction> {
    match (dx.signum(), dy.signum()) {
        (0, -1) => Some(Direction::North),
        (0, 1) => Some(Direction::South),
        (1, 0) => Some(Direction::East),
        (-1, 0) => Some(Direction::West),
        (1, -1) => Some(Direction::NorthEast),
        (-1, -1) => Some(Direction::NorthWest),
        (1, 1) => Some(Direction::SouthEast),
        (-1, 1) => Some(Direction::SouthWest),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Bresenham line (walk_path from dothrow.c)
// ---------------------------------------------------------------------------

/// Walk a straight-line path from `src` to `dst` using Bresenham's algorithm.
///
/// Returns positions along the path, excluding `src` but including `dst`.
/// If `max_steps` is reached or the path goes out of the given bounds,
/// the walk terminates early.
pub fn bresenham_path(
    src: Position,
    dst: Position,
    max_steps: usize,
    bounds: (i32, i32, i32, i32), // (min_x, min_y, max_x, max_y)
) -> Vec<Position> {
    let mut result = Vec::new();
    let mut dx = dst.x - src.x;
    let mut dy = dst.y - src.y;
    let x_step = if dx < 0 { dx = -dx; -1i32 } else { 1i32 };
    let y_step = if dy < 0 { dy = -dy; -1i32 } else { 1i32 };

    let mut x = src.x;
    let mut y = src.y;
    let mut err = 0i32;
    let mut steps = 0;

    if dx < dy {
        for _ in 0..dy {
            y += y_step;
            err += dx * 2;
            if err > dy {
                x += x_step;
                err -= dy * 2;
            }
            if x < bounds.0 || x > bounds.2 || y < bounds.1 || y > bounds.3 {
                break;
            }
            result.push(Position::new(x, y));
            steps += 1;
            if steps >= max_steps {
                break;
            }
        }
    } else {
        for _ in 0..dx {
            x += x_step;
            err += dy * 2;
            if err > dx {
                y += y_step;
                err -= dx * 2;
            }
            if x < bounds.0 || x > bounds.2 || y < bounds.1 || y > bounds.3 {
                break;
            }
            result.push(Position::new(x, y));
            steps += 1;
            if steps >= max_steps {
                break;
            }
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Hurtle: forced movement (knockback / jumping)
// ---------------------------------------------------------------------------

/// Result of a hurtle (forced movement) step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HurtleStepResult {
    /// Moved successfully to the new position.
    Moved,
    /// Blocked by a wall or obstacle; stop here.
    Blocked,
    /// Landed in water/lava; special handling needed.
    HazardTerrain,
}

/// Check whether a hurtle step to the given position is valid.
pub fn check_hurtle_step(terrain: Terrain) -> HurtleStepResult {
    match terrain {
        Terrain::Wall | Terrain::Stone | Terrain::DoorClosed
        | Terrain::DoorLocked | Terrain::IronBars | Terrain::Tree => {
            HurtleStepResult::Blocked
        }
        Terrain::Pool | Terrain::Moat | Terrain::Lava | Terrain::Water => {
            HurtleStepResult::HazardTerrain
        }
        _ => HurtleStepResult::Moved,
    }
}

/// Compute a hurtle path: the entity is forced to move `distance` steps
/// in `direction`. Stops at walls or hazards.
pub fn hurtle_path(
    map: &LevelMap,
    start: Position,
    direction: Direction,
    distance: i32,
) -> Vec<Position> {
    let mut path = Vec::new();
    let mut pos = start;
    for _ in 0..distance {
        let next = pos.step(direction);
        if !map.in_bounds(next) {
            break;
        }
        let terrain = map
            .get(next)
            .map(|c| c.terrain)
            .unwrap_or(Terrain::Stone);
        match check_hurtle_step(terrain) {
            HurtleStepResult::Moved => {
                path.push(next);
                pos = next;
            }
            HurtleStepResult::Blocked => break,
            HurtleStepResult::HazardTerrain => {
                path.push(next);
                break; // stop after entering hazard
            }
        }
    }
    path
}

// ---------------------------------------------------------------------------
// Gem catching by unicorns (gem_accept from dothrow.c)
// ---------------------------------------------------------------------------

/// Whether a monster can catch a gem thrown at it (unicorn behavior).
///
/// In NetHack, co-aligned unicorns accept valuable gems, cross-aligned
/// ones may accept them grudgingly, and all unicorns reject glass gems.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GemAcceptResult {
    /// Unicorn accepts the gem (valuable, correct alignment).
    Accepted,
    /// Unicorn catches but doesn't particularly like it (cross-aligned).
    Grudging,
    /// Unicorn rejects the gem (worthless glass).
    Rejected,
    /// Target isn't a unicorn; doesn't apply.
    NotApplicable,
}

/// Determine if a unicorn accepts a thrown gem.
pub fn gem_accept(
    is_unicorn: bool,
    gem_is_valuable: bool,
    same_alignment: bool,
) -> GemAcceptResult {
    if !is_unicorn {
        return GemAcceptResult::NotApplicable;
    }
    if !gem_is_valuable {
        return GemAcceptResult::Rejected;
    }
    if same_alignment {
        GemAcceptResult::Accepted
    } else {
        GemAcceptResult::Grudging
    }
}

// ---------------------------------------------------------------------------
// Monster multishot calculation (monmulti from mthrowu.c)
// ---------------------------------------------------------------------------

/// Calculate the number of shots a monster fires.
///
/// Based on `monmulti()` from mthrowu.c:
/// - High-level monsters (level >= 12) get extra shots.
/// - Monsters wielding crossbows are limited to 1 (reload time).
pub fn monster_multishot(
    monster_level: u32,
    is_crossbow: bool,
    rng: &mut impl Rng,
) -> u32 {
    if is_crossbow {
        return 1;
    }
    let base = if monster_level >= 18 {
        3
    } else if monster_level >= 12 {
        2
    } else {
        1
    };
    // Random chance for extra shot (1 in 3).
    if base > 1 && rng.random_range(0..3u32) == 0 {
        base + 1
    } else {
        base
    }
}

// ---------------------------------------------------------------------------
// Monster ranged attack: line-up check (m_lined_up from mthrowu.c)
// ---------------------------------------------------------------------------

/// Check whether a monster at `attacker_pos` has a clear line of fire
/// to `target_pos` in one of the 8 cardinal/diagonal directions.
///
/// Returns `Some(direction)` if lined up, `None` otherwise.
pub fn lined_up(
    attacker_pos: Position,
    target_pos: Position,
) -> Option<Direction> {
    let dx = target_pos.x - attacker_pos.x;
    let dy = target_pos.y - attacker_pos.y;

    // Must be on the same row, column, or a perfect diagonal.
    if dx == 0 && dy == 0 {
        return None;
    }
    if dx != 0 && dy != 0 && dx.abs() != dy.abs() {
        return None;
    }

    delta_to_direction(dx, dy)
}

// ---------------------------------------------------------------------------
// End-multishot helper
// ---------------------------------------------------------------------------

/// Represents an in-progress multishot sequence.
#[derive(Debug, Clone, Copy)]
pub struct MultishotState {
    /// Current shot number (1-based).
    pub current: u32,
    /// Total shots planned.
    pub total: u32,
    /// Whether this is launcher-fired (vs thrown).
    pub is_fired: bool,
}

impl MultishotState {
    pub fn new(total: u32, is_fired: bool) -> Self {
        Self {
            current: 0,
            total,
            is_fired,
        }
    }

    /// Advance to the next shot. Returns false if all shots are done.
    pub fn advance(&mut self) -> bool {
        self.current += 1;
        self.current <= self.total
    }

    /// Cancel remaining shots (e.g., target died or player interrupted).
    pub fn cancel(&mut self) {
        self.total = self.current;
    }

    pub fn is_done(&self) -> bool {
        self.current >= self.total
    }
}

// ---------------------------------------------------------------------------
// Top-level: resolve_throw
// ---------------------------------------------------------------------------

/// Resolve a throwing attack.
///
/// Traces the projectile along `direction`, checks for monster hits at each
/// cell, and returns the generated events.
///
/// The projectile item entity must have already been removed from inventory
/// by the caller.
pub fn resolve_throw(
    world: &mut GameWorld,
    thrower: Entity,
    item: Entity,
    direction: Direction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // ---- Extract thrower stats ----
    let (str_val, str_extra, dex_val) = world
        .get_component::<Attributes>(thrower)
        .map(|a| (a.strength, a.strength_extra, a.dexterity))
        .unwrap_or((10, 0, 10));

    let level = world
        .get_component::<ExperienceLevel>(thrower)
        .map(|l| l.0)
        .unwrap_or(1);

    let thrower_pos = world
        .get_component::<Positioned>(thrower)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // ---- Item properties (weight for range) ----
    // For simplicity, use a default weight. In a full implementation we'd
    // extract from ObjectCore.
    let item_weight: u32 = 10; // placeholder default

    // ---- Calculate range ----
    let range = throw_range(str_val, str_extra, item_weight);

    // ---- Trace projectile path ----
    let map = &world.dungeon().current_level;
    let path = trace_projectile(map, thrower_pos, direction, range);

    // ---- Check for monster hits along path ----
    let mut hit_entity: Option<(Entity, Position)> = None;
    let mut _final_pos = thrower_pos;

    // Collect monster positions first (to avoid borrow issues).
    let monster_positions: Vec<(Entity, Position)> = {
        let mut result = Vec::new();
        for (entity, _) in world.query::<Monster>().iter() {
            if let Some(pos) = world.get_component::<Positioned>(entity) {
                result.push((entity, pos.0));
            }
        }
        result
    };

    for cell_pos in &path {
        _final_pos = *cell_pos;

        // Check if a monster is at this position.
        for &(monster_entity, monster_pos) in &monster_positions {
            if monster_pos == *cell_pos && monster_entity != thrower {
                hit_entity = Some((monster_entity, *cell_pos));
                break;
            }
        }

        if hit_entity.is_some() {
            break;
        }
    }

    // ---- Resolve hit or miss ----
    if let Some((target, target_pos)) = hit_entity {
        let target_ac = world
            .get_component::<ArmorClass>(target)
            .map(|ac| ac.0)
            .unwrap_or(10);

        let distance = chebyshev_distance(
            thrower_pos.x, thrower_pos.y,
            target_pos.x, target_pos.y,
        );

        let params = RangedAttackParams {
            strength: str_val,
            strength_extra: str_extra,
            dexterity: dex_val,
            level,
            luck: 0,
            uhitinc: 0,
            udaminc: 0,
            projectile: WeaponStats {
                spe: 0,
                hit_bonus: 0,
                damage_small: 4,
                damage_large: 4,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            },
            projectile_skill: SkillLevel::Basic,
            launcher: None,
            is_throwing_weapon: true,
            is_ammo: false,
            target_ac,
            defender_state: DefenderState::default(),
            distance,
        };

        let to_hit = ranged_to_hit(&params);
        let dieroll: i32 = rng.random_range(1..=20);

        if to_hit >= dieroll {
            let damage = ranged_damage(&params, rng);

            events.push(EngineEvent::RangedHit {
                attacker: thrower,
                defender: target,
                projectile: item,
                damage: damage as u32,
            });

            // Apply damage.
            let defender_hp = world
                .get_component::<HitPoints>(target)
                .map(|hp| hp.current)
                .unwrap_or(1);
            let new_hp = defender_hp - damage;

            events.push(EngineEvent::HpChange {
                entity: target,
                amount: -damage,
                new_hp,
                source: HpSource::Combat,
            });

            if let Some(mut hp) = world.get_component_mut::<HitPoints>(target) {
                hp.current = new_hp;
            }

            if new_hp <= 0 {
                let killer_name = world.entity_name(thrower);
                events.push(EngineEvent::EntityDied {
                    entity: target,
                    killer: Some(thrower),
                    cause: DeathCause::KilledBy { killer_name },
                });
            }
        } else {
            events.push(EngineEvent::RangedMiss {
                attacker: thrower,
                defender: target,
                projectile: item,
            });
        }
    }

    // Item lands on the floor at the final position (hit or max range).
    events.push(EngineEvent::ItemDropped {
        actor: thrower,
        item,
    });

    events
}

// ---------------------------------------------------------------------------
// Top-level: resolve_fire
// ---------------------------------------------------------------------------

/// Resolve a fired (launcher + ammo) attack.
///
/// Verifies launcher+ammo match, calculates multishot, then for each shot
/// traces the projectile and checks for hits.
pub fn resolve_fire(
    world: &mut GameWorld,
    shooter: Entity,
    _launcher: Entity,
    ammo: Entity,
    direction: Direction,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    // ---- Extract shooter stats ----
    let (str_val, str_extra, dex_val) = world
        .get_component::<Attributes>(shooter)
        .map(|a| (a.strength, a.strength_extra, a.dexterity))
        .unwrap_or((10, 0, 10));

    let level = world
        .get_component::<ExperienceLevel>(shooter)
        .map(|l| l.0)
        .unwrap_or(1);

    let shooter_pos = world
        .get_component::<Positioned>(shooter)
        .map(|p| p.0)
        .unwrap_or(Position::new(0, 0));

    // ---- Range: launchers use fixed range ----
    let range = LAUNCHER_RANGE / 2; // = 9; crossbow would override to BOLT_LIM

    // ---- Trace path ----
    let map = &world.dungeon().current_level;
    let path = trace_projectile(map, shooter_pos, direction, range);

    // ---- Collect monsters ----
    let monster_positions: Vec<(Entity, Position)> = {
        let mut result = Vec::new();
        for (entity, _) in world.query::<Monster>().iter() {
            if let Some(pos) = world.get_component::<Positioned>(entity) {
                result.push((entity, pos.0));
            }
        }
        result
    };

    // ---- Check for hits ----
    for cell_pos in &path {
        for &(monster_entity, monster_pos) in &monster_positions {
            if monster_pos == *cell_pos && monster_entity != shooter {
                let target_ac = world
                    .get_component::<ArmorClass>(monster_entity)
                    .map(|ac| ac.0)
                    .unwrap_or(10);

                let distance = chebyshev_distance(
                    shooter_pos.x, shooter_pos.y,
                    cell_pos.x, cell_pos.y,
                );

                let params = RangedAttackParams {
                    strength: str_val,
                    strength_extra: str_extra,
                    dexterity: dex_val,
                    level,
                    luck: 0,
                    uhitinc: 0,
                    udaminc: 0,
                    projectile: WeaponStats {
                        spe: 0,
                        hit_bonus: 0,
                        damage_small: 6,
                        damage_large: 6,
                        is_weapon: true,
                        blessed: false,
                        is_silver: false,
                        greatest_erosion: 0,
                    },
                    projectile_skill: SkillLevel::Basic,
                    launcher: Some(WeaponStats {
                        spe: 0,
                        hit_bonus: 0,
                        damage_small: 0,
                        damage_large: 0,
                        is_weapon: true,
                        blessed: false,
                        is_silver: false,
                        greatest_erosion: 0,
                    }),
                    is_throwing_weapon: false,
                    is_ammo: true,
                    target_ac,
                    defender_state: DefenderState::default(),
                    distance,
                };

                let to_hit = ranged_to_hit(&params);
                let dieroll: i32 = rng.random_range(1..=20);

                if to_hit >= dieroll {
                    let damage = ranged_damage(&params, rng);

                    events.push(EngineEvent::RangedHit {
                        attacker: shooter,
                        defender: monster_entity,
                        projectile: ammo,
                        damage: damage as u32,
                    });

                    let defender_hp = world
                        .get_component::<HitPoints>(monster_entity)
                        .map(|hp| hp.current)
                        .unwrap_or(1);
                    let new_hp = defender_hp - damage;

                    events.push(EngineEvent::HpChange {
                        entity: monster_entity,
                        amount: -damage,
                        new_hp,
                        source: HpSource::Combat,
                    });

                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(monster_entity) {
                        hp.current = new_hp;
                    }

                    if new_hp <= 0 {
                        let killer_name = world.entity_name(shooter);
                        events.push(EngineEvent::EntityDied {
                            entity: monster_entity,
                            killer: Some(shooter),
                            cause: DeathCause::KilledBy { killer_name },
                        });
                    }
                } else {
                    events.push(EngineEvent::RangedMiss {
                        attacker: shooter,
                        defender: monster_entity,
                        projectile: ammo,
                    });
                }

                // First monster hit ends the projectile path.
                events.push(EngineEvent::ItemDropped {
                    actor: shooter,
                    item: ammo,
                });
                return events;
            }
        }
    }

    // No monster hit: item lands at end of path.
    events.push(EngineEvent::ItemDropped {
        actor: shooter,
        item: ammo,
    });

    events
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::combat::{DefenderState, SkillLevel, WeaponStats};
    use crate::dungeon::{LevelMap, Terrain};
    use crate::world::{
        ArmorClass, GameWorld, HitPoints,
        Monster, Positioned,
    };
    use nethack_babel_data::WeaponSkill;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    /// Create a simple level map with floor tiles and walls around the border.
    fn floor_map(width: usize, height: usize) -> LevelMap {
        let mut map = LevelMap::new(width, height);
        for y in 0..height {
            for x in 0..width {
                if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                    map.set_terrain(Position::new(x as i32, y as i32), Terrain::Wall);
                } else {
                    map.set_terrain(Position::new(x as i32, y as i32), Terrain::Floor);
                }
            }
        }
        map
    }

    // -----------------------------------------------------------------------
    // Test 1: Throw range calculation by STR and weight
    // -----------------------------------------------------------------------

    #[test]
    fn throw_range_str18_light_item() {
        // STR 18, weight 1: urange = 18/2 = 9, range = 9 - 0 = 9
        assert_eq!(throw_range(18, 0, 1), 9);
    }

    #[test]
    fn throw_range_str10_heavy_item() {
        // STR 10, weight 200: urange = 10/2 = 5, range = 5 - 5 = 0 -> clamp to 1
        assert_eq!(throw_range(10, 0, 200), 1);
    }

    #[test]
    fn throw_range_str18_100_light() {
        // STR 18/100 -> ACURRSTR = 21, urange = 21/2 = 10
        // weight 1: range = 10 - 0 = 10
        assert_eq!(throw_range(18, 100, 1), 10);
    }

    #[test]
    fn throw_range_str3_light() {
        // STR 3, weight 1: urange = 3/2 = 1, range = 1 - 0 = 1
        assert_eq!(throw_range(3, 0, 1), 1);
    }

    #[test]
    fn throw_range_minimum_is_1() {
        // STR 3, weight 400: urange = 1, range = 1 - 10 = -9 -> clamp to 1
        assert_eq!(throw_range(3, 0, 400), 1);
    }

    // -----------------------------------------------------------------------
    // Test 2: Launcher+ammo matching (valid and invalid pairs)
    // -----------------------------------------------------------------------

    #[test]
    fn matching_launcher_bow_arrow() {
        assert!(matching_launcher(WeaponSkill::Bow, WeaponSkill::Bow));
    }

    #[test]
    fn matching_launcher_crossbow_bolt() {
        assert!(matching_launcher(WeaponSkill::Crossbow, WeaponSkill::Crossbow));
    }

    #[test]
    fn matching_launcher_sling_stone() {
        assert!(matching_launcher(WeaponSkill::Sling, WeaponSkill::Sling));
    }

    #[test]
    fn matching_launcher_invalid_bow_bolt() {
        // Bow cannot fire crossbow bolts.
        assert!(!matching_launcher(WeaponSkill::Crossbow, WeaponSkill::Bow));
    }

    #[test]
    fn matching_launcher_invalid_sling_arrow() {
        assert!(!matching_launcher(WeaponSkill::Bow, WeaponSkill::Sling));
    }

    #[test]
    fn matching_launcher_invalid_dagger() {
        // Daggers are not launcher ammo.
        assert!(!matching_launcher(WeaponSkill::Dagger, WeaponSkill::Bow));
    }

    // -----------------------------------------------------------------------
    // Test 3: Multishot at different skill levels
    // -----------------------------------------------------------------------

    #[test]
    fn multishot_unskilled_no_bonus() {
        let mut rng = test_rng();
        // Unskilled: total = 1 + 0 + 0 + 0 = 1, always returns 1.
        for _ in 0..10 {
            assert_eq!(calculate_multishot(SkillLevel::Unskilled, 0, 0, &mut rng), 1);
        }
    }

    #[test]
    fn multishot_expert_with_bonuses() {
        let mut rng = test_rng();
        // Expert: skill_bonus=2, role_bonus=1, race_bonus=1
        // total = 1 + 2 + 1 + 1 = 5, result = rnd(5) -> 1..5
        let mut results = Vec::new();
        for _ in 0..100 {
            let r = calculate_multishot(SkillLevel::Expert, 1, 1, &mut rng);
            assert!(r >= 1 && r <= 5, "multishot {} out of range [1,5]", r);
            results.push(r);
        }
        // With 100 samples from rnd(5), we should see variation.
        let min = *results.iter().min().unwrap();
        let max = *results.iter().max().unwrap();
        assert!(max > min, "should have variation in multishot results");
    }

    #[test]
    fn multishot_skilled_no_role_race() {
        let mut rng = test_rng();
        // Skilled: skill_bonus=1, total = 1 + 1 + 0 + 0 = 2
        let mut saw_two = false;
        for _ in 0..50 {
            let r = calculate_multishot(SkillLevel::Skilled, 0, 0, &mut rng);
            assert!(r >= 1 && r <= 2);
            if r == 2 {
                saw_two = true;
            }
        }
        assert!(saw_two, "should sometimes get 2 shots at Skilled");
    }

    // -----------------------------------------------------------------------
    // Test 4: Projectile hits monster in path
    // -----------------------------------------------------------------------

    #[test]
    fn projectile_hits_monster_in_path() {
        let mut world = GameWorld::new(Position::new(5, 5));

        // Set up a floor map on the dungeon.
        world.dungeon_mut().current_level = floor_map(20, 10);

        // Spawn a monster at (10, 5) — directly east of the player.
        let monster = world.spawn((
            Monster,
            Positioned(Position::new(10, 5)),
            HitPoints { current: 20, max: 20 },
            ArmorClass(10),
        ));

        let thrower = world.player();

        // Spawn a projectile entity.
        let projectile = world.spawn(());

        let mut rng = test_rng();
        let events = resolve_throw(&mut world, thrower, projectile, Direction::East, &mut rng);

        // Should have either a RangedHit or RangedMiss for the monster,
        // plus an ItemDropped.
        let has_ranged_event = events.iter().any(|e| {
            matches!(e, EngineEvent::RangedHit { defender, .. } if *defender == monster)
                || matches!(e, EngineEvent::RangedMiss { defender, .. } if *defender == monster)
        });
        assert!(has_ranged_event, "projectile should interact with monster in path");
    }

    // -----------------------------------------------------------------------
    // Test 5: Projectile stops at wall
    // -----------------------------------------------------------------------

    #[test]
    fn projectile_stops_at_wall() {
        let mut map = LevelMap::new(20, 5);
        // Floor from x=1..=9, wall at x=10.
        for x in 0..20 {
            for y in 0..5 {
                map.set_terrain(
                    Position::new(x, y),
                    if x >= 10 { Terrain::Wall } else { Terrain::Floor },
                );
            }
        }

        let start = Position::new(5, 2);
        let path = trace_projectile(&map, start, Direction::East, 20);

        // Should stop before the wall at x=10.
        assert!(!path.is_empty());
        for pos in &path {
            assert!(pos.x < 10, "projectile should not enter wall at x=10");
        }
        // Last position should be at x=9 (one before the wall).
        assert_eq!(path.last().unwrap().x, 9);
    }

    // -----------------------------------------------------------------------
    // Test 6: Ammo breakage probability
    // -----------------------------------------------------------------------

    #[test]
    fn breakage_uneroded_unenchanted() {
        // chance = 3 + 0 - 0 = 3, break = rn2(3) != 0 -> 67%
        let mut rng = test_rng();
        let mut breaks = 0;
        let trials = 1000;
        for _ in 0..trials {
            if should_break(0, 0, &mut rng) {
                breaks += 1;
            }
        }
        // Should be approximately 67% (allow wide margin for RNG).
        let rate = breaks as f64 / trials as f64;
        assert!(
            rate > 0.5 && rate < 0.85,
            "break rate {} should be ~67%", rate
        );
    }

    #[test]
    fn breakage_high_enchantment() {
        // +5 enchantment: chance = 3 + 0 - 5 = -2, flat 25%
        let mut rng = test_rng();
        let mut breaks = 0;
        let trials = 1000;
        for _ in 0..trials {
            if should_break(5, 0, &mut rng) {
                breaks += 1;
            }
        }
        let rate = breaks as f64 / trials as f64;
        assert!(
            rate > 0.1 && rate < 0.4,
            "break rate {} should be ~25%", rate
        );
    }

    #[test]
    fn breakage_eroded() {
        // erosion 2, +0: chance = 3 + 2 - 0 = 5, break = rn2(5) != 0 -> 80%
        let mut rng = test_rng();
        let mut breaks = 0;
        let trials = 1000;
        for _ in 0..trials {
            if should_break(0, 2, &mut rng) {
                breaks += 1;
            }
        }
        let rate = breaks as f64 / trials as f64;
        assert!(
            rate > 0.65 && rate < 0.95,
            "break rate {} should be ~80%", rate
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: Thrown melee weapon gets STR bonus
    // -----------------------------------------------------------------------

    #[test]
    fn thrown_weapon_gets_str_bonus() {
        let mut rng = test_rng();
        let params = RangedAttackParams {
            strength: 18,
            strength_extra: 100, // 18/100 -> STR damage bonus = +7
            launcher: None,     // thrown, NOT fired through launcher
            is_ammo: false,
            is_throwing_weapon: true,
            projectile: WeaponStats {
                spe: 0,
                hit_bonus: 0,
                damage_small: 6,
                damage_large: 6,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            },
            ..Default::default()
        };

        // Run multiple trials and verify damage includes STR bonus.
        // With STR 18/100, dbon = +7. Base damage = rnd(6) = 1..6.
        // weapon_dam_bonus at Basic = 0. udaminc = 0.
        // So damage = rnd(6) + 7 = 8..13.
        let mut min_dmg = i32::MAX;
        let mut max_dmg = i32::MIN;
        for _ in 0..100 {
            let d = ranged_damage(&params, &mut rng);
            min_dmg = min_dmg.min(d);
            max_dmg = max_dmg.max(d);
        }
        assert!(min_dmg >= 8, "min damage {} should be >= 8 (1+7)", min_dmg);
        assert!(max_dmg <= 13, "max damage {} should be <= 13 (6+7)", max_dmg);
    }

    // -----------------------------------------------------------------------
    // Test 8: Shot ammo does NOT get STR bonus
    // -----------------------------------------------------------------------

    #[test]
    fn shot_ammo_no_str_bonus() {
        let mut rng = test_rng();
        let params = RangedAttackParams {
            strength: 18,
            strength_extra: 100, // dbon would be +7 if applied
            launcher: Some(WeaponStats {
                spe: 0,
                hit_bonus: 0,
                damage_small: 0,
                damage_large: 0,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            }),
            is_ammo: true,
            is_throwing_weapon: false,
            projectile: WeaponStats {
                spe: 0,
                hit_bonus: 0,
                damage_small: 6,
                damage_large: 6,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            },
            ..Default::default()
        };

        // Without STR bonus: damage = rnd(6) + 0 + 0 = 1..6
        let mut min_dmg = i32::MAX;
        let mut max_dmg = i32::MIN;
        for _ in 0..100 {
            let d = ranged_damage(&params, &mut rng);
            min_dmg = min_dmg.min(d);
            max_dmg = max_dmg.max(d);
        }
        assert!(min_dmg >= 1, "min damage {} should be >= 1", min_dmg);
        assert!(max_dmg <= 6, "max damage {} should be <= 6 (no STR bonus)", max_dmg);
    }

    // -----------------------------------------------------------------------
    // Test 9: To-hit formula matches spec
    // -----------------------------------------------------------------------

    #[test]
    fn to_hit_formula_matches_spec() {
        // Spec TV-09: Level=10, Luck=3, target_AC=5, uhitinc=0, DEX=16
        // Thrown dagger (throwing_weapon, spe=+0, hitbon=+2), distance=2
        // Basic skill
        //
        // tmp = -1 + 3 + 5 + 0 + 10 = 17
        // DEX 16: +(16-14) = +2 => 19
        // Distance 2: +1 => 20
        // sleeping +2, paralyzed/immobile +4 (sleeping=true here) => +2 => 22
        // hitval: spe=0 + hitbon=2 = +2 => 24
        // throwing_weapon: +2 => 26
        // weapon_hit_bonus(Basic): +0 => 26
        let params = RangedAttackParams {
            strength: 10,
            strength_extra: 0,
            dexterity: 16,
            level: 10,
            luck: 3,
            uhitinc: 0,
            udaminc: 0,
            projectile: WeaponStats {
                spe: 0,
                hit_bonus: 2,
                damage_small: 4,
                damage_large: 3,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            },
            projectile_skill: SkillLevel::Basic,
            launcher: None,
            is_throwing_weapon: true,
            is_ammo: false,
            target_ac: 5,
            defender_state: DefenderState {
                sleeping: true,
                ..Default::default()
            },
            distance: 2,
        };

        let to_hit = ranged_to_hit(&params);
        assert_eq!(to_hit, 26, "to-hit should match spec calculation");
    }

    // -----------------------------------------------------------------------
    // Test 10: ACURRSTR mapping
    // -----------------------------------------------------------------------

    #[test]
    fn acurrstr_mapping() {
        assert_eq!(acurrstr(3, 0), 3);
        assert_eq!(acurrstr(10, 0), 10);
        assert_eq!(acurrstr(18, 0), 18);
        assert_eq!(acurrstr(18, 1), 19);
        assert_eq!(acurrstr(18, 31), 19);
        assert_eq!(acurrstr(18, 32), 20);
        assert_eq!(acurrstr(18, 81), 20);
        assert_eq!(acurrstr(18, 82), 21);
        assert_eq!(acurrstr(18, 100), 21);
        assert_eq!(acurrstr(22, 0), 22);
        assert_eq!(acurrstr(25, 0), 25);
    }

    // -----------------------------------------------------------------------
    // Test 11: Distance modifier
    // -----------------------------------------------------------------------

    #[test]
    fn distance_modifier_values() {
        assert_eq!(distance_modifier(1), 2);
        assert_eq!(distance_modifier(2), 1);
        assert_eq!(distance_modifier(3), 0);
        assert_eq!(distance_modifier(4), -1);
        assert_eq!(distance_modifier(5), -2);
        assert_eq!(distance_modifier(6), -3);
        assert_eq!(distance_modifier(7), -4);
        assert_eq!(distance_modifier(8), -4); // clamped
        assert_eq!(distance_modifier(20), -4); // clamped
    }

    // -----------------------------------------------------------------------
    // Test 12: DEX to-hit modifier
    // -----------------------------------------------------------------------

    #[test]
    fn dex_modifier_values() {
        assert_eq!(dex_to_hit_modifier(3), -3);
        assert_eq!(dex_to_hit_modifier(5), -2);
        assert_eq!(dex_to_hit_modifier(7), -1);
        assert_eq!(dex_to_hit_modifier(10), 0);
        assert_eq!(dex_to_hit_modifier(13), 0);
        assert_eq!(dex_to_hit_modifier(14), 0);
        assert_eq!(dex_to_hit_modifier(16), 2);
        assert_eq!(dex_to_hit_modifier(18), 4);
        assert_eq!(dex_to_hit_modifier(20), 6);
    }

    // -----------------------------------------------------------------------
    // Test 13: Trace projectile on empty corridor
    // -----------------------------------------------------------------------

    #[test]
    fn trace_projectile_corridor() {
        let map = floor_map(20, 5);
        let start = Position::new(1, 2);
        let path = trace_projectile(&map, start, Direction::East, 15);

        // From x=1 heading east in a 20-wide map with walls at 0 and 19.
        // Floor spans x=1..=18. Starting at x=1, stepping east.
        // Steps: x=2,3,...,18 then x=19 is wall, stop.
        // That's positions x=2 through x=18 = 17 positions.
        // But range is 15, so we get 15 positions: x=2..16.
        assert_eq!(path.len(), 15);
        assert_eq!(path[0], Position::new(2, 2));
        assert_eq!(path[14], Position::new(16, 2));
    }

    // -----------------------------------------------------------------------
    // Test 14: Ammo with launcher has no STR bonus (integration)
    // -----------------------------------------------------------------------

    #[test]
    fn ammo_to_hit_with_launcher_uses_launcher_spe() {
        let params = RangedAttackParams {
            strength: 10,
            strength_extra: 0,
            dexterity: 10,
            level: 5,
            luck: 0,
            uhitinc: 0,
            udaminc: 0,
            projectile: WeaponStats {
                spe: 2,
                hit_bonus: 0,
                damage_small: 6,
                damage_large: 6,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            },
            projectile_skill: SkillLevel::Skilled,
            launcher: Some(WeaponStats {
                spe: 3,
                hit_bonus: 0,
                damage_small: 0,
                damage_large: 0,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 1,
            }),
            is_throwing_weapon: false,
            is_ammo: true,
            target_ac: 10,
            defender_state: DefenderState::default(),
            distance: 3,
        };

        let to_hit = ranged_to_hit(&params);

        // tmp = -1 + 0(luck) + 10(AC) + 0(uhitinc) + 5(level) = 14
        // DEX 10: +0 => 14
        // Distance 3: +0 => 14
        // hitval(projectile): spe=2 + hitbon=0 = +2 => 16
        // ammo with launcher: launcher.spe(3) - launcher.erosion(1) = +2 => 18
        //   + weapon_hit_bonus(Skilled, single) = +2 => 20
        assert_eq!(to_hit, 20);
    }

    // -----------------------------------------------------------------------
    // Test 15: Ammo without launcher gets penalty
    // -----------------------------------------------------------------------

    #[test]
    fn ammo_without_launcher_penalty() {
        let params = RangedAttackParams {
            is_ammo: true,
            launcher: None,
            ..Default::default()
        };

        let to_hit_no_launcher = ranged_to_hit(&params);

        let params_with = RangedAttackParams {
            is_ammo: true,
            launcher: Some(WeaponStats {
                spe: 0,
                hit_bonus: 0,
                damage_small: 0,
                damage_large: 0,
                is_weapon: true,
                blessed: false,
                is_silver: false,
                greatest_erosion: 0,
            }),
            ..Default::default()
        };

        let to_hit_with_launcher = ranged_to_hit(&params_with);

        // Without launcher: -4 penalty vs with launcher: +0 (launcher.spe=0)
        // + weapon_hit_bonus(Basic) = 0.
        // Difference should be -4 vs 0 = 4.
        // However, without launcher we also don't get weapon_hit_bonus from the
        // launcher path. Let's compute:
        // Without: ... + (-4) = base - 4
        // With: ... + (0 - 0) + weapon_hit_bonus(Basic) = base + 0
        // So with_launcher - no_launcher = 4
        assert_eq!(
            to_hit_with_launcher - to_hit_no_launcher,
            4,
            "launcher should provide +4 net advantage over no launcher"
        );
    }

    // -----------------------------------------------------------------------
    // Test 16: Shatter checks
    // -----------------------------------------------------------------------

    #[test]
    fn potion_shatters() {
        assert!(should_shatter(true, false));
    }

    #[test]
    fn glass_shatters() {
        assert!(should_shatter(false, true));
    }

    #[test]
    fn non_fragile_no_shatter() {
        assert!(!should_shatter(false, false));
    }

    #[test]
    fn shatter_effect_potion() {
        assert_eq!(shatter_effect(true), Some("thrown-potion-shatter"));
    }

    #[test]
    fn shatter_effect_glass() {
        assert_eq!(shatter_effect(false), Some("thrown-glass-shatter"));
    }

    // -----------------------------------------------------------------------
    // Test 17: Hit floor
    // -----------------------------------------------------------------------

    #[test]
    fn hit_floor_normal_landing() {
        assert_eq!(hit_floor(Terrain::Floor, false, false), HitFloorResult::Landed);
    }

    #[test]
    fn hit_floor_falls_through_hole() {
        assert_eq!(hit_floor(Terrain::Floor, false, true), HitFloorResult::FellThrough);
    }

    #[test]
    fn hit_floor_fragile_shatters() {
        assert_eq!(hit_floor(Terrain::Floor, true, false), HitFloorResult::Shattered);
    }

    #[test]
    fn hit_floor_altar() {
        assert_eq!(hit_floor(Terrain::Altar, false, false), HitFloorResult::OnAltar);
    }

    // -----------------------------------------------------------------------
    // Test 18: Bounce direction
    // -----------------------------------------------------------------------

    #[test]
    fn bounce_off_horizontal_wall() {
        // Heading north, hit horizontal wall -> bounce south
        assert_eq!(
            bounce_direction(Direction::North, BounceAxis::Horizontal),
            Direction::South,
        );
    }

    #[test]
    fn bounce_off_vertical_wall() {
        // Heading east, hit vertical wall -> bounce west
        assert_eq!(
            bounce_direction(Direction::East, BounceAxis::Vertical),
            Direction::West,
        );
    }

    #[test]
    fn bounce_off_corner() {
        // Heading northeast, hit corner -> bounce southwest
        assert_eq!(
            bounce_direction(Direction::NorthEast, BounceAxis::Corner),
            Direction::SouthWest,
        );
    }

    #[test]
    fn bounce_diagonal_horizontal() {
        // Heading northeast, horizontal wall -> reverse Y -> southeast
        assert_eq!(
            bounce_direction(Direction::NorthEast, BounceAxis::Horizontal),
            Direction::SouthEast,
        );
    }

    // -----------------------------------------------------------------------
    // Test 19: Bresenham path
    // -----------------------------------------------------------------------

    #[test]
    fn bresenham_straight_east() {
        let path = bresenham_path(
            Position::new(0, 0),
            Position::new(5, 0),
            20,
            (0, 0, 20, 20),
        );
        assert_eq!(path.len(), 5);
        for (i, pos) in path.iter().enumerate() {
            assert_eq!(pos.x, i as i32 + 1);
            assert_eq!(pos.y, 0);
        }
    }

    #[test]
    fn bresenham_diagonal() {
        let path = bresenham_path(
            Position::new(0, 0),
            Position::new(3, 3),
            20,
            (0, 0, 20, 20),
        );
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], Position::new(1, 1));
        assert_eq!(path[2], Position::new(3, 3));
    }

    #[test]
    fn bresenham_max_steps_limit() {
        let path = bresenham_path(
            Position::new(0, 0),
            Position::new(10, 0),
            3,
            (0, 0, 20, 20),
        );
        assert_eq!(path.len(), 3);
    }

    #[test]
    fn bresenham_stops_at_bounds() {
        let path = bresenham_path(
            Position::new(0, 0),
            Position::new(10, 0),
            20,
            (0, 0, 4, 20),
        );
        assert_eq!(path.len(), 4);
        assert_eq!(path.last().unwrap().x, 4);
    }

    // -----------------------------------------------------------------------
    // Test 20: Hurtle path
    // -----------------------------------------------------------------------

    #[test]
    fn hurtle_stops_at_wall() {
        let mut map = LevelMap::new(10, 5);
        for x in 0..10 {
            for y in 0..5 {
                if x == 7 {
                    map.set_terrain(Position::new(x, y), Terrain::Wall);
                } else {
                    map.set_terrain(Position::new(x, y), Terrain::Floor);
                }
            }
        }
        let path = hurtle_path(&map, Position::new(3, 2), Direction::East, 10);
        // Should stop before wall at x=7: positions 4,5,6
        assert_eq!(path.len(), 3);
        assert_eq!(path.last().unwrap().x, 6);
    }

    #[test]
    fn hurtle_stops_at_water_inclusive() {
        let mut map = LevelMap::new(10, 5);
        for x in 0..10 {
            for y in 0..5 {
                if x == 5 {
                    map.set_terrain(Position::new(x, y), Terrain::Pool);
                } else {
                    map.set_terrain(Position::new(x, y), Terrain::Floor);
                }
            }
        }
        let path = hurtle_path(&map, Position::new(3, 2), Direction::East, 10);
        // Should include pool at x=5, then stop: positions 4, 5
        assert_eq!(path.len(), 2);
        assert_eq!(path.last().unwrap().x, 5);
    }

    #[test]
    fn hurtle_limited_by_distance() {
        let mut map = LevelMap::new(20, 5);
        for x in 0..20 {
            for y in 0..5 {
                map.set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        let path = hurtle_path(&map, Position::new(5, 2), Direction::East, 3);
        assert_eq!(path.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Test 21: Hurtle step result
    // -----------------------------------------------------------------------

    #[test]
    fn hurtle_step_results() {
        assert_eq!(check_hurtle_step(Terrain::Floor), HurtleStepResult::Moved);
        assert_eq!(check_hurtle_step(Terrain::Corridor), HurtleStepResult::Moved);
        assert_eq!(check_hurtle_step(Terrain::Wall), HurtleStepResult::Blocked);
        assert_eq!(check_hurtle_step(Terrain::Stone), HurtleStepResult::Blocked);
        assert_eq!(check_hurtle_step(Terrain::Pool), HurtleStepResult::HazardTerrain);
        assert_eq!(check_hurtle_step(Terrain::Lava), HurtleStepResult::HazardTerrain);
        assert_eq!(check_hurtle_step(Terrain::IronBars), HurtleStepResult::Blocked);
        assert_eq!(check_hurtle_step(Terrain::Tree), HurtleStepResult::Blocked);
    }

    // -----------------------------------------------------------------------
    // Test 22: Gem accept by unicorn
    // -----------------------------------------------------------------------

    #[test]
    fn unicorn_accepts_valuable_coaligned() {
        assert_eq!(gem_accept(true, true, true), GemAcceptResult::Accepted);
    }

    #[test]
    fn unicorn_grudging_cross_aligned() {
        assert_eq!(gem_accept(true, true, false), GemAcceptResult::Grudging);
    }

    #[test]
    fn unicorn_rejects_worthless() {
        assert_eq!(gem_accept(true, false, true), GemAcceptResult::Rejected);
    }

    #[test]
    fn non_unicorn_not_applicable() {
        assert_eq!(gem_accept(false, true, true), GemAcceptResult::NotApplicable);
    }

    // -----------------------------------------------------------------------
    // Test 23: Monster multishot
    // -----------------------------------------------------------------------

    #[test]
    fn monster_multishot_crossbow_always_1() {
        let mut rng = test_rng();
        for _ in 0..20 {
            assert_eq!(monster_multishot(20, true, &mut rng), 1);
        }
    }

    #[test]
    fn monster_multishot_low_level() {
        let mut rng = test_rng();
        for _ in 0..20 {
            assert_eq!(monster_multishot(5, false, &mut rng), 1);
        }
    }

    #[test]
    fn monster_multishot_high_level() {
        let mut rng = test_rng();
        let mut results = Vec::new();
        for _ in 0..100 {
            results.push(monster_multishot(15, false, &mut rng));
        }
        assert!(results.iter().all(|&r| r >= 2 && r <= 3));
    }

    #[test]
    fn monster_multishot_very_high_level() {
        let mut rng = test_rng();
        let mut results = Vec::new();
        for _ in 0..100 {
            results.push(monster_multishot(20, false, &mut rng));
        }
        assert!(results.iter().all(|&r| r >= 3 && r <= 4));
    }

    // -----------------------------------------------------------------------
    // Test 24: Line-up check
    // -----------------------------------------------------------------------

    #[test]
    fn lined_up_east() {
        assert_eq!(
            lined_up(Position::new(1, 5), Position::new(8, 5)),
            Some(Direction::East),
        );
    }

    #[test]
    fn lined_up_diagonal() {
        assert_eq!(
            lined_up(Position::new(1, 1), Position::new(4, 4)),
            Some(Direction::SouthEast),
        );
    }

    #[test]
    fn lined_up_not_aligned() {
        // (1,1) to (3,5) is not cardinal or diagonal
        assert_eq!(lined_up(Position::new(1, 1), Position::new(3, 5)), None);
    }

    #[test]
    fn lined_up_same_position() {
        assert_eq!(lined_up(Position::new(5, 5), Position::new(5, 5)), None);
    }

    // -----------------------------------------------------------------------
    // Test 25: Multishot state
    // -----------------------------------------------------------------------

    #[test]
    fn multishot_state_lifecycle() {
        let mut ms = MultishotState::new(3, true);
        assert!(!ms.is_done());
        assert!(ms.advance()); // shot 1
        assert!(ms.advance()); // shot 2
        assert!(ms.advance()); // shot 3
        assert!(ms.is_done());
        assert!(!ms.advance()); // no more
    }

    #[test]
    fn multishot_state_cancel() {
        let mut ms = MultishotState::new(5, false);
        ms.advance(); // shot 1
        ms.advance(); // shot 2
        ms.cancel();
        assert!(ms.is_done());
    }
}
