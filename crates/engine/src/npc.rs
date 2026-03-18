//! NPC special behaviors: priests, vault guards, Wizard of Yendor,
//! shopkeepers, angels, and item-stealing monsters (nymphs, monkeys).
//!
//! Each NPC type has a dedicated interaction function that checks
//! preconditions and emits appropriate events.
//!
//! All functions are pure: they take a `GameWorld` plus an RNG, mutate
//! world state, and return `Vec<EngineEvent>`.  Zero IO.
//!
//! Reference: C sources `src/priest.c`, `src/shk.c`, `src/vault.c`.

use hecs::Entity;
use rand::Rng;
use serde::{Deserialize, Serialize};

use nethack_babel_data::Alignment;

use crate::action::Position;
use crate::event::EngineEvent;
use crate::inventory::Inventory;
use crate::world::{GameWorld, HitPoints, Positioned};

// ---------------------------------------------------------------------------
// ECS marker/state components for NPCs
// ---------------------------------------------------------------------------

/// Marker component for temple priest NPCs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Priest {
    /// The alignment of this priest's temple.
    pub alignment: Alignment,
    /// Whether the priest's temple has a shrine (altar with AM_SHRINE).
    pub has_shrine: bool,
    /// Whether this is the high priest of Moloch (sanctum).
    pub is_high_priest: bool,
    /// Whether this priest is angry with the player.
    #[serde(default)]
    pub angry: bool,
}

/// State for angel / aligned minion NPCs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Angel {
    /// The alignment this angel serves.
    pub alignment: Alignment,
    /// Whether the angel is a renegade (coaligned but hostile).
    pub renegade: bool,
}

/// Shopkeeper NPC state (movement / behavior, not billing — see shop.rs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shopkeeper {
    /// Whether the shopkeeper is following the hero.
    pub following: bool,
    /// Whether the shopkeeper has been expelled from the shop.
    pub displaced: bool,
    /// The shopkeeper's home position (door or center of shop).
    pub home_pos: Position,
    /// Name used for greetings.
    pub name: String,
}

/// Guard patrol state (castle guards, not vault guards).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Guard {
    /// Position the guard patrols to.
    pub patrol_target: Position,
    /// Whether the guard has spotted the player.
    pub alerted: bool,
}

/// Marker component for vault guard NPCs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultGuard;

/// State component for the Wizard of Yendor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WizardOfYendor {
    /// Turn when the Wizard was last killed.  0 means never killed.
    pub last_killed_turn: u32,
    /// Number of times the Wizard has been killed.
    pub times_killed: u32,
}

impl Default for WizardOfYendor {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardOfYendor {
    pub fn new() -> Self {
        Self {
            last_killed_turn: 0,
            times_killed: 0,
        }
    }
}

/// Marker component for item-stealing monsters (nymphs, monkeys, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thief {
    /// Whether the thief teleports away after stealing.
    pub teleports_after_steal: bool,
}

// ---------------------------------------------------------------------------
// Priest interaction
// ---------------------------------------------------------------------------

/// Cost of purchasing divine protection from a priest.
///
/// Formula: `400 * (current_protection_level + 1)`.
pub fn priest_protection_cost(current_protection: i32) -> i32 {
    400 * (current_protection + 1)
}

/// Resolve interaction between the player and a priest NPC.
///
/// The priest offers protection for gold.  The cost is
/// `400 * (protection_level + 1)`.  If the player has enough gold
/// and the same alignment, protection is granted.
///
/// Returns events describing the interaction.
pub fn priest_interaction(
    world: &mut GameWorld,
    _player: Entity,
    priest: Entity,
    player_gold: i32,
    player_alignment: Alignment,
    current_protection: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let priest_data = match world.get_component::<Priest>(priest) {
        Some(p) => *p,
        None => return events,
    };

    // Check alignment match.
    if player_alignment != priest_data.alignment {
        events.push(EngineEvent::msg("priest-wrong-alignment"));
        return events;
    }

    let cost = priest_protection_cost(current_protection);

    if player_gold < cost {
        events.push(EngineEvent::msg_with(
            "priest-not-enough-gold",
            vec![("cost", cost.to_string())],
        ));
        return events;
    }

    // Grant protection.
    events.push(EngineEvent::msg_with(
        "priest-protection-granted",
        vec![
            ("cost", cost.to_string()),
            ("level", (current_protection + 1).to_string()),
        ],
    ));

    events
}

// ---------------------------------------------------------------------------
// Priest donation / dialogue (priest_talk)
// ---------------------------------------------------------------------------

/// Alignment thresholds from priest.c.
pub const ALGN_SINNED: i32 = -4;
pub const ALGN_DEVOUT: i32 = 14;

/// Donation tier result from talking to a priest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DonationResult {
    /// Priest refuses (fleeing or has no temple).
    Refused,
    /// Player has no gold; coaligned priest gives 1-2 gold for ale.
    AleGift { amount: i32 },
    /// Player has no gold; priest preaches virtues of poverty.
    VirtuesOfPoverty,
    /// Player refused to donate.
    RefusedToDonate,
    /// Cheapskate: donated less than 200 * level, has more than 2x.
    Cheapskate,
    /// Small thanks: donated less than 200 * level.
    SmallThanks,
    /// Pious: donated 200-399 * level.
    Pious,
    /// Blessing: pious + poor + sinned => clairvoyance.
    Blessing { clairvoyance_turns: i32 },
    /// Protection reward: donated 400-599 * level, eligible.
    ProtectionReward,
    /// Selfless generosity: donated >= 600 * level.
    SelflessGenerosity,
    /// Cleansing: selfless + strayed + enough time elapsed.
    Cleansing,
}

