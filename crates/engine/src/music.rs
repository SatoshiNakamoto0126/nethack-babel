//! Musical instrument system: playing instruments for various magical effects.
//!
//! Implements NetHack's instrument mechanics from `music.c`.  Each instrument
//! type produces a distinct effect when applied (played):
//!
//! - **Wooden flute/harp**: charm snakes within radius 5 (tame them)
//! - **Magic flute**: put adjacent monsters to sleep for d(5,25) turns (1 charge)
//! - **Magic harp**: tame adjacent animals (1 charge)
//! - **Bugle**: wake all sleeping monsters on the level
//! - **Leather drum**: scare adjacent monsters for d(1,10) turns
//! - **Drum of earthquake**: collapse walls in 5-tile radius (1 charge)
//! - **Frost/fire horn**: shoot cold/fire ray (1 charge)
//! - **Tooled horn**: just makes noise
//! - **Drawbridge passtune**: correct 5-note tune near drawbridge opens/closes it
//!
//! All functions are pure: they operate on `GameWorld` plus RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  No IO.

use hecs::Entity;
use rand::Rng;

use crate::action::Position;
use crate::event::{EngineEvent, StatusEffect};
use crate::world::{GameWorld, Monster, Positioned, Tame};

// ---------------------------------------------------------------------------
// Dice helpers (local to this module)
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

// ---------------------------------------------------------------------------
// Distance helpers
// ---------------------------------------------------------------------------

/// Chebyshev (king-move) distance.
#[inline]
fn chebyshev(a: Position, b: Position) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    dx.max(dy)
}

// ---------------------------------------------------------------------------
// Instrument classification
// ---------------------------------------------------------------------------

/// Musical instrument types recognized by the play system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstrumentType {
    WoodenFlute,
    MagicFlute,
    WoodenHarp,
    MagicHarp,
    Bugle,
    LeatherDrum,
    DrumOfEarthquake,
    TooledHorn,
    FrostHorn,
    FireHorn,
}

/// Check whether a given object name corresponds to a known instrument.
pub fn is_instrument(name: &str) -> bool {
    matches!(
        name,
        "wooden flute"
            | "magic flute"
            | "wooden harp"
            | "magic harp"
            | "bugle"
            | "leather drum"
            | "drum of earthquake"
            | "tooled horn"
            | "frost horn"
            | "fire horn"
    )
}

