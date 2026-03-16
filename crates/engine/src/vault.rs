//! Vault guard system for NetHack Babel.
//!
//! Implements vault detection, guard spawning, and the guard's state machine
//! for interacting with players found inside gold vaults.
//!
//! Reference: C source `src/vault.c` (1,288 lines).

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::Position;
use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// Vault Guard state machine
// ---------------------------------------------------------------------------

/// Vault guard state machine phases.
///
/// The guard transitions through these states as it interacts with the
/// player who was found inside a vault.  This mirrors the behavior in
/// C's `gd_move()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaultGuardState {
    /// Guard has not yet noticed the player.
    Idle,
    /// Guard is approaching the vault.
    Approaching,
    /// Guard is asking for the player's name ("Who are you?").
    Asking,
    /// Guard is following the player to the exit.
    Escorting,
    /// Guard is collecting gold from the player.
    CollectingGold,
    /// Guard is done and leaving.
    Leaving,
}

/// Vault guard behavior data, attached as an ECS component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultGuardData {
    pub state: VaultGuardState,
    pub guard_name: String,
    pub gold_demanded: i32,
    pub turns_waiting: i32,
    pub player_said_name: Option<String>,
}

impl VaultGuardData {
    pub fn new(name: &str) -> Self {
        Self {
            state: VaultGuardState::Idle,
            guard_name: name.to_string(),
            gold_demanded: 0,
            turns_waiting: 0,
            player_said_name: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Guard events
// ---------------------------------------------------------------------------

/// Events emitted by the vault guard system each turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardEvent {
    /// Guard says something to the player.
    Message(String),
    /// Guard demands gold.
    DemandGold { amount: i32 },
    /// Guard confiscates gold from the player.
    ConfiscateGold { amount: i32 },
    /// Guard escorts the player toward the vault exit.
    Escort { direction: (i32, i32) },
    /// Guard disappears (leaves the level).
    Disappear,
    /// Guard transitions to a new state.
    StateChange { new_state: VaultGuardState },
}

// ---------------------------------------------------------------------------
// Guard name list (from C — ghost names used for vault guards too)
// ---------------------------------------------------------------------------

/// Guard name list.  In C, vault guards are named using `rndghostname()`
/// from `do_name.c`.  We use the same pool here.
const GUARD_NAMES: &[&str] = &[
    "Adri", "Andries", "Andreas", "Bert", "David", "Dirk",
    "Emile", "Frans", "Fred", "Greg", "Hether", "Jay",
    "John", "Jon", "Karnov", "Kay", "Kenny", "Kevin",
    "Maud", "Michiel", "Mike", "Peter", "Robert", "Ron",
    "Tom", "Wilmar", "Nick Danger", "Phoenix", "Jiro", "Mizue",
    "Stephan", "Lance Braccus", "Shadowhawk", "Murphy",
];

/// Pick a random guard name.
pub fn random_guard_name(rng: &mut impl Rng) -> &'static str {
    let idx = rng.random_range(0..GUARD_NAMES.len());
    GUARD_NAMES[idx]
}

// ---------------------------------------------------------------------------
// Vault room detection
// ---------------------------------------------------------------------------

/// Simplified vault room descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultRoom {
    /// Top-left corner of the vault.
    pub top_left: Position,
    /// Bottom-right corner of the vault.
    pub bottom_right: Position,
}

impl VaultRoom {
    /// Check whether a position is inside this vault room.
    pub fn contains(&self, pos: Position) -> bool {
        pos.x >= self.top_left.x
            && pos.x <= self.bottom_right.x
            && pos.y >= self.top_left.y
            && pos.y <= self.bottom_right.y
    }
}

/// Check if the player is inside any vault room.
///
/// Returns the index of the vault containing the player, if any.
/// A vault is a small room (typically 2x2) designated as a vault type.
pub fn player_in_vault(player_pos: Position, vaults: &[VaultRoom]) -> Option<usize> {
    vaults.iter().position(|v| v.contains(player_pos))
}

// ---------------------------------------------------------------------------
// Guard spawning
// ---------------------------------------------------------------------------

/// Spawn a new vault guard with a random name from `GUARD_NAMES`.
pub fn spawn_guard(rng: &mut impl Rng) -> VaultGuardData {
    let name = random_guard_name(rng);
    VaultGuardData::new(name)
}

// ---------------------------------------------------------------------------
// Guard per-turn action resolution
// ---------------------------------------------------------------------------

/// Maximum turns the guard waits before giving up.
const MAX_WAIT_TURNS: i32 = 30;