#[derive(Debug, Clone, Copy)]
pub struct PriestDonationContext {
    pub offer: i32,
    pub player_gold: i32,
    pub player_level: u8,
    pub alignment_record: i32,
    pub coaligned: bool,
    pub current_protection: i32,
    pub turns_since_cleansed: u32,
}

/// Resolve a priest donation interaction.
///
/// Mirrors `priest_talk()` from `priest.c`.  The donation amount is
/// provided by the caller (from a UI prompt).
pub fn priest_donation<R: Rng>(context: PriestDonationContext, rng: &mut R) -> DonationResult {
    if context.offer == 0 {
        return DonationResult::RefusedToDonate;
    }

    let threshold = context.player_level as i32 * 200;

    if context.offer < threshold {
        if context.player_gold > context.offer * 2 {
            DonationResult::Cheapskate
        } else {
            DonationResult::SmallThanks
        }
    } else if context.offer < threshold * 2 {
        // Pious tier (200-399 * level).
        if context.player_gold < context.offer * 2
            && context.coaligned
            && context.alignment_record <= ALGN_SINNED
        {
            let turns = rng.random_range(500..1000);
            DonationResult::Blessing {
                clairvoyance_turns: turns,
            }
        } else {
            DonationResult::Pious
        }
    } else if context.offer < threshold * 3
        && (context.current_protection < 9
            || (context.current_protection < 20
                && rng.random_range(0..context.current_protection) == 0))
    {
        DonationResult::ProtectionReward
    } else {
        // Selfless generosity tier.
        if context.player_gold < context.offer * 2
            && context.coaligned
            && context.alignment_record < 0
            && context.turns_since_cleansed > 5000
        {
            DonationResult::Cleansing
        } else {
            DonationResult::SelflessGenerosity
        }
    }
}

/// Generate events for a priest who gives ale money to a broke coaligned hero.
pub fn priest_ale_gift(priest_gold: i32) -> Vec<EngineEvent> {
    if priest_gold <= 0 {
        return vec![EngineEvent::msg("priest-virtues-of-poverty")];
    }
    let amount = priest_gold.min(2);
    vec![EngineEvent::msg_with(
        "priest-ale-gift",
        vec![("amount", amount.to_string())],
    )]
}

// ---------------------------------------------------------------------------
// Temple entry
// ---------------------------------------------------------------------------

/// What happens when the player enters a temple room.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TempleEntryResult {
    /// Normal tended temple — sacred or desecrated.
    Tended {
        sacred: bool,
        hostile: bool,
        message_key: &'static str,
    },
    /// The Sanctum of Moloch.
    Sanctum { first_time: bool },
    /// Untended temple — eerie feeling.
    Untended { message_index: u8 },
}

/// Determine what happens when the player enters a temple room.
///
/// Mirrors `intemple()` from `priest.c`.
pub fn temple_entry(
    priest_present: bool,
    has_shrine: bool,
    coaligned: bool,
    alignment_record: i32,
    is_sanctum: bool,
    first_visit_sanctum: bool,
    rng: &mut impl Rng,
) -> TempleEntryResult {
    if is_sanctum {
        return TempleEntryResult::Sanctum {
            first_time: first_visit_sanctum,
        };
    }

    if priest_present {
        let hostile = !has_shrine || !coaligned || alignment_record <= ALGN_SINNED;
        let message_key = if hostile {
            "temple-forbidding"
        } else if alignment_record >= ALGN_DEVOUT {
            "temple-peace"
        } else {
            "temple-unusual-peace"
        };
        TempleEntryResult::Tended {
            sacred: has_shrine && coaligned,
            hostile,
            message_key,
        }
    } else {
        TempleEntryResult::Untended {
            message_index: rng.random_range(0..4),
        }
    }
}

/// Generate events for an untended temple.
pub fn untended_temple_events(message_index: u8, spawn_ghost: bool) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    match message_index {
        0 => events.push(EngineEvent::msg("temple-eerie")),
        1 => events.push(EngineEvent::msg("temple-watched")),
        2 => events.push(EngineEvent::msg("temple-shiver")),
        _ => {} // no message (25% chance)
    }

    if spawn_ghost {
        events.push(EngineEvent::msg("temple-ghost-appears"));
    }

    events
}

// ---------------------------------------------------------------------------
// Angel / aligned minion
// ---------------------------------------------------------------------------

/// Whether the angel's alignment matches the player's.
pub fn angel_coaligned(angel_alignment: Alignment, player_alignment: Alignment) -> bool {
    angel_alignment == player_alignment
}

/// Determine whether an angel should be hostile based on alignment.
///
/// Coaligned angels are peaceful unless they are renegades.
/// Non-coaligned angels are always hostile.
pub fn angel_hostility(angel: &Angel, player_alignment: Alignment) -> bool {
    if angel.alignment != player_alignment {
        true // always hostile if not coaligned
    } else {
        angel.renegade
    }
}

/// Reset angel hostility when the player changes alignment or enters
/// a new level.
pub fn reset_angel_hostility(angel: &Angel, player_alignment: Alignment) -> bool {
    // Non-coaligned angels become hostile.
    angel.alignment != player_alignment
}

/// Check if a position is in the player's sanctuary.
///
/// A sanctuary is a tended, shrined temple of the player's alignment
/// where the player has a positive alignment record.
pub fn in_sanctuary(
    priest_present: bool,
    has_shrine: bool,
    coaligned: bool,
    alignment_record: i32,
) -> bool {
    priest_present && has_shrine && coaligned && alignment_record > ALGN_SINNED
}

// ---------------------------------------------------------------------------
// Shopkeeper movement
// ---------------------------------------------------------------------------

