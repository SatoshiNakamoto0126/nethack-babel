//! Explosion system for NetHack Babel.
//!
//! Implements explosion mechanics for wand/spell/trap/artifact explosions.
//! Covers blast radius calculation, damage with resistance/half-damage
//! adjustments, item destruction by element, and type-to-visual mapping.
//!
//! Reference: `src/explode.c` from NetHack 3.7 (rev 1.122).

use rand::Rng;

use nethack_babel_data::{DamageType, ObjectClass};

// ---------------------------------------------------------------------------
// Explosion visual type
// ---------------------------------------------------------------------------

/// Visual display type for an explosion, controlling the glyph color/style.
/// Corresponds to `EXPL_xxx` defines in C NetHack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExplosionType {
    /// EXPL_DARK — dark explosion
    Dark,
    /// EXPL_NOXIOUS — noxious cloud (green)
    Noxious,
    /// EXPL_MUDDY — muddy
    Muddy,
    /// EXPL_WET — wet/watery
    Wet,
    /// EXPL_MAGICAL — magical (blue)
    Magical,
    /// EXPL_FIERY — fiery (red/orange)
    Fiery,
    /// EXPL_FROSTY — frosty (white/cyan)
    Frosty,
}

// ---------------------------------------------------------------------------
// Explosion tile result
// ---------------------------------------------------------------------------

/// Result of an explosion at a single tile.
#[derive(Debug, Clone)]
pub struct ExplosionTileResult {
    pub x: i32,
    pub y: i32,
    pub damage_dealt: i32,
    pub hit_player: bool,
    pub hit_monsters: Vec<String>,
    pub items_destroyed: Vec<String>,
    pub terrain_changed: bool,
}

// ---------------------------------------------------------------------------
// Explosion result
// ---------------------------------------------------------------------------