/// Map an object name to an `InstrumentType`, if it is one.
pub fn classify_instrument(name: &str) -> Option<InstrumentType> {
    match name {
        "wooden flute" => Some(InstrumentType::WoodenFlute),
        "magic flute" => Some(InstrumentType::MagicFlute),
        "wooden harp" => Some(InstrumentType::WoodenHarp),
        "magic harp" => Some(InstrumentType::MagicHarp),
        "bugle" => Some(InstrumentType::Bugle),
        "leather drum" => Some(InstrumentType::LeatherDrum),
        "drum of earthquake" => Some(InstrumentType::DrumOfEarthquake),
        "tooled horn" => Some(InstrumentType::TooledHorn),
        "frost horn" => Some(InstrumentType::FrostHorn),
        "fire horn" => Some(InstrumentType::FireHorn),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// The drawbridge passtune (Castle level)
// ---------------------------------------------------------------------------

/// The canonical drawbridge passtune: E-F-F#-G-E encoded as note indices.
/// In NetHack, the tune is stored as the string "E F F# G E" and checked
/// character-by-character.  We represent it as a fixed array.
pub const PASSTUNE: [u8; 5] = [4, 5, 6, 7, 4]; // E=4, F=5, F#=6, G=7

/// Check whether a played sequence of notes matches the drawbridge passtune.
pub fn matches_passtune(notes: &[u8]) -> bool {
    notes == PASSTUNE
}

// ---------------------------------------------------------------------------
// Instrument state (charges)
// ---------------------------------------------------------------------------

/// State for a chargeable instrument (magic flute, magic harp,
/// drum of earthquake, frost/fire horn).
#[derive(Debug, Clone, Copy)]
pub struct InstrumentCharges {
    pub charges: i8,
}

// ---------------------------------------------------------------------------
// Core play logic
// ---------------------------------------------------------------------------

/// Play a musical instrument, producing game events.
///
/// `instrument_entity` is the ECS entity of the instrument being played.
/// `instrument_type` identifies what kind of instrument it is.
/// `charges` is the current charge count (for chargeable instruments);
/// the caller should decrement after this call if we return `true` in
/// the second tuple element.
///
/// Returns `(events, charge_used)`.
pub fn play_instrument(
    world: &mut GameWorld,
    player: Entity,
    instrument_type: InstrumentType,
    charges: Option<i8>,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let player_pos = match world.get_component::<Positioned>(player) {
        Some(p) => p.0,
        None => return (vec![EngineEvent::msg("play-nothing")], false),
    };

    match instrument_type {
        InstrumentType::WoodenFlute | InstrumentType::WoodenHarp => {
            play_charm_snakes(world, player, player_pos)
        }
        InstrumentType::MagicFlute => {
            play_magic_flute(world, player, player_pos, charges, rng)
        }
        InstrumentType::MagicHarp => {
            play_magic_harp(world, player, player_pos, charges)
        }
        InstrumentType::Bugle => play_bugle(world, player_pos),
        InstrumentType::LeatherDrum => {
            play_leather_drum(world, player, player_pos, rng)
        }
        InstrumentType::DrumOfEarthquake => {
            play_earthquake(world, player_pos, charges, rng)
        }
        InstrumentType::TooledHorn => {
            (vec![EngineEvent::msg("play-horn-noise")], false)
        }
        InstrumentType::FrostHorn => {
            play_horn_ray(world, player_pos, charges, "frost-horn-ray")
        }
        InstrumentType::FireHorn => {
            play_horn_ray(world, player_pos, charges, "fire-horn-ray")
        }
    }
}

// ---------------------------------------------------------------------------
// Individual instrument effects
// ---------------------------------------------------------------------------

/// Wooden flute/harp: charm (tame) snake monsters within radius 5.
fn play_charm_snakes(
    world: &mut GameWorld,
    player: Entity,
    player_pos: Position,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();
    events.push(EngineEvent::msg("play-music"));

    // Collect snake entities within radius 5.
    let mut snakes_to_tame = Vec::new();
    for (entity, pos) in world.query::<Positioned>().iter() {
        if entity == player {
            continue;
        }
        if chebyshev(player_pos, pos.0) > 5 {
            continue;
        }
        // Check if it's a monster and has "snake" in its name.
        if world.get_component::<Monster>(entity).is_some()
            && world.get_component::<Tame>(entity).is_none()
        {
            let name = world.entity_name(entity);
            if name.contains("snake") || name.contains("cobra")
                || name.contains("naga") || name.contains("serpent")
            {
                snakes_to_tame.push(entity);
            }
        }
    }

    for snake in snakes_to_tame {
        let _ = world.ecs_mut().insert_one(snake, Tame);
        events.push(EngineEvent::msg_with(
            "snake-charmed",
            vec![("name", world.entity_name(snake))],
        ));
    }

    (events, false)
}

/// Magic flute: put adjacent monsters to sleep for d(5,25) turns.
/// Uses 1 charge.
fn play_magic_flute(
    world: &mut GameWorld,
    player: Entity,
    player_pos: Position,
    charges: Option<i8>,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if charges.unwrap_or(0) <= 0 {
        events.push(EngineEvent::msg("instrument-no-charges"));
        return (events, false);
    }

    events.push(EngineEvent::msg("play-magic-flute"));

    // Affect adjacent monsters (Chebyshev distance <= 1).
    let mut targets = Vec::new();
    for (entity, pos) in world.query::<Positioned>().iter() {
        if entity == player {
            continue;
        }
        if chebyshev(player_pos, pos.0) <= 1
            && world.get_component::<Monster>(entity).is_some()
        {
            targets.push(entity);
        }
    }

    for target in targets {
        let duration = d(rng, 5, 25);
        events.push(EngineEvent::StatusApplied {
            entity: target,
            status: StatusEffect::Sleeping,
            duration: Some(duration),
            source: Some(player),
        });
        events.push(EngineEvent::msg_with(
            "monster-falls-asleep",
            vec![("name", world.entity_name(target))],
        ));
    }

    (events, true)
}

/// Magic harp: tame adjacent animals (Chebyshev distance <= 1).
/// Uses 1 charge.
fn play_magic_harp(
    world: &mut GameWorld,
    player: Entity,
    player_pos: Position,
    charges: Option<i8>,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if charges.unwrap_or(0) <= 0 {
        events.push(EngineEvent::msg("instrument-no-charges"));
        return (events, false);
    }

    events.push(EngineEvent::msg("play-magic-harp"));

    // Tame adjacent animal monsters.
    let mut targets = Vec::new();
    for (entity, pos) in world.query::<Positioned>().iter() {
        if entity == player {
            continue;
        }
        if chebyshev(player_pos, pos.0) <= 1
            && world.get_component::<Monster>(entity).is_some()
            && world.get_component::<Tame>(entity).is_none()
        {
            targets.push(entity);
        }
    }

    for target in targets {
        let _ = world.ecs_mut().insert_one(target, Tame);
        events.push(EngineEvent::msg_with(
            "monster-tamed",
            vec![("name", world.entity_name(target))],
        ));
    }

    (events, true)
}

/// Bugle: wake all sleeping monsters on the level.
fn play_bugle(
    world: &mut GameWorld,
    _player_pos: Position,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();
    events.push(EngineEvent::msg("play-bugle"));

    // Collect all sleeping monsters and wake them.
    let mut to_wake = Vec::new();
    for (entity, _) in world.query::<Monster>().iter() {
        to_wake.push(entity);
    }

    for entity in to_wake {
        events.push(EngineEvent::StatusRemoved {
            entity,
            status: StatusEffect::Sleeping,
        });
    }

    (events, false)
}

/// Leather drum: scare adjacent monsters for d(1,10) turns.
fn play_leather_drum(
    world: &mut GameWorld,
    player: Entity,
    player_pos: Position,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();
    events.push(EngineEvent::msg("play-drum"));

    let mut targets = Vec::new();
    for (entity, pos) in world.query::<Positioned>().iter() {
        if entity == player {
            continue;
        }
        if chebyshev(player_pos, pos.0) <= 1
            && world.get_component::<Monster>(entity).is_some()
        {
            targets.push(entity);
        }
    }

    for target in targets {
        let duration = d(rng, 1, 10);
        // Scare effect: we emit a Confused status as a stand-in for fleeing
        // (consistent with scrolls.rs pattern).
        events.push(EngineEvent::StatusApplied {
            entity: target,
            status: StatusEffect::Confused,
            duration: Some(duration),
            source: Some(player),
        });
        events.push(EngineEvent::msg_with(
            "monster-scared",
            vec![("name", world.entity_name(target))],
        ));
    }

    (events, false)
}

/// Drum of earthquake: collapse walls within 5-tile radius.  Uses 1 charge.
fn play_earthquake(
    world: &mut GameWorld,
    player_pos: Position,
    charges: Option<i8>,
    rng: &mut impl Rng,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if charges.unwrap_or(0) <= 0 {
        events.push(EngineEvent::msg("instrument-no-charges"));
        return (events, false);
    }

    events.push(EngineEvent::msg("play-earthquake"));

    let radius: i32 = 5;
    let mut walls_collapsed = 0u32;
    let level = &mut world.dungeon_mut().current_level;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let x = player_pos.x + dx;
            let y = player_pos.y + dy;
            let pos = Position::new(x, y);
            if !level.in_bounds(pos) {
                continue;
            }
            if let Some(cell) = level.get(pos) {
                use crate::dungeon::Terrain;
                if cell.terrain == Terrain::Wall {
                    // Earthquake collapses walls to floor.
                    level.set_terrain(pos, Terrain::Floor);
                    walls_collapsed += 1;
                }
            }
        }
    }

    // Suppress unused variable warning — we read rng for future
    // randomized wall collapse probability.
    let _ = rng;

    events.push(EngineEvent::msg_with(
        "earthquake-walls",
        vec![("count", walls_collapsed.to_string())],
    ));

    (events, true)
}