/// Process vault guard behavior for one turn.
///
/// The guard steps through its state machine based on player behavior:
/// - Idle -> Approaching when player detected in vault
/// - Approaching -> Asking when guard reaches vault
/// - Asking -> Escorting/CollectingGold based on player response
/// - Escorting -> Leaving when player reaches the exit
/// - CollectingGold -> Leaving after gold is taken
/// - Leaving -> guard despawns
pub fn guard_action(
    guard: &mut VaultGuardData,
    player_pos: Position,
    player_gold: i32,
    player_in_vault: bool,
    player_name_response: Option<&str>,
) -> Vec<GuardEvent> {
    let mut events = Vec::new();

    match guard.state {
        VaultGuardState::Idle => {
            if player_in_vault {
                guard.state = VaultGuardState::Approaching;
                events.push(GuardEvent::StateChange {
                    new_state: VaultGuardState::Approaching,
                });
            }
        }
        VaultGuardState::Approaching => {
            // Guard arrived at the vault, now asks the player.
            guard.state = VaultGuardState::Asking;
            events.push(GuardEvent::Message(
                format!("{} says: \"Who are you?\"", guard.guard_name),
            ));
            events.push(GuardEvent::StateChange {
                new_state: VaultGuardState::Asking,
            });
        }
        VaultGuardState::Asking => {
            guard.turns_waiting += 1;

            if let Some(name) = player_name_response {
                guard.player_said_name = Some(name.to_string());

                // If player gave name "Croesus", guard is suspicious.
                if name.eq_ignore_ascii_case("croesus") {
                    events.push(GuardEvent::Message(
                        format!("{} says: \"I don't believe you!\"", guard.guard_name),
                    ));
                } else {
                    events.push(GuardEvent::Message(
                        format!("{} says: \"I'll be watching you, {}.\"", guard.guard_name, name),
                    ));
                }

                if player_gold > 0 {
                    guard.gold_demanded = player_gold;
                    guard.state = VaultGuardState::CollectingGold;
                    events.push(GuardEvent::StateChange {
                        new_state: VaultGuardState::CollectingGold,
                    });
                } else {
                    guard.state = VaultGuardState::Escorting;
                    events.push(GuardEvent::StateChange {
                        new_state: VaultGuardState::Escorting,
                    });
                }
            } else if guard.turns_waiting > MAX_WAIT_TURNS {
                // Player didn't respond; guard gets aggressive.
                events.push(GuardEvent::Message(
                    format!("{} gets angry!", guard.guard_name),
                ));
                guard.state = VaultGuardState::Leaving;
                events.push(GuardEvent::StateChange {
                    new_state: VaultGuardState::Leaving,
                });
            }
        }
        VaultGuardState::Escorting => {
            if !player_in_vault {
                // Player has left the vault.
                guard.state = VaultGuardState::Leaving;
                events.push(GuardEvent::StateChange {
                    new_state: VaultGuardState::Leaving,
                });
            } else {
                // Point the player toward the exit.
                events.push(GuardEvent::Escort { direction: (0, -1) });
            }
        }
        VaultGuardState::CollectingGold => {
            let amount = guard.gold_demanded;
            events.push(GuardEvent::ConfiscateGold { amount });
            guard.state = VaultGuardState::Escorting;
            events.push(GuardEvent::StateChange {
                new_state: VaultGuardState::Escorting,
            });
        }
        VaultGuardState::Leaving => {
            events.push(GuardEvent::Disappear);
        }
    }

    events
}