/// Result of a full explosion across all tiles in its blast area.
#[derive(Debug, Clone)]
pub struct ExplosionResult {
    pub center_x: i32,
    pub center_y: i32,
    pub radius: i32,
    pub explosion_type: ExplosionType,
    pub tiles: Vec<ExplosionTileResult>,
    pub total_damage: i32,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// Check whether a position `(x, y)` is within the Chebyshev-distance
/// explosion radius of centre `(cx, cy)`.
///
/// NetHack explosions use a 3×3 grid (radius 1), meaning every tile at
/// most 1 step away in both x and y is inside the blast.
pub fn in_blast_radius(cx: i32, cy: i32, x: i32, y: i32, radius: i32) -> bool {
    let dx = (x - cx).abs();
    let dy = (y - cy).abs();
    dx <= radius && dy <= radius
}

/// Calculate damage to a target, applying resistance and half-damage
/// armour adjustments.
///
/// From C `explode()`:
/// - Resistance halves damage: `(dam + 1) / 2`
/// - Half-physical-damage (e.g. from certain armour) halves physical/acid:
///   applied via `Maybe_Half_Phys` in C, here explicitly.
/// - Both can stack (resistance first, then half-damage).
pub fn explosion_damage(
    base_damage: i32,
    damage_type: DamageType,
    has_resistance: bool,
    has_half_damage: bool,
) -> i32 {
    let mut dam = base_damage;

    // Resistance halves damage (rounded up like C: `(dam + 1) / 2`)
    if has_resistance {
        dam = (dam + 1) / 2;
    }

    // Half_physical_damage applies to Physical and Acid only
    // (matching C's `Maybe_Half_Phys` in `explode()`)
    if has_half_damage
        && matches!(damage_type, DamageType::Physical | DamageType::Acid)
    {
        dam = (dam + 1) / 2;
    }

    dam
}

/// Check whether an item of the given object class would be destroyed
/// by an explosion of the given damage type.
///
/// From C `destroy_items()` and `explode()`:
/// - Fire:       scrolls (`?`/Scroll), spellbooks (`+`/Spellbook), potions (`!`/Potion)
/// - Cold:       potions shatter
/// - Lightning:  wands (`/`/Wand), rings (`=`/Ring)
pub fn item_destroyed_by_explosion(
    item_class: ObjectClass,
    damage_type: DamageType,
) -> bool {
    match damage_type {
        DamageType::Fire => matches!(
            item_class,
            ObjectClass::Scroll | ObjectClass::Spellbook | ObjectClass::Potion
        ),
        DamageType::Cold => matches!(item_class, ObjectClass::Potion),
        DamageType::Electricity => matches!(
            item_class,
            ObjectClass::Wand | ObjectClass::Ring
        ),
        _ => false,
    }
}

/// Convert a damage type to the corresponding visual explosion type.
///
/// Matches C `adtyp_to_expltype()`:
/// - Fire → Fiery
/// - Cold → Frosty
/// - Electricity / MagicMissile → Magical
/// - Poison / Physical (gas spore) → Noxious
/// - Acid → Noxious (C uses `EXPL_NOXIOUS` for AD_DRST; acid doesn't have
///   a dedicated type so we use Wet to match the task spec)
/// - Sleep → Noxious
/// - Others → Dark
pub fn damage_to_explosion_type(damage_type: DamageType) -> ExplosionType {
    match damage_type {
        DamageType::Fire => ExplosionType::Fiery,
        DamageType::Cold => ExplosionType::Frosty,
        DamageType::MagicMissile => ExplosionType::Magical,
        DamageType::Electricity => ExplosionType::Magical,
        DamageType::Poison => ExplosionType::Noxious,
        DamageType::Physical => ExplosionType::Noxious,
        DamageType::Acid => ExplosionType::Wet,
        DamageType::Sleep => ExplosionType::Noxious,
        DamageType::Disintegrate => ExplosionType::Dark,
        _ => ExplosionType::Dark,
    }
}

/// Main explosion function.  Explodes in a 3×3 area centred on
/// `(center_x, center_y)`.
///
/// Parameters:
/// - `center_x`, `center_y` — explosion centre position
/// - `damage_dice` — `(num_dice, die_size)`, e.g. `(6, 6)` for a 6d6
///   fireball
/// - `damage_type` — what kind of damage (`DamageType` enum)
/// - `explosion_type` — visual type for display
/// - `source_name` — what caused the explosion (for messages)
/// - `rng` — random number generator
///
/// In C NetHack, damage is rolled once and applied uniformly to all tiles
/// in the blast area (no distance fall-off).  We replicate that here.
pub fn explode(
    center_x: i32,
    center_y: i32,
    damage_dice: (i32, i32),
    damage_type: DamageType,
    explosion_type: ExplosionType,
    _source_name: &str,
    rng: &mut impl Rng,
) -> ExplosionResult {
    let (num_dice, die_size) = damage_dice;

    // Roll total damage once, same as C `explode()`.
    let mut total_roll = 0i32;
    for _ in 0..num_dice {
        total_roll += rng.gen_range(1..=die_size);
    }

    let radius = 1; // standard 3×3 blast
    let mut tiles = Vec::with_capacity(9);
    let mut total_damage = 0i32;

    for dx in -radius..=radius {
        for dy in -radius..=radius {
            let tx = center_x + dx;
            let ty = center_y + dy;

            // Each tile receives the full rolled damage (no fall-off).
            // Actual resistance/half-damage would be applied per-target
            // by the caller based on monster/player properties.
            let tile_damage = total_roll;
            total_damage += tile_damage;

            tiles.push(ExplosionTileResult {
                x: tx,
                y: ty,
                damage_dealt: tile_damage,
                hit_player: false,
                hit_monsters: Vec::new(),
                items_destroyed: Vec::new(),
                terrain_changed: false,
            });
        }
    }

    ExplosionResult {
        center_x,
        center_y,
        radius,
        explosion_type,
        tiles,
        total_damage,
    }
}

/// Scatter debris from an explosion.
///
/// Simplified version of C's `scatter()`.  Returns positions where
/// debris might land, each at a random offset within `radius` of centre.
pub fn scatter(
    center_x: i32,
    center_y: i32,
    radius: i32,
    rng: &mut impl Rng,
) -> Vec<(i32, i32)> {
    let count = rng.gen_range(1..=((2 * radius + 1) * (2 * radius + 1)));
    let mut positions = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let dx = rng.gen_range(-radius..=radius);
        let dy = rng.gen_range(-radius..=radius);
        positions.push((center_x + dx, center_y + dy));
    }

    positions
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn test_rng() -> StdRng {
        StdRng::seed_from_u64(42)
    }

    // -- in_blast_radius --