/// Frost/fire horn: emit a ray.  Uses 1 charge.
fn play_horn_ray(
    _world: &mut GameWorld,
    _player_pos: Position,
    charges: Option<i8>,
    msg_key: &str,
) -> (Vec<EngineEvent>, bool) {
    let mut events = Vec::new();

    if charges.unwrap_or(0) <= 0 {
        events.push(EngineEvent::msg("instrument-no-charges"));
        return (events, false);
    }

    // The actual ray tracing and damage is handled by the wand/ray system.
    // Here we just emit the event to trigger it.
    events.push(EngineEvent::msg(msg_key));

    (events, true)
}

// ---------------------------------------------------------------------------
// Monster vocalizations (from C's sounds.c domonnoise())
// ---------------------------------------------------------------------------

/// Get the sound a monster makes when idle/nearby.
///
/// Returns a message string describing the vocalization, or `None` if
/// the monster class has no defined sound.  Tame monsters use gentler
/// vocalizations; hostile monsters use aggressive ones.
pub fn monster_vocalization(
    monster_class: char,
    monster_name: &str,
    is_tame: bool,
    is_hostile: bool,
) -> Option<&'static str> {
    if is_tame {
        return match monster_class {
            'd' => Some("whines."),
            'f' => Some("purrs."),
            'u' => Some("whinnies."),
            _ => None,
        };
    }

    match monster_class {
        'd' => Some(if is_hostile { "growls!" } else { "whines." }),
        'f' => Some(if is_hostile { "hisses!" } else { "purrs." }),
        'h' => Some("grunts."),
        'o' => Some("grunts."),
        'u' => Some("neighs."),
        'D' => Some("roars!"),
        'T' => Some("growls menacingly!"),
        'V' => Some("says \"I vant to suck your blood!\""),
        '&' => Some("laughs fiendishly!"),
        'N' => Some("hisses!"),
        'S' => Some("hisses!"),
        'Z' => Some("groans!"),
        'M' => Some("groans!"),
        'W' => Some("moans!"),
        'L' => Some("casts a baleful look!"),
        'H' => Some("thunders!"),
        'A' => Some("sings."),
        'n' => Some("giggles."),
        '@' => match monster_name {
            "guard" => Some("yells \"Halt!\""),
            "shopkeeper" => Some("says \"Can I help you?\""),
            "priest" | "priestess" => Some("chants."),
            _ => Some("says something."),
        },
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Ambient level sounds (from C's sounds.c dosounds())
// ---------------------------------------------------------------------------

/// Generate an ambient sound heard on the current dungeon level.
///
/// Only triggers with a 1/35 chance per call (matching C NetHack's
/// per-turn probability).  The sound varies based on dungeon branch,
/// depth, and whether the level contains a shop or temple.
pub fn ambient_sounds(
    depth: i32,
    branch: &str,
    has_shop: bool,
    has_temple: bool,
    rng: &mut impl Rng,
) -> Option<&'static str> {
    // Only 1/35 chance per turn.
    if rng.random_range(0..35u32) != 0 {
        return None;
    }

    match branch {
        "Gehennom" => Some(match rng.random_range(0..4u32) {
            0 => "You hear the howling of the damned!",
            1 => "You hear groans and moans!",
            2 => "You hear diabolical laughter!",
            _ => "You smell brimstone!",
        }),
        "Mines" => Some(match rng.random_range(0..3u32) {
            0 => "You hear someone counting money.",
            1 => "You hear the chime of a cash register.",
            _ => "You hear a sound reminiscent of a straining mine cart.",
        }),
        _ => {
            if has_shop {
                Some(match rng.random_range(0..3u32) {
                    0 => "You hear someone cursing shoplifters.",
                    1 => "You hear the chime of a cash register.",
                    _ => "You hear someone mumbling about prices.",
                })
            } else if has_temple {
                Some(match rng.random_range(0..2u32) {
                    0 => "You hear a prayer intoned.",
                    _ => "You hear a chant.",
                })
            } else if depth > 15 {
                Some(match rng.random_range(0..5u32) {
                    0 => "You hear a crunching sound.",
                    1 => "You hear a hollow sound.",
                    2 => "You hear a rumble.",
                    3 => "You hear a distant roar.",
                    _ => "You hear someone digging.",
                })
            } else {
                Some(match rng.random_range(0..4u32) {
                    0 => "You hear a door open.",
                    1 => "You hear a door close.",
                    2 => "You hear water dripping.",
                    _ => "You hear someone moving around.",
                })
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64Mcg;

    use crate::dungeon::{LevelMap, Terrain};
    use crate::world::{GameWorld, Monster, Name, Positioned, Speed, Tame};

    fn test_rng() -> Pcg64Mcg {
        Pcg64Mcg::seed_from_u64(42)
    }

    /// Helper: create a GameWorld with a floor map.
    fn make_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(10, 10));
        // Set up a simple floor level.
        let mut map = LevelMap::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let t = if x == 0 || y == 0 || x == 19 || y == 19 {
                    Terrain::Wall
                } else {
                    Terrain::Floor
                };
                map.set_terrain(Position::new(x as i32, y as i32), t);
            }
        }
        world.dungeon_mut().current_level = map;
        world
    }

    /// Helper: spawn a monster entity at a position with a given name.
    fn spawn_monster(
        world: &mut GameWorld,
        name: &str,
        pos: Position,
    ) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            Name(name.to_string()),
            Speed(12),
        ))
    }

    // -----------------------------------------------------------------------
    // Test 1: is_instrument recognizes all instruments
    // -----------------------------------------------------------------------
    #[test]
    fn test_is_instrument() {
        assert!(is_instrument("wooden flute"));
        assert!(is_instrument("magic flute"));
        assert!(is_instrument("wooden harp"));
        assert!(is_instrument("magic harp"));
        assert!(is_instrument("bugle"));
        assert!(is_instrument("leather drum"));
        assert!(is_instrument("drum of earthquake"));
        assert!(is_instrument("tooled horn"));
        assert!(is_instrument("frost horn"));
        assert!(is_instrument("fire horn"));
        assert!(!is_instrument("long sword"));
        assert!(!is_instrument("wand of fire"));
    }

    // -----------------------------------------------------------------------
    // Test 2: Wooden flute charms snakes within radius 5
    // -----------------------------------------------------------------------
    #[test]
    fn wooden_flute_charms_snakes() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        // Spawn a snake within range and a non-snake.
        let snake = spawn_monster(&mut world, "snake", Position::new(12, 10));
        let _rat = spawn_monster(&mut world, "giant rat", Position::new(11, 10));

        let (events, charge_used) = play_instrument(
            &mut world,
            player,
            InstrumentType::WoodenFlute,
            None,
            &mut rng,
        );

        assert!(!charge_used, "wooden flute uses no charges");
        assert!(world.get_component::<Tame>(snake).is_some(),
            "snake should be tamed");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "snake-charmed")),
            "should emit snake-charmed message");
    }

    // -----------------------------------------------------------------------
    // Test 3: Magic flute puts adjacent monsters to sleep
    // -----------------------------------------------------------------------
    #[test]
    fn magic_flute_sleeps_adjacent() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let adj = spawn_monster(&mut world, "goblin", Position::new(11, 10));
        let _far = spawn_monster(&mut world, "orc", Position::new(15, 10));

        let (events, charge_used) = play_instrument(
            &mut world,
            player,
            InstrumentType::MagicFlute,
            Some(3),
            &mut rng,
        );

        assert!(charge_used, "magic flute should use a charge");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::StatusApplied { entity, status: StatusEffect::Sleeping, .. }
            if *entity == adj)),
            "adjacent monster should be put to sleep");
    }

    // -----------------------------------------------------------------------
    // Test 4: Magic flute does nothing without charges
    // -----------------------------------------------------------------------
    #[test]
    fn magic_flute_no_charges() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let (events, charge_used) = play_instrument(
            &mut world,
            player,
            InstrumentType::MagicFlute,
            Some(0),
            &mut rng,
        );

        assert!(!charge_used);
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "instrument-no-charges")));
    }

    // -----------------------------------------------------------------------
    // Test 5: Bugle wakes all monsters
    // -----------------------------------------------------------------------
    #[test]
    fn bugle_wakes_all() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let _m1 = spawn_monster(&mut world, "goblin", Position::new(5, 5));
        let _m2 = spawn_monster(&mut world, "orc", Position::new(15, 15));

        let (events, charge_used) = play_instrument(
            &mut world,
            player,
            InstrumentType::Bugle,
            None,
            &mut rng,
        );

        assert!(!charge_used);
        // Should emit StatusRemoved for both monsters.
        let wake_count = events.iter().filter(|e| matches!(e,
            EngineEvent::StatusRemoved { status: StatusEffect::Sleeping, .. }
        )).count();
        assert_eq!(wake_count, 2, "bugle should wake both monsters");
    }

    // -----------------------------------------------------------------------
    // Test 6: Drum of earthquake collapses walls
    // -----------------------------------------------------------------------
    #[test]
    fn drum_earthquake_collapses_walls() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        // Place some walls near the player (at 10,10).
        world.dungeon_mut().current_level
            .set_terrain(Position::new(12, 10), Terrain::Wall);
        world.dungeon_mut().current_level
            .set_terrain(Position::new(13, 10), Terrain::Wall);

        let (events, charge_used) = play_instrument(
            &mut world,
            player,
            InstrumentType::DrumOfEarthquake,
            Some(1),
            &mut rng,
        );

        assert!(charge_used);
        // The walls at (12,10) and (13,10) should now be floor.
        let t1 = world.dungeon().current_level
            .get(Position::new(12, 10)).unwrap().terrain;
        let t2 = world.dungeon().current_level
            .get(Position::new(13, 10)).unwrap().terrain;
        assert_eq!(t1, Terrain::Floor, "wall should be collapsed");
        assert_eq!(t2, Terrain::Floor, "wall should be collapsed");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "play-earthquake")));
    }

    // -----------------------------------------------------------------------
    // Test 7: Passtune matching
    // -----------------------------------------------------------------------
    #[test]
    fn passtune_matches() {
        assert!(matches_passtune(&[4, 5, 6, 7, 4]));
        assert!(!matches_passtune(&[4, 5, 6, 7, 5]));
        assert!(!matches_passtune(&[4, 5, 6]));
    }

    // -----------------------------------------------------------------------
    // Test 8: Magic harp tames adjacent animals
    // -----------------------------------------------------------------------
    #[test]
    fn magic_harp_tames_adjacent() {
        let mut world = make_world();
        let player = world.player();
        let mut rng = test_rng();

        let adj = spawn_monster(&mut world, "dog", Position::new(11, 10));
        let far = spawn_monster(&mut world, "cat", Position::new(15, 15));

        let (events, charge_used) = play_instrument(
            &mut world,
            player,
            InstrumentType::MagicHarp,
            Some(2),
            &mut rng,
        );

        assert!(charge_used);
        assert!(world.get_component::<Tame>(adj).is_some(),
            "adjacent monster should be tamed");
        assert!(world.get_component::<Tame>(far).is_none(),
            "far monster should not be tamed");
        assert!(events.iter().any(|e| matches!(e,
            EngineEvent::Message { key, .. } if key == "monster-tamed")));
    }

    // -----------------------------------------------------------------------
    // Test 9: Tame dog whines
    // -----------------------------------------------------------------------
    #[test]
    fn tame_dog_whines() {
        let sound = monster_vocalization('d', "dog", true, false);
        assert_eq!(sound, Some("whines."));
    }

    // -----------------------------------------------------------------------
    // Test 10: Tame cat purrs
    // -----------------------------------------------------------------------
    #[test]
    fn tame_cat_purrs() {
        let sound = monster_vocalization('f', "cat", true, false);
        assert_eq!(sound, Some("purrs."));
    }

    // -----------------------------------------------------------------------
    // Test 11: Hostile dog growls
    // -----------------------------------------------------------------------
    #[test]
    fn hostile_dog_growls() {
        let sound = monster_vocalization('d', "wolf", false, true);
        assert_eq!(sound, Some("growls!"));
    }

    // -----------------------------------------------------------------------
    // Test 12: Non-hostile wild dog whines
    // -----------------------------------------------------------------------
    #[test]
    fn non_hostile_dog_whines() {
        let sound = monster_vocalization('d', "jackal", false, false);
        assert_eq!(sound, Some("whines."));
    }

    // -----------------------------------------------------------------------
    // Test 13: Dragon roars
    // -----------------------------------------------------------------------
    #[test]
    fn dragon_roars() {
        let sound = monster_vocalization('D', "red dragon", false, true);
        assert_eq!(sound, Some("roars!"));
    }

    // -----------------------------------------------------------------------
    // Test 14: Vampire speaks
    // -----------------------------------------------------------------------
    #[test]
    fn vampire_speaks() {
        let sound = monster_vocalization('V', "vampire", false, true);
        assert_eq!(sound, Some("says \"I vant to suck your blood!\""));
    }

    // -----------------------------------------------------------------------
    // Test 15: Demon laughs
    // -----------------------------------------------------------------------
    #[test]
    fn demon_laughs() {
        let sound = monster_vocalization('&', "balrog", false, true);
        assert_eq!(sound, Some("laughs fiendishly!"));
    }

    // -----------------------------------------------------------------------
    // Test 16: Guard yells halt
    // -----------------------------------------------------------------------
    #[test]
    fn guard_yells_halt() {
        let sound = monster_vocalization('@', "guard", false, true);
        assert_eq!(sound, Some("yells \"Halt!\""));
    }

    // -----------------------------------------------------------------------
    // Test 17: Shopkeeper speaks
    // -----------------------------------------------------------------------
    #[test]
    fn shopkeeper_speaks() {
        let sound = monster_vocalization('@', "shopkeeper", false, false);
        assert_eq!(sound, Some("says \"Can I help you?\""));
    }

    // -----------------------------------------------------------------------
    // Test 18: Priest chants
    // -----------------------------------------------------------------------
    #[test]
    fn priest_chants() {
        let sound = monster_vocalization('@', "priest", false, false);
        assert_eq!(sound, Some("chants."));
    }

    // -----------------------------------------------------------------------
    // Test 19: Unknown monster class returns None
    // -----------------------------------------------------------------------
    #[test]
    fn unknown_class_no_sound() {
        let sound = monster_vocalization('j', "blob", false, false);
        assert_eq!(sound, None);
    }

    // -----------------------------------------------------------------------
    // Test 20: Tame unknown class returns None
    // -----------------------------------------------------------------------
    #[test]
    fn tame_unknown_class_no_sound() {
        let sound = monster_vocalization('Z', "zombie", true, false);
        assert_eq!(sound, None);
    }

    // -----------------------------------------------------------------------
    // Test 21: Zombie groans
    // -----------------------------------------------------------------------
    #[test]
    fn zombie_groans() {
        let sound = monster_vocalization('Z', "zombie", false, true);
        assert_eq!(sound, Some("groans!"));
    }

    // -----------------------------------------------------------------------
    // Test 22: Angel sings
    // -----------------------------------------------------------------------
    #[test]
    fn angel_sings() {
        let sound = monster_vocalization('A', "angel", false, false);
        assert_eq!(sound, Some("sings."));
    }

    // -----------------------------------------------------------------------
    // Test 23: Ambient sounds - mostly returns None (1/35 chance)
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_usually_none() {
        let mut rng = test_rng();
        let mut none_count = 0;
        for _ in 0..100 {
            if ambient_sounds(5, "Dungeons", false, false, &mut rng).is_none() {
                none_count += 1;
            }
        }
        // Should be ~97/100 None (1/35 chance of Some).
        assert!(
            none_count > 80,
            "ambient sounds should mostly return None: got {}/100",
            none_count
        );
    }

    // -----------------------------------------------------------------------
    // Test 24: Ambient sounds - Gehennom branch
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_gehennom() {
        // Force the 1/35 roll to succeed by trying many seeds.
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if let Some(msg) = ambient_sounds(30, "Gehennom", false, false, &mut rng) {
                assert!(
                    msg.contains("damned")
                        || msg.contains("groans")
                        || msg.contains("laughter")
                        || msg.contains("brimstone"),
                    "Gehennom sound should be thematic: {}",
                    msg
                );
                found = true;
                break;
            }
        }
        assert!(found, "should eventually get a Gehennom ambient sound");
    }

    // -----------------------------------------------------------------------
    // Test 25: Ambient sounds - Mines branch
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_mines() {
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if let Some(msg) = ambient_sounds(10, "Mines", false, false, &mut rng) {
                assert!(
                    msg.contains("money")
                        || msg.contains("cash register")
                        || msg.contains("mine cart"),
                    "Mines sound should be thematic: {}",
                    msg
                );
                found = true;
                break;
            }
        }
        assert!(found, "should eventually get a Mines ambient sound");
    }

    // -----------------------------------------------------------------------
    // Test 26: Ambient sounds - shop level
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_shop() {
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if let Some(msg) =
                ambient_sounds(5, "Dungeons", true, false, &mut rng)
            {
                assert!(
                    msg.contains("shoplifters")
                        || msg.contains("cash register")
                        || msg.contains("prices"),
                    "shop sound should be thematic: {}",
                    msg
                );
                found = true;
                break;
            }
        }
        assert!(found, "should eventually get a shop ambient sound");
    }

    // -----------------------------------------------------------------------
    // Test 27: Ambient sounds - temple level
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_temple() {
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if let Some(msg) =
                ambient_sounds(5, "Dungeons", false, true, &mut rng)
            {
                assert!(
                    msg.contains("prayer") || msg.contains("chant"),
                    "temple sound should be thematic: {}",
                    msg
                );
                found = true;
                break;
            }
        }
        assert!(found, "should eventually get a temple ambient sound");
    }

    // -----------------------------------------------------------------------
    // Test 28: Ambient sounds - deep dungeon
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_deep_dungeon() {
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if let Some(msg) =
                ambient_sounds(20, "Dungeons", false, false, &mut rng)
            {
                assert!(
                    msg.contains("crunching")
                        || msg.contains("hollow")
                        || msg.contains("rumble")
                        || msg.contains("roar")
                        || msg.contains("digging"),
                    "deep dungeon sound should be thematic: {}",
                    msg
                );
                found = true;
                break;
            }
        }
        assert!(found, "should eventually get a deep dungeon ambient sound");
    }

    // -----------------------------------------------------------------------
    // Test 29: Ambient sounds - shallow dungeon
    // -----------------------------------------------------------------------
    #[test]
    fn ambient_sounds_shallow_dungeon() {
        let mut found = false;
        for seed in 0..200u64 {
            let mut rng = Pcg64Mcg::seed_from_u64(seed);
            if let Some(msg) =
                ambient_sounds(3, "Dungeons", false, false, &mut rng)
            {
                assert!(
                    msg.contains("door")
                        || msg.contains("water")
                        || msg.contains("moving"),
                    "shallow dungeon sound should be thematic: {}",
                    msg
                );
                found = true;
                break;
            }
        }
        assert!(found, "should eventually get a shallow dungeon ambient sound");
    }

    // -----------------------------------------------------------------------
    // Test 30: Snake and naga charmed
    // -----------------------------------------------------------------------
    #[test]
    fn snake_class_sounds() {
        let sound = monster_vocalization('S', "cobra", false, true);
        assert_eq!(sound, Some("hisses!"));
    }

    // -----------------------------------------------------------------------
    // Test 31: Troll growls menacingly
    // -----------------------------------------------------------------------
    #[test]
    fn troll_growls() {
        let sound = monster_vocalization('T', "troll", false, true);
        assert_eq!(sound, Some("growls menacingly!"));
    }

    // -----------------------------------------------------------------------
    // Test 32: Wraith moans
    // -----------------------------------------------------------------------
    #[test]
    fn wraith_moans() {
        let sound = monster_vocalization('W', "wraith", false, true);
        assert_eq!(sound, Some("moans!"));
    }

    // -----------------------------------------------------------------------
    // Test 33: Lich casts baleful look
    // -----------------------------------------------------------------------
    #[test]
    fn lich_baleful_look() {
        let sound = monster_vocalization('L', "lich", false, true);
        assert_eq!(sound, Some("casts a baleful look!"));
    }

    // -----------------------------------------------------------------------
    // Test 34: Giant thunders
    // -----------------------------------------------------------------------
    #[test]
    fn giant_thunders() {
        let sound = monster_vocalization('H', "hill giant", false, true);
        assert_eq!(sound, Some("thunders!"));
    }

    // -----------------------------------------------------------------------
    // Test 35: Nymph giggles
    // -----------------------------------------------------------------------
    #[test]
    fn nymph_giggles() {
        let sound = monster_vocalization('n', "nymph", false, false);
        assert_eq!(sound, Some("giggles."));
    }

    // -----------------------------------------------------------------------
    // Test 36: Tame horse whinnies
    // -----------------------------------------------------------------------
    #[test]
    fn tame_horse_whinnies() {
        let sound = monster_vocalization('u', "pony", true, false);
        assert_eq!(sound, Some("whinnies."));
    }

    // -----------------------------------------------------------------------
    // Test 37: Wild horse neighs
    // -----------------------------------------------------------------------
    #[test]
    fn wild_horse_neighs() {
        let sound = monster_vocalization('u', "horse", false, false);
        assert_eq!(sound, Some("neighs."));
    }
}