/// Convert a `GuardEvent` to an `EngineEvent` for the event bus.
pub fn guard_event_to_engine_event(event: &GuardEvent) -> EngineEvent {
    match event {
        GuardEvent::Message(msg) => EngineEvent::msg_with(
            "vault-guard-speaks",
            vec![("message", msg.clone())],
        ),
        GuardEvent::DemandGold { amount } => EngineEvent::msg_with(
            "vault-guard-demand-gold",
            vec![("amount", amount.to_string())],
        ),
        GuardEvent::ConfiscateGold { amount } => EngineEvent::msg_with(
            "vault-guard-confiscate",
            vec![("amount", amount.to_string())],
        ),
        GuardEvent::Escort { .. } => EngineEvent::msg("vault-guard-escort"),
        GuardEvent::Disappear => EngineEvent::msg("vault-guard-disappear"),
        GuardEvent::StateChange { .. } => EngineEvent::msg("vault-guard-state-change"),
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    #[test]
    fn test_vault_guard_initial_state() {
        let guard = VaultGuardData::new("Fred");
        assert_eq!(guard.state, VaultGuardState::Idle);
        assert_eq!(guard.guard_name, "Fred");
        assert_eq!(guard.gold_demanded, 0);
        assert_eq!(guard.turns_waiting, 0);
        assert!(guard.player_said_name.is_none());
    }

    #[test]
    fn test_vault_guard_transitions_idle_to_approaching() {
        let mut guard = VaultGuardData::new("Dirk");
        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            100,
            true, // player in vault
            None,
        );
        assert_eq!(guard.state, VaultGuardState::Approaching);
        assert!(events.iter().any(|e| matches!(
            e,
            GuardEvent::StateChange { new_state: VaultGuardState::Approaching }
        )));
    }

    #[test]
    fn test_vault_guard_transitions_approaching_to_asking() {
        let mut guard = VaultGuardData::new("Bert");
        guard.state = VaultGuardState::Approaching;

        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            100,
            true,
            None,
        );
        assert_eq!(guard.state, VaultGuardState::Asking);
        assert!(events.iter().any(|e| matches!(e, GuardEvent::Message(_))));
    }

    #[test]
    fn test_vault_guard_asking_with_name_and_gold() {
        let mut guard = VaultGuardData::new("Jay");
        guard.state = VaultGuardState::Asking;

        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            500, // has gold
            true,
            Some("Gandalf"),
        );
        assert_eq!(guard.state, VaultGuardState::CollectingGold);
        assert_eq!(guard.player_said_name.as_deref(), Some("Gandalf"));
        assert_eq!(guard.gold_demanded, 500);
        assert!(events.iter().any(|e| matches!(
            e,
            GuardEvent::StateChange { new_state: VaultGuardState::CollectingGold }
        )));
    }

    #[test]
    fn test_vault_guard_asking_with_name_no_gold() {
        let mut guard = VaultGuardData::new("Kay");
        guard.state = VaultGuardState::Asking;

        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            0, // no gold
            true,
            Some("Adventurer"),
        );
        assert_eq!(guard.state, VaultGuardState::Escorting);
        assert!(events.iter().any(|e| matches!(
            e,
            GuardEvent::StateChange { new_state: VaultGuardState::Escorting }
        )));
    }

    #[test]
    fn test_guard_name_from_list() {
        let mut rng = test_rng();
        let name = random_guard_name(&mut rng);
        assert!(GUARD_NAMES.contains(&name));
    }

    #[test]
    fn test_player_in_vault() {
        let vaults = vec![
            VaultRoom {
                top_left: Position::new(10, 10),
                bottom_right: Position::new(12, 12),
            },
        ];
        assert_eq!(player_in_vault(Position::new(11, 11), &vaults), Some(0));
        assert_eq!(player_in_vault(Position::new(10, 10), &vaults), Some(0));
        assert_eq!(player_in_vault(Position::new(12, 12), &vaults), Some(0));
        assert_eq!(player_in_vault(Position::new(9, 9), &vaults), None);
        assert_eq!(player_in_vault(Position::new(13, 13), &vaults), None);
    }

    #[test]
    fn test_spawn_guard_has_valid_name() {
        let mut rng = test_rng();
        let guard = spawn_guard(&mut rng);
        assert!(GUARD_NAMES.contains(&guard.guard_name.as_str()));
        assert_eq!(guard.state, VaultGuardState::Idle);
    }

    #[test]
    fn test_guard_collecting_gold_then_escorting() {
        let mut guard = VaultGuardData::new("Tom");
        guard.state = VaultGuardState::CollectingGold;
        guard.gold_demanded = 300;

        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            300,
            true,
            None,
        );
        assert_eq!(guard.state, VaultGuardState::Escorting);
        assert!(events.iter().any(|e| matches!(
            e,
            GuardEvent::ConfiscateGold { amount: 300 }
        )));
    }

    #[test]
    fn test_guard_escorting_player_leaves_vault() {
        let mut guard = VaultGuardData::new("Ron");
        guard.state = VaultGuardState::Escorting;

        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            0,
            false, // player left the vault
            None,
        );
        assert_eq!(guard.state, VaultGuardState::Leaving);
        assert!(events.iter().any(|e| matches!(
            e,
            GuardEvent::StateChange { new_state: VaultGuardState::Leaving }
        )));
    }

    #[test]
    fn test_guard_leaving_emits_disappear() {
        let mut guard = VaultGuardData::new("Maud");
        guard.state = VaultGuardState::Leaving;

        let events = guard_action(
            &mut guard,
            Position::new(5, 5),
            0,
            false,
            None,
        );
        assert!(events.iter().any(|e| matches!(e, GuardEvent::Disappear)));
    }

    #[test]
    fn test_guard_event_to_engine_event() {
        let event = GuardEvent::ConfiscateGold { amount: 100 };
        let engine_event = guard_event_to_engine_event(&event);
        assert!(matches!(
            engine_event,
            EngineEvent::Message { key, .. } if key == "vault-guard-confiscate"
        ));
    }
}