    #[test]
    fn test_in_blast_radius_center() {
        assert!(in_blast_radius(5, 5, 5, 5, 1));
    }

    #[test]
    fn test_in_blast_radius_edge() {
        assert!(in_blast_radius(5, 5, 6, 6, 1));
    }

    #[test]
    fn test_in_blast_radius_adjacent() {
        assert!(in_blast_radius(5, 5, 4, 5, 1));
        assert!(in_blast_radius(5, 5, 5, 4, 1));
        assert!(in_blast_radius(5, 5, 6, 5, 1));
        assert!(in_blast_radius(5, 5, 5, 6, 1));
    }

    #[test]
    fn test_in_blast_radius_outside() {
        assert!(!in_blast_radius(5, 5, 7, 7, 1));
    }

    #[test]
    fn test_in_blast_radius_outside_one_axis() {
        // (5,5) → (7,5) is dx=2 which exceeds radius 1
        assert!(!in_blast_radius(5, 5, 7, 5, 1));
    }

    #[test]
    fn test_in_blast_radius_larger() {
        assert!(in_blast_radius(10, 10, 12, 12, 2));
        assert!(!in_blast_radius(10, 10, 13, 10, 2));
    }

    // -- explosion_damage --

    #[test]
    fn test_explosion_damage_no_resistance() {
        assert_eq!(explosion_damage(20, DamageType::Fire, false, false), 20);
    }

    #[test]
    fn test_explosion_damage_with_resistance() {
        // (20 + 1) / 2 = 10 (rounded up)
        assert_eq!(explosion_damage(20, DamageType::Fire, true, false), 10);
    }

    #[test]
    fn test_explosion_damage_with_resistance_odd() {
        // (15 + 1) / 2 = 8
        assert_eq!(explosion_damage(15, DamageType::Cold, true, false), 8);
    }

    #[test]
    fn test_explosion_damage_with_half_damage_physical() {
        // Half-damage applies to Physical: (20 + 1) / 2 = 10
        assert_eq!(
            explosion_damage(20, DamageType::Physical, false, true),
            10
        );
    }

    #[test]
    fn test_explosion_damage_with_half_damage_acid() {
        // Half-damage applies to Acid too
        assert_eq!(explosion_damage(20, DamageType::Acid, false, true), 10);
    }

    #[test]
    fn test_explosion_damage_half_damage_not_fire() {
        // Half-damage does NOT apply to Fire
        assert_eq!(explosion_damage(20, DamageType::Fire, false, true), 20);
    }

    #[test]
    fn test_explosion_damage_both_resistance_and_half() {
        // Physical with both: resistance first: (20+1)/2=10, then half: (10+1)/2=5
        assert_eq!(
            explosion_damage(20, DamageType::Physical, true, true),
            5
        );
    }

    // -- item_destroyed_by_explosion --

    #[test]
    fn test_item_destroyed_by_fire_scroll() {
        assert!(item_destroyed_by_explosion(
            ObjectClass::Scroll,
            DamageType::Fire
        ));
    }

    #[test]
    fn test_item_destroyed_by_fire_spellbook() {
        assert!(item_destroyed_by_explosion(
            ObjectClass::Spellbook,
            DamageType::Fire
        ));
    }

    #[test]
    fn test_item_destroyed_by_fire_potion() {
        assert!(item_destroyed_by_explosion(
            ObjectClass::Potion,
            DamageType::Fire
        ));
    }

    #[test]
    fn test_item_not_destroyed_by_fire_weapon() {
        assert!(!item_destroyed_by_explosion(
            ObjectClass::Weapon,
            DamageType::Fire
        ));
    }

    #[test]
    fn test_item_destroyed_by_cold_potion() {
        assert!(item_destroyed_by_explosion(
            ObjectClass::Potion,
            DamageType::Cold
        ));
    }

    #[test]
    fn test_item_not_destroyed_by_cold_scroll() {
        assert!(!item_destroyed_by_explosion(
            ObjectClass::Scroll,
            DamageType::Cold
        ));
    }

    #[test]
    fn test_item_destroyed_by_lightning_wand() {
        assert!(item_destroyed_by_explosion(
            ObjectClass::Wand,
            DamageType::Electricity
        ));
    }

    #[test]
    fn test_item_destroyed_by_lightning_ring() {
        assert!(item_destroyed_by_explosion(
            ObjectClass::Ring,
            DamageType::Electricity
        ));
    }