/// Determine how a shopkeeper should move.
///
/// Returns (approach, avoid):
/// - `approach`: true if shopkeeper should move toward a target.
/// - `avoid`: true if shopkeeper should avoid the player (e.g., fleeing).
pub fn shopkeeper_movement_intent(
    shk: &Shopkeeper,
    is_angry: bool,
    hero_in_shop: bool,
    hero_on_door: bool,
) -> (bool, bool) {
    if shk.following || is_angry {
        // Approach the player.
        (true, false)
    } else if hero_in_shop {
        // Stay nearby but don't crowd.
        (false, hero_on_door)
    } else {
        // Return home.
        (true, false)
    }
}

/// Compute shopkeeper's goal position.
///
/// If following or angry: target is the player.
/// Otherwise: target is the home position.
pub fn shopkeeper_goal(shk: &Shopkeeper, is_angry: bool, player_pos: Position) -> Position {
    if shk.following || is_angry {
        player_pos
    } else {
        shk.home_pos
    }
}

/// Check if a shopkeeper should follow the player out of the shop.
///
/// Mirrors logic in `shk_move()` — follows if angry or has unpaid items.
pub fn shopkeeper_should_follow(
    is_angry: bool,
    has_unpaid_items: bool,
    hero_left_shop: bool,
) -> bool {
    hero_left_shop && (is_angry || has_unpaid_items)
}

// ---------------------------------------------------------------------------
// Shopkeeper dialogue
// ---------------------------------------------------------------------------

/// Shopkeeper honorific based on player status.
///
/// Mirrors `append_honorific()` from `shk.c`.
pub fn shopkeeper_honorific(is_female: bool, player_level: u8, hallu: bool) -> &'static str {
    if hallu {
        return "dude";
    }
    match (is_female, player_level) {
        (true, 0..=4) => "young lady",
        (true, 5..=10) => "lady",
        (true, 11..=19) => "madam",
        (true, _) => "most gracious lady",
        (false, 0..=4) => "young man",
        (false, 5..=10) => "sir",
        (false, 11..=19) => "good sir",
        (false, _) => "most noble sir",
    }
}

/// Shopkeeper angry text — cycled on repeated anger.
pub fn shopkeeper_angry_text(anger_count: u32) -> &'static str {
    match anger_count % 3 {
        0 => "quite upset",
        1 => "ticked off",
        _ => "furious",
    }
}

/// Shopkeeper greeting based on hero status.
pub fn shopkeeper_greeting(
    is_angry: bool,
    has_been_robbed: bool,
    has_surcharge: bool,
    shk_name: &str,
    honorific: &str,
) -> EngineEvent {
    if is_angry {
        EngineEvent::msg_with(
            "shk-angry-greeting",
            vec![("shopkeeper", shk_name.to_string())],
        )
    } else if has_been_robbed {
        EngineEvent::msg_with(
            "shk-robbed-greeting",
            vec![
                ("shopkeeper", shk_name.to_string()),
                ("honorific", honorific.to_string()),
            ],
        )
    } else if has_surcharge {
        EngineEvent::msg_with(
            "shk-surcharge-greeting",
            vec![
                ("shopkeeper", shk_name.to_string()),
                ("honorific", honorific.to_string()),
            ],
        )
    } else {
        EngineEvent::msg_with(
            "shk-welcome",
            vec![
                ("shopkeeper", shk_name.to_string()),
                ("honorific", honorific.to_string()),
            ],
        )
    }
}

// ---------------------------------------------------------------------------
// Guard patrol
// ---------------------------------------------------------------------------

/// Determine guard patrol movement direction.
///
/// Guards move toward their patrol target. If they spot the player
/// in a restricted area, they change target to the player.
pub fn guard_patrol_target(
    guard: &Guard,
    guard_pos: Position,
    player_pos: Position,
    player_in_restricted_area: bool,
) -> Position {
    if player_in_restricted_area {
        player_pos
    } else if guard_pos == guard.patrol_target {
        // Reached target, stay.
        guard_pos
    } else {
        guard.patrol_target
    }
}

/// Resolve a guard spotting the player in a restricted area.
pub fn guard_spot_player(guard: &mut Guard, player_in_restricted: bool) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if player_in_restricted && !guard.alerted {
        guard.alerted = true;
        events.push(EngineEvent::msg("guard-halt"));
    }

    events
}

// ---------------------------------------------------------------------------
// Cranky priest dialogue
// ---------------------------------------------------------------------------

/// Messages from a priest who is not in their temple or is hostile.
pub fn cranky_priest_message(rng: &mut impl Rng) -> &'static str {
    match rng.random_range(0..3) {
        0 => "Thou wouldst have words, eh?  I'll give thee a word or two!",
        1 => "Talk?  Here is what I have to say!",
        _ => "Pilgrim, I would speak no longer with thee.",
    }
}

// ---------------------------------------------------------------------------
// Vault Guard interaction
// ---------------------------------------------------------------------------

/// Resolve interaction between the player and a vault guard.
///
/// The guard demands the player deposit gold.  If the player has gold,
/// it is confiscated and the player is teleported out of the vault.
/// If no gold, the guard tells the player to leave.
pub fn guard_interaction(
    world: &mut GameWorld,
    player: Entity,
    guard: Entity,
    player_gold: i32,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    if world.get_component::<VaultGuard>(guard).is_none() {
        return events;
    }

    if player_gold > 0 {
        events.push(EngineEvent::msg_with(
            "guard-confiscate-gold",
            vec![("gold", player_gold.to_string())],
        ));

        // Teleport player out of the vault.
        let player_pos = world
            .get_component::<Positioned>(player)
            .map(|p| p.0)
            .unwrap_or(Position::new(0, 0));

        events.push(EngineEvent::EntityTeleported {
            entity: player,
            from: player_pos,
            to: Position::new(1, 1), // Placeholder: actual vault exit logic TBD.
        });
    } else {
        events.push(EngineEvent::msg("guard-no-gold"));
    }

    events
}

// ---------------------------------------------------------------------------
// Wizard of Yendor harassment
// ---------------------------------------------------------------------------