    #[test]
    fn test_item_not_destroyed_by_physical() {
        assert!(!item_destroyed_by_explosion(
            ObjectClass::Scroll,
            DamageType::Physical
        ));
    }

    // -- damage_to_explosion_type --

    #[test]
    fn test_damage_to_explosion_type_fire() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Fire),
            ExplosionType::Fiery
        );
    }

    #[test]
    fn test_damage_to_explosion_type_cold() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Cold),
            ExplosionType::Frosty
        );
    }

    #[test]
    fn test_damage_to_explosion_type_magic_missile() {
        assert_eq!(
            damage_to_explosion_type(DamageType::MagicMissile),
            ExplosionType::Magical
        );
    }

    #[test]
    fn test_damage_to_explosion_type_electricity() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Electricity),
            ExplosionType::Magical
        );
    }

    #[test]
    fn test_damage_to_explosion_type_poison() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Poison),
            ExplosionType::Noxious
        );
    }

    #[test]
    fn test_damage_to_explosion_type_acid() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Acid),
            ExplosionType::Wet
        );
    }

    #[test]
    fn test_damage_to_explosion_type_sleep() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Sleep),
            ExplosionType::Noxious
        );
    }

    #[test]
    fn test_damage_to_explosion_type_disintegrate() {
        assert_eq!(
            damage_to_explosion_type(DamageType::Disintegrate),
            ExplosionType::Dark
        );
    }

    // -- explode --

    #[test]
    fn test_explode_basic_fire_9_tiles() {
        let mut rng = test_rng();
        let result = explode(5, 5, (6, 6), DamageType::Fire, ExplosionType::Fiery, "fireball", &mut rng);

        // Standard 3×3 blast = 9 tiles
        assert_eq!(result.tiles.len(), 9);
        assert_eq!(result.radius, 1);
        assert_eq!(result.center_x, 5);
        assert_eq!(result.center_y, 5);
        assert_eq!(result.explosion_type, ExplosionType::Fiery);
    }

    #[test]
    fn test_explode_damage_dice_range() {
        // 6d6: min=6, max=36
        let mut rng = test_rng();
        let result = explode(5, 5, (6, 6), DamageType::Fire, ExplosionType::Fiery, "fireball", &mut rng);

        // Each tile gets the same roll
        let per_tile = result.tiles[0].damage_dealt;
        assert!(per_tile >= 6 && per_tile <= 36, "damage {} not in 6..=36", per_tile);

        // All tiles get the same damage (uniform, no fall-off)
        for tile in &result.tiles {
            assert_eq!(tile.damage_dealt, per_tile);
        }
    }

    #[test]
    fn test_explode_total_damage() {
        let mut rng = test_rng();
        let result = explode(5, 5, (6, 6), DamageType::Fire, ExplosionType::Fiery, "fireball", &mut rng);

        let per_tile = result.tiles[0].damage_dealt;
        assert_eq!(result.total_damage, per_tile * 9);
    }

    #[test]
    fn test_explode_tile_coordinates() {
        let mut rng = test_rng();
        let result = explode(10, 20, (1, 1), DamageType::Cold, ExplosionType::Frosty, "frost", &mut rng);

        let mut coords: Vec<(i32, i32)> = result.tiles.iter().map(|t| (t.x, t.y)).collect();
        coords.sort();

        let mut expected = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                expected.push((10 + dx, 20 + dy));
            }
        }
        expected.sort();

        assert_eq!(coords, expected);
    }

    // -- scatter --

    #[test]
    fn test_scatter_within_radius() {
        let mut rng = test_rng();
        let positions = scatter(5, 5, 2, &mut rng);

        assert!(!positions.is_empty());
        for &(px, py) in &positions {
            assert!(
                in_blast_radius(5, 5, px, py, 2),
                "scatter position ({}, {}) outside radius 2 of (5,5)",
                px,
                py,
            );
        }
    }

    #[test]
    fn test_scatter_produces_positions() {
        let mut rng = test_rng();
        let positions = scatter(0, 0, 1, &mut rng);
        // Should produce at least 1 position
        assert!(!positions.is_empty());
        // At most (2*1+1)^2 = 9 positions
        assert!(positions.len() <= 9);
    }
}