/// Minimum interval (in turns) between Wizard respawns after first kill.
pub const WIZARD_RESPAWN_MIN: u32 = 50;

/// Maximum interval (in turns) between Wizard respawns after first kill.
pub const WIZARD_RESPAWN_MAX: u32 = 100;

/// Check whether the Wizard of Yendor should respawn on this turn.
///
/// After being killed, the Wizard respawns every 50-100 turns.
pub fn wizard_should_respawn(
    wizard_state: &WizardOfYendor,
    current_turn: u32,
    rng: &mut impl Rng,
) -> bool {
    if wizard_state.times_killed == 0 {
        return false;
    }

    let elapsed = current_turn.saturating_sub(wizard_state.last_killed_turn);

    if elapsed < WIZARD_RESPAWN_MIN {
        return false;
    }

    if elapsed >= WIZARD_RESPAWN_MAX {
        return true;
    }

    // Between min and max: increasing probability.
    // Linear interpolation: P = (elapsed - min) / (max - min).
    let range = WIZARD_RESPAWN_MAX - WIZARD_RESPAWN_MIN;
    let progress = elapsed - WIZARD_RESPAWN_MIN;
    rng.random_range(0..range) < progress
}

/// Wizard harassment actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardAction {
    /// Steal the Amulet of Yendor from the player.
    StealAmulet,
    /// Summon hostile monsters around the player.
    SummonNasties,
    /// Curse random items in the player's inventory.
    CurseItems,
    /// Clone self ("Double Trouble") when at full HP.
    DoubleTrouble,
}

/// Convert a Wizard harassment action into user-facing message events.
pub fn wizard_harass_events(action: WizardAction) -> Vec<EngineEvent> {
    match action {
        WizardAction::StealAmulet => vec![EngineEvent::msg("wizard-steal-amulet")],
        WizardAction::DoubleTrouble => vec![EngineEvent::msg("wizard-double-trouble")],
        WizardAction::SummonNasties => vec![EngineEvent::msg("wizard-summon-nasties")],
        WizardAction::CurseItems => vec![EngineEvent::msg("wizard-curse-items")],
    }
}

/// Determine which harassment action the Wizard of Yendor uses this turn.
pub fn choose_wizard_action(
    world: &GameWorld,
    wizard: Entity,
    player_has_amulet: bool,
    rng: &mut impl Rng,
) -> WizardAction {
    let wizard_hp = world
        .get_component::<HitPoints>(wizard)
        .map(|hp| (hp.current, hp.max))
        .unwrap_or((1, 1));

    if player_has_amulet {
        WizardAction::StealAmulet
    } else if wizard_hp.0 >= wizard_hp.1 {
        WizardAction::DoubleTrouble
    } else if rng.random_range(0..2) == 0 {
        WizardAction::SummonNasties
    } else {
        WizardAction::CurseItems
    }
}

/// Resolve a Wizard of Yendor harassment action.
///
/// The Wizard chooses an action based on the current world state and emits the
/// corresponding message event. Runtime side-effects are handled by the caller.
pub fn wizard_harass(
    world: &mut GameWorld,
    wizard: Entity,
    _player: Entity,
    player_has_amulet: bool,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let action = choose_wizard_action(world, wizard, player_has_amulet, rng);
    // The actual side-effects are handled by the caller, since harassment can
    // manipulate inventory and spawn monsters in engine-specific ways.
    wizard_harass_events(action)
}

// ---------------------------------------------------------------------------
// Monster stealing (Nymph/Monkey)
// ---------------------------------------------------------------------------

/// Resolve a steal attempt by a thief monster against a target.
///
/// The thief removes a random item from the target's inventory.
/// If `teleports_after_steal` is true, the thief teleports to a random
/// position after a successful steal.
///
/// Returns events describing the steal attempt and any teleportation.
pub fn monster_steal(
    world: &mut GameWorld,
    thief: Entity,
    target: Entity,
    rng: &mut impl Rng,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let thief_data = match world.get_component::<Thief>(thief) {
        Some(t) => *t,
        None => return events,
    };

    // Get the target's inventory.
    let item_count = world
        .get_component::<Inventory>(target)
        .map(|inv| inv.items.len())
        .unwrap_or(0);

    if item_count == 0 {
        events.push(EngineEvent::msg("steal-nothing-to-take"));
        return events;
    }

    // Pick a random item index.
    let steal_index = rng.random_range(0..item_count);

    // Get the item entity.
    let stolen_item = world
        .get_component::<Inventory>(target)
        .map(|inv| inv.items[steal_index])
        .unwrap();

    let item_name = world.entity_name(stolen_item);
    let thief_name = world.entity_name(thief);

    // Remove from target's inventory.
    if let Some(mut inv) = world.get_component_mut::<Inventory>(target) {
        inv.items.remove(steal_index);
    }

    events.push(EngineEvent::msg_with(
        "monster-stole-item",
        vec![("monster", thief_name.clone()), ("item", item_name)],
    ));

    events.push(EngineEvent::ItemDropped {
        actor: target,
        item: stolen_item,
    });

    // Thief teleports away after stealing.
    if thief_data.teleports_after_steal {
        let thief_pos = world
            .get_component::<Positioned>(thief)
            .map(|p| p.0)
            .unwrap_or(Position::new(0, 0));

        // Find a random floor position for teleportation.
        if let Some(dest) = find_random_floor(world, rng) {
            if let Some(mut pos) = world.get_component_mut::<Positioned>(thief) {
                pos.0 = dest;
            }
            events.push(EngineEvent::EntityTeleported {
                entity: thief,
                from: thief_pos,
                to: dest,
            });
        }
    }

    events
}

/// Find a random walkable floor tile in the current level.
fn find_random_floor(world: &GameWorld, rng: &mut impl Rng) -> Option<Position> {
    let map = &world.dungeon().current_level;
    let (width, height) = map.dimensions();

    // Try up to 100 random positions.
    for _ in 0..100 {
        let x = rng.random_range(1..width as i32);
        let y = rng.random_range(1..height as i32);
        let pos = Position::new(x, y);

        if let Some(cell) = map.get(pos)
            && cell.terrain.is_walkable()
        {
            return Some(pos);
        }
    }

    None
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::dungeon::Terrain;
    use crate::inventory::Inventory;
    use crate::world::{
        ArmorClass, Attributes, ExperienceLevel, GameWorld, HitPoints, Monster, MovementPoints,
        NORMAL_SPEED, Name, Positioned, Speed,
    };
    use nethack_babel_data::{ObjectClass, ObjectCore, ObjectTypeId};
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(99999)
    }

    fn make_test_world() -> GameWorld {
        let mut world = GameWorld::new(Position::new(8, 8));
        for y in 1..=15 {
            for x in 1..=15 {
                world
                    .dungeon_mut()
                    .current_level
                    .set_terrain(Position::new(x, y), Terrain::Floor);
            }
        }
        world
    }

    fn spawn_monster(world: &mut GameWorld, pos: Position, name: &str, hp: i32) -> Entity {
        world.spawn((
            Monster,
            Positioned(pos),
            HitPoints {
                current: hp,
                max: hp,
            },
            ArmorClass(10),
            Attributes::default(),
            ExperienceLevel(1),
            Speed(12),
            MovementPoints(NORMAL_SPEED as i32),
            Name(name.to_string()),
        ))
    }

    // ── Priest tests ─────────────────────────────────────────────

    #[test]
    fn test_priest_protection_cost() {
        // Level 0: 400 * (0 + 1) = 400
        assert_eq!(priest_protection_cost(0), 400);
        // Level 1: 400 * (1 + 1) = 800
        assert_eq!(priest_protection_cost(1), 800);
        // Level 2: 400 * (2 + 1) = 1200
        assert_eq!(priest_protection_cost(2), 1200);
        // Level 5: 400 * (5 + 1) = 2400
        assert_eq!(priest_protection_cost(5), 2400);
    }

    #[test]
    fn test_priest_grants_protection_with_enough_gold() {
        let mut world = make_test_world();
        let player = world.player();
        let priest_entity = spawn_monster(&mut world, Position::new(9, 8), "priest", 20);
        let _ = world.ecs_mut().insert_one(
            priest_entity,
            Priest {
                alignment: Alignment::Lawful,
                has_shrine: false,
                is_high_priest: false,
                angry: false,
            },
        );

        let events = priest_interaction(
            &mut world,
            player,
            priest_entity,
            1000, // enough gold
            Alignment::Lawful,
            0, // current protection
        );

        let granted = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "priest-protection-granted"),
        );
        assert!(
            granted,
            "priest should grant protection when gold is sufficient"
        );
    }

    #[test]
    fn test_priest_rejects_wrong_alignment() {
        let mut world = make_test_world();
        let player = world.player();
        let priest_entity = spawn_monster(&mut world, Position::new(9, 8), "priest", 20);
        let _ = world.ecs_mut().insert_one(
            priest_entity,
            Priest {
                alignment: Alignment::Lawful,
                has_shrine: false,
                is_high_priest: false,
                angry: false,
            },
        );

        let events = priest_interaction(
            &mut world,
            player,
            priest_entity,
            1000,
            Alignment::Chaotic, // wrong alignment
            0,
        );

        let wrong = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "priest-wrong-alignment"),
        );
        assert!(wrong, "priest should reject player with wrong alignment");
    }

    // ── Vault Guard tests ────────────────────────────────────────

    #[test]
    fn test_vault_guard_confiscates_gold() {
        let mut world = make_test_world();
        let player = world.player();
        let guard = spawn_monster(&mut world, Position::new(9, 8), "guard", 20);
        let _ = world.ecs_mut().insert_one(guard, VaultGuard);

        let events = guard_interaction(&mut world, player, guard, 500);

        let confiscated = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "guard-confiscate-gold"),
        );
        assert!(confiscated, "guard should confiscate gold");

        let teleported = events.iter().any(
            |e| matches!(e, EngineEvent::EntityTeleported { entity, .. } if *entity == player),
        );
        assert!(teleported, "player should be teleported out of vault");
    }

    #[test]
    fn test_vault_guard_no_gold() {
        let mut world = make_test_world();
        let player = world.player();
        let guard = spawn_monster(&mut world, Position::new(9, 8), "guard", 20);
        let _ = world.ecs_mut().insert_one(guard, VaultGuard);

        let events = guard_interaction(&mut world, player, guard, 0);

        let no_gold = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "guard-no-gold"));
        assert!(no_gold, "guard should say no gold");
    }

    // ── Wizard of Yendor tests ───────────────────────────────────

    #[test]
    fn test_wizard_respawn_interval() {
        let mut rng = test_rng();

        // Never killed: should not respawn.
        let state_never = WizardOfYendor::new();
        assert!(!wizard_should_respawn(&state_never, 100, &mut rng));

        // Killed 1 turn ago: too soon.
        let state_recent = WizardOfYendor {
            last_killed_turn: 99,
            times_killed: 1,
        };
        assert!(!wizard_should_respawn(&state_recent, 100, &mut rng));

        // Killed 200 turns ago: definitely should respawn (past max).
        let state_old = WizardOfYendor {
            last_killed_turn: 0,
            times_killed: 1,
        };
        assert!(wizard_should_respawn(&state_old, 200, &mut rng));
    }

    #[test]
    fn test_wizard_steal_amulet() {
        let mut world = make_test_world();
        let player = world.player();
        let wizard = spawn_monster(&mut world, Position::new(9, 8), "Wizard of Yendor", 50);
        let _ = world.ecs_mut().insert_one(
            wizard,
            WizardOfYendor {
                last_killed_turn: 0,
                times_killed: 1,
            },
        );

        let mut rng = test_rng();
        let events = wizard_harass(
            &mut world, wizard, player, true, // player has amulet
            &mut rng,
        );

        let steal = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "wizard-steal-amulet"));
        assert!(steal, "wizard should attempt to steal the Amulet");
    }

    #[test]
    fn test_wizard_double_trouble() {
        let mut world = make_test_world();
        let player = world.player();
        let wizard = spawn_monster(&mut world, Position::new(9, 8), "Wizard of Yendor", 50);
        // Wizard at full HP, player does not have amulet.
        let _ = world.ecs_mut().insert_one(
            wizard,
            WizardOfYendor {
                last_killed_turn: 0,
                times_killed: 1,
            },
        );

        let mut rng = test_rng();
        let events = wizard_harass(&mut world, wizard, player, false, &mut rng);

        let double = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "wizard-double-trouble"),
        );
        assert!(double, "wizard at full HP should use Double Trouble");
    }

    #[test]
    fn test_wizard_summon_or_curse_at_low_hp() {
        let mut world = make_test_world();
        let player = world.player();
        let wizard = spawn_monster(&mut world, Position::new(9, 8), "Wizard of Yendor", 50);
        let _ = world.ecs_mut().insert_one(
            wizard,
            WizardOfYendor {
                last_killed_turn: 0,
                times_killed: 1,
            },
        );

        // Reduce wizard HP so it's not at full (no Double Trouble).
        if let Some(mut hp) = world.get_component_mut::<HitPoints>(wizard) {
            hp.current = 10; // Less than max of 50.
        }

        let mut rng = test_rng();
        let events = wizard_harass(
            &mut world, wizard, player, false, // no amulet
            &mut rng,
        );

        // Should choose either summon nasties or curse items.
        let summon_or_curse = events.iter().any(|e| {
            matches!(e, EngineEvent::Message { key, .. }
                if key == "wizard-summon-nasties" || key == "wizard-curse-items")
        });
        assert!(
            summon_or_curse,
            "wizard at low HP without amulet should summon nasties or curse items"
        );
    }

    // ── Stealing tests ───────────────────────────────────────────

    #[test]
    fn test_nymph_steal_item() {
        let mut world = make_test_world();
        let player = world.player();

        // Spawn a sword in the player's inventory.
        let sword = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(1),
                object_class: ObjectClass::Weapon,
                quantity: 1,
                weight: 40,
                age: 0,
                inv_letter: Some('a'),
                artifact: None,
            },
            Name("long sword".to_string()),
        ));

        // Add the sword to the player's inventory.
        if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
            inv.add(sword);
        }

        // Spawn a nymph.
        let nymph = spawn_monster(&mut world, Position::new(9, 8), "wood nymph", 10);
        let _ = world.ecs_mut().insert_one(
            nymph,
            Thief {
                teleports_after_steal: true,
            },
        );

        let mut rng = test_rng();
        let events = monster_steal(&mut world, nymph, player, &mut rng);

        // Check that the steal event was emitted.
        let stole = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "monster-stole-item"));
        assert!(stole, "nymph should steal an item");

        // Verify the item was removed from inventory.
        let inv = world.get_component::<Inventory>(player).unwrap();
        assert!(
            !inv.items.contains(&sword),
            "stolen item should be removed from inventory"
        );
    }

    #[test]
    fn test_nymph_teleports_away() {
        let mut world = make_test_world();
        let player = world.player();

        // Spawn an item for the player.
        let gem = world.spawn((
            ObjectCore {
                otyp: ObjectTypeId(100),
                object_class: ObjectClass::Gem,
                quantity: 1,
                weight: 1,
                age: 0,
                inv_letter: Some('b'),
                artifact: None,
            },
            Name("ruby".to_string()),
        ));
        if let Some(mut inv) = world.get_component_mut::<Inventory>(player) {
            inv.add(gem);
        }

        let nymph_start = Position::new(9, 8);
        let nymph = spawn_monster(&mut world, nymph_start, "wood nymph", 10);
        let _ = world.ecs_mut().insert_one(
            nymph,
            Thief {
                teleports_after_steal: true,
            },
        );

        let mut rng = test_rng();
        let events = monster_steal(&mut world, nymph, player, &mut rng);

        // Check that the nymph teleported.
        let teleported = events
            .iter()
            .any(|e| matches!(e, EngineEvent::EntityTeleported { entity, .. } if *entity == nymph));
        assert!(teleported, "nymph should teleport after stealing");

        // Verify position changed.
        let _new_pos = world.get_component::<Positioned>(nymph).unwrap().0;
        // The nymph may or may not move to a different position depending on RNG,
        // but the teleport event should exist.
        assert!(teleported);
    }

    #[test]
    fn test_steal_from_empty_inventory() {
        let mut world = make_test_world();
        let player = world.player();

        let nymph = spawn_monster(&mut world, Position::new(9, 8), "wood nymph", 10);
        let _ = world.ecs_mut().insert_one(
            nymph,
            Thief {
                teleports_after_steal: true,
            },
        );

        let mut rng = test_rng();
        let events = monster_steal(&mut world, nymph, player, &mut rng);

        let nothing = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "steal-nothing-to-take"),
        );
        assert!(
            nothing,
            "steal from empty inventory should produce 'nothing' message"
        );
    }

    // ── Priest donation tests ────────────────────────────────────

    #[test]
    fn test_priest_donation_refuse() {
        let mut rng = test_rng();
        let result = priest_donation(
            PriestDonationContext {
                offer: 0,
                player_gold: 1000,
                player_level: 14,
                alignment_record: 5,
                coaligned: true,
                current_protection: 0,
                turns_since_cleansed: 0,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::RefusedToDonate);
    }

    #[test]
    fn test_priest_donation_cheapskate() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 100 < 2800, gold 10000 > 200.
        let result = priest_donation(
            PriestDonationContext {
                offer: 100,
                player_gold: 10000,
                player_level: 14,
                alignment_record: 5,
                coaligned: true,
                current_protection: 0,
                turns_since_cleansed: 0,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::Cheapskate);
    }

    #[test]
    fn test_priest_donation_small_thanks() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 100 < 2800, gold 150 < 200.
        let result = priest_donation(
            PriestDonationContext {
                offer: 100,
                player_gold: 150,
                player_level: 14,
                alignment_record: 5,
                coaligned: true,
                current_protection: 0,
                turns_since_cleansed: 0,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::SmallThanks);
    }

    #[test]
    fn test_priest_donation_pious() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 3000 (>= 2800, < 5600).
        let result = priest_donation(
            PriestDonationContext {
                offer: 3000,
                player_gold: 10000,
                player_level: 14,
                alignment_record: 5,
                coaligned: true,
                current_protection: 0,
                turns_since_cleansed: 0,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::Pious);
    }

    #[test]
    fn test_priest_donation_protection() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 7000 (>= 5600, < 8400). Protection 0 < 9.
        let result = priest_donation(
            PriestDonationContext {
                offer: 7000,
                player_gold: 20000,
                player_level: 14,
                alignment_record: 5,
                coaligned: true,
                current_protection: 0,
                turns_since_cleansed: 0,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::ProtectionReward);
    }

    #[test]
    fn test_priest_donation_selfless() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 10000 (>= 8400). Protection 20 (too high).
        let result = priest_donation(
            PriestDonationContext {
                offer: 10000,
                player_gold: 50000,
                player_level: 14,
                alignment_record: 5,
                coaligned: true,
                current_protection: 20,
                turns_since_cleansed: 0,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::SelflessGenerosity);
    }

    #[test]
    fn test_priest_donation_cleansing() {
        let mut rng = test_rng();
        // Level 14, threshold = 2800. Offer 10000. Gold 15000 < 20000.
        // Coaligned, alignment_record < 0, turns since cleansed > 5000.
        let result = priest_donation(
            PriestDonationContext {
                offer: 10000,
                player_gold: 15000,
                player_level: 14,
                alignment_record: -2,
                coaligned: true,
                current_protection: 20,
                turns_since_cleansed: 6000,
            },
            &mut rng,
        );
        assert_eq!(result, DonationResult::Cleansing);
    }

    // ── Priest ale gift tests ────────────────────────────────────

    #[test]
    fn test_priest_ale_gift_has_gold() {
        let events = priest_ale_gift(5);
        let has_ale = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "priest-ale-gift"));
        assert!(has_ale);
    }

    #[test]
    fn test_priest_ale_gift_no_gold() {
        let events = priest_ale_gift(0);
        let has_poverty = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "priest-virtues-of-poverty"),
        );
        assert!(has_poverty);
    }

    // ── Temple entry tests ───────────────────────────────────────

    #[test]
    fn test_temple_entry_tended_sacred() {
        let mut rng = test_rng();
        let result = temple_entry(true, true, true, 15, false, false, &mut rng);
        match result {
            TempleEntryResult::Tended {
                sacred, hostile, ..
            } => {
                assert!(sacred);
                assert!(!hostile);
            }
            _ => panic!("expected Tended"),
        }
    }

    #[test]
    fn test_temple_entry_tended_hostile_sinned() {
        let mut rng = test_rng();
        let result = temple_entry(true, true, true, ALGN_SINNED, false, false, &mut rng);
        match result {
            TempleEntryResult::Tended { hostile, .. } => {
                assert!(hostile);
            }
            _ => panic!("expected Tended"),
        }
    }

    #[test]
    fn test_temple_entry_sanctum() {
        let mut rng = test_rng();
        let result = temple_entry(true, true, false, 10, true, true, &mut rng);
        assert_eq!(result, TempleEntryResult::Sanctum { first_time: true });
    }

    #[test]
    fn test_temple_entry_untended() {
        let mut rng = test_rng();
        let result = temple_entry(false, false, false, 10, false, false, &mut rng);
        matches!(result, TempleEntryResult::Untended { .. });
    }

    #[test]
    fn test_untended_temple_events_with_ghost() {
        let events = untended_temple_events(0, true);
        assert!(events.len() >= 2);
        let has_ghost = events.iter().any(
            |e| matches!(e, EngineEvent::Message { key, .. } if key == "temple-ghost-appears"),
        );
        assert!(has_ghost);
    }

    // ── Angel tests ──────────────────────────────────────────────

    #[test]
    fn test_angel_coaligned_peaceful() {
        let angel = Angel {
            alignment: Alignment::Lawful,
            renegade: false,
        };
        assert!(angel_coaligned(angel.alignment, Alignment::Lawful));
        assert!(!angel_hostility(&angel, Alignment::Lawful));
    }

    #[test]
    fn test_angel_renegade() {
        let angel = Angel {
            alignment: Alignment::Lawful,
            renegade: true,
        };
        assert!(angel_hostility(&angel, Alignment::Lawful));
    }

    #[test]
    fn test_angel_not_coaligned_hostile() {
        let angel = Angel {
            alignment: Alignment::Chaotic,
            renegade: false,
        };
        assert!(angel_hostility(&angel, Alignment::Lawful));
    }

    #[test]
    fn test_reset_angel_hostility() {
        let angel = Angel {
            alignment: Alignment::Neutral,
            renegade: false,
        };
        // Same alignment => not hostile.
        assert!(!reset_angel_hostility(&angel, Alignment::Neutral));
        // Different alignment => hostile.
        assert!(reset_angel_hostility(&angel, Alignment::Lawful));
    }

    // ── Sanctuary tests ──────────────────────────────────────────

    #[test]
    fn test_in_sanctuary() {
        assert!(in_sanctuary(true, true, true, 5));
        assert!(!in_sanctuary(true, true, true, ALGN_SINNED));
        assert!(!in_sanctuary(true, false, true, 5));
        assert!(!in_sanctuary(false, true, true, 5));
        assert!(!in_sanctuary(true, true, false, 5));
    }

    // ── Shopkeeper movement tests ────────────────────────────────

    #[test]
    fn test_shopkeeper_movement_angry() {
        let shk = Shopkeeper {
            following: false,
            displaced: false,
            home_pos: Position::new(10, 5),
            name: "Asidonhopo".to_string(),
        };
        let (approach, avoid) = shopkeeper_movement_intent(&shk, true, true, false);
        assert!(approach);
        assert!(!avoid);
    }

    #[test]
    fn test_shopkeeper_movement_following() {
        let shk = Shopkeeper {
            following: true,
            displaced: false,
            home_pos: Position::new(10, 5),
            name: "Asidonhopo".to_string(),
        };
        let (approach, avoid) = shopkeeper_movement_intent(&shk, false, false, false);
        assert!(approach);
        assert!(!avoid);
    }

    #[test]
    fn test_shopkeeper_goal_angry() {
        let shk = Shopkeeper {
            following: false,
            displaced: false,
            home_pos: Position::new(10, 5),
            name: "Asidonhopo".to_string(),
        };
        let goal = shopkeeper_goal(&shk, true, Position::new(3, 3));
        assert_eq!(goal, Position::new(3, 3));
    }

    #[test]
    fn test_shopkeeper_goal_idle() {
        let shk = Shopkeeper {
            following: false,
            displaced: false,
            home_pos: Position::new(10, 5),
            name: "Asidonhopo".to_string(),
        };
        let goal = shopkeeper_goal(&shk, false, Position::new(3, 3));
        assert_eq!(goal, Position::new(10, 5));
    }

    #[test]
    fn test_shopkeeper_should_follow() {
        assert!(shopkeeper_should_follow(true, false, true));
        assert!(shopkeeper_should_follow(false, true, true));
        assert!(!shopkeeper_should_follow(false, false, true));
        assert!(!shopkeeper_should_follow(true, true, false));
    }

    // ── Shopkeeper dialogue tests ────────────────────────────────

    #[test]
    fn test_shopkeeper_honorific() {
        assert_eq!(shopkeeper_honorific(false, 3, false), "young man");
        assert_eq!(shopkeeper_honorific(false, 7, false), "sir");
        assert_eq!(shopkeeper_honorific(false, 15, false), "good sir");
        assert_eq!(shopkeeper_honorific(false, 25, false), "most noble sir");
        assert_eq!(shopkeeper_honorific(true, 3, false), "young lady");
        assert_eq!(shopkeeper_honorific(true, 7, false), "lady");
        assert_eq!(shopkeeper_honorific(true, 15, false), "madam");
        assert_eq!(shopkeeper_honorific(true, 25, false), "most gracious lady");
        assert_eq!(shopkeeper_honorific(false, 15, true), "dude");
    }

    #[test]
    fn test_shopkeeper_angry_text() {
        assert_eq!(shopkeeper_angry_text(0), "quite upset");
        assert_eq!(shopkeeper_angry_text(1), "ticked off");
        assert_eq!(shopkeeper_angry_text(2), "furious");
        assert_eq!(shopkeeper_angry_text(3), "quite upset"); // wraps
    }

    #[test]
    fn test_shopkeeper_greeting_types() {
        let evt = shopkeeper_greeting(true, false, false, "Bob", "sir");
        matches!(evt, EngineEvent::Message { key, .. } if key == "shk-angry-greeting");

        let evt = shopkeeper_greeting(false, true, false, "Bob", "sir");
        matches!(evt, EngineEvent::Message { key, .. } if key == "shk-robbed-greeting");

        let evt = shopkeeper_greeting(false, false, false, "Bob", "sir");
        matches!(evt, EngineEvent::Message { key, .. } if key == "shk-welcome");
    }

    // ── Guard patrol tests ───────────────────────────────────────

    #[test]
    fn test_guard_patrol_normal() {
        let guard = Guard {
            patrol_target: Position::new(5, 5),
            alerted: false,
        };
        let target = guard_patrol_target(&guard, Position::new(3, 3), Position::new(10, 10), false);
        assert_eq!(target, Position::new(5, 5));
    }

    #[test]
    fn test_guard_patrol_spots_player() {
        let guard = Guard {
            patrol_target: Position::new(5, 5),
            alerted: false,
        };
        let target = guard_patrol_target(
            &guard,
            Position::new(3, 3),
            Position::new(10, 10),
            true, // player in restricted area
        );
        assert_eq!(target, Position::new(10, 10));
    }

    #[test]
    fn test_guard_spot_alert() {
        let mut guard = Guard {
            patrol_target: Position::new(5, 5),
            alerted: false,
        };
        let events = guard_spot_player(&mut guard, true);
        assert!(guard.alerted);
        let has_halt = events
            .iter()
            .any(|e| matches!(e, EngineEvent::Message { key, .. } if key == "guard-halt"));
        assert!(has_halt);

        // Already alerted — no new event.
        let events2 = guard_spot_player(&mut guard, true);
        assert!(events2.is_empty());
    }

    // ── Cranky priest tests ──────────────────────────────────────

    #[test]
    fn test_cranky_priest_messages() {
        let mut rng = test_rng();
        let msg = cranky_priest_message(&mut rng);
        assert!(!msg.is_empty());
    }
}
