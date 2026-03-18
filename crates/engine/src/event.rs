//! Engine event types: the primary output channel for all game logic.
//!
//! The engine communicates every observable action through `EngineEvent`.
//! Consumers (TUI, i18n, audio, replay) subscribe to these events to
//! drive rendering, localization, sound, and recording.

use hecs::Entity;
use serde::{Deserialize, Serialize};

use crate::action::{PlayerAction, Position};

// Re-export TrapType from the canonical data crate definition.
pub use nethack_babel_data::TrapType;

// ── Sub-enums used by EngineEvent ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageSource {
    Melee,
    Ranged,
    Spell,
    Wand,
    Breath,
    Explosion,
    Trap,
    Poison,
    Starvation,
    Fire,
    Cold,
    Shock,
    Acid,
    Disintegration,
    Drain,
    Petrification,
    Divine,
    Environment,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusEffect {
    Blind,
    Confused,
    Stunned,
    Hallucinating,
    Clairvoyant,
    Paralyzed,
    Sleeping,
    Levitating,
    Flying,
    Invisible,
    SeeInvisible,
    FastSpeed,
    SlowSpeed,
    Sick,
    FoodPoisoned,
    Stoning,
    Slimed,
    Strangled,
    Polymorphed,
    Lycanthropy,
    Telepathy,
    Warning,
    Aggravate,
    Stealth,
    Protected,
    Reflection,
    MagicResistance,
    FireResistance,
    ColdResistance,
    ShockResistance,
    SleepResistance,
    PoisonResistance,
    DisintegrationResistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PassiveEffect {
    AcidSplash { damage: u32 },
    ElectricShock { damage: u32 },
    PoisonSting,
    Petrify,
    Paralyze,
    Slow,
    Corrode,
    Teleport,
    Engulf,
    Steal,
    Seduce,
    Wrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageCause {
    Fire,
    Cold,
    Shock,
    Acid,
    Rust,
    Rot,
    Corrosion,
    Disenchant,
    Physical,
    Curse,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeathCause {
    KilledBy { killer_name: String },
    Starvation,
    Poisoning,
    Petrification,
    Drowning,
    Burning,
    Disintegration,
    Sickness,
    Strangulation,
    Falling,
    CrushedByBoulder,
    Quit,
    Escaped,
    Ascended,
    Trickery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HungerLevel {
    Satiated,
    NotHungry,
    Hungry,
    Weak,
    Fainting,
    Fainted,
    Starved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HpSource {
    Combat,
    Spell,
    Potion,
    Regeneration,
    Trap,
    Poison,
    Environment,
    Divine,
    Drain,
    Other,
}

// ── Fine-grained engine events ────────────────────────────────────────────

/// Events emitted by the engine during turn resolution.
/// These are the primary output channel: the renderer converts them to
/// visual/audio effects, the replay system records them, and analytics
/// aggregates them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    // ── Combat ────────────────────────────────────────────────
    MeleeHit {
        attacker: Entity,
        defender: Entity,
        weapon: Option<Entity>,
        damage: u32,
    },
    MeleeMiss {
        attacker: Entity,
        defender: Entity,
    },
    RangedHit {
        attacker: Entity,
        defender: Entity,
        projectile: Entity,
        damage: u32,
    },
    RangedMiss {
        attacker: Entity,
        defender: Entity,
        projectile: Entity,
    },
    ExtraDamage {
        target: Entity,
        amount: u32,
        source: DamageSource,
    },
    StatusApplied {
        entity: Entity,
        status: StatusEffect,
        duration: Option<u32>,
        source: Option<Entity>,
    },
    StatusRemoved {
        entity: Entity,
        status: StatusEffect,
    },
    PassiveAttack {
        attacker: Entity,
        defender: Entity,
        effect: PassiveEffect,
    },

    // ── Items ─────────────────────────────────────────────────
    ItemPickedUp {
        actor: Entity,
        item: Entity,
        quantity: u32,
    },
    ItemDropped {
        actor: Entity,
        item: Entity,
    },
    ItemWielded {
        actor: Entity,
        item: Entity,
    },
    ItemWorn {
        actor: Entity,
        item: Entity,
    },
    ItemRemoved {
        actor: Entity,
        item: Entity,
    },
    ItemDamaged {
        item: Entity,
        cause: DamageCause,
    },
    ItemDestroyed {
        item: Entity,
        cause: DamageCause,
    },
    ItemIdentified {
        item: Entity,
    },
    ItemCharged {
        item: Entity,
        new_charges: i8,
    },

    // ── Entity state ──────────────────────────────────────────
    EntityDied {
        entity: Entity,
        killer: Option<Entity>,
        cause: DeathCause,
    },
    EntityMoved {
        entity: Entity,
        from: Position,
        to: Position,
    },
    EntityTeleported {
        entity: Entity,
        from: Position,
        to: Position,
    },
    HungerChange {
        entity: Entity,
        old: HungerLevel,
        new_level: HungerLevel,
    },
    LevelUp {
        entity: Entity,
        new_level: u8,
    },
    HpChange {
        entity: Entity,
        amount: i32,
        new_hp: i32,
        source: HpSource,
    },
    PwChange {
        entity: Entity,
        amount: i32,
        new_pw: i32,
    },

    // ── Environment ───────────────────────────────────────────
    DoorOpened {
        position: Position,
    },
    DoorClosed {
        position: Position,
    },
    DoorLocked {
        position: Position,
    },
    DoorBroken {
        position: Position,
    },
    TrapTriggered {
        entity: Entity,
        trap_type: TrapType,
        position: Position,
    },
    TrapRevealed {
        position: Position,
        trap_type: TrapType,
    },
    FountainDrank {
        entity: Entity,
        position: Position,
    },
    AltarPrayed {
        entity: Entity,
        position: Position,
    },

    // ── Dungeon ───────────────────────────────────────────────
    LevelChanged {
        entity: Entity,
        from_depth: String,
        to_depth: String,
    },
    MonsterGenerated {
        entity: Entity,
        position: Position,
    },

    // ── Messages ──────────────────────────────────────────────
    /// A localizable message to display to the player.
    /// `key` is a Fluent message identifier; `args` are interpolation
    /// parameters the CLI layer converts to `FluentArgs`.
    Message {
        key: String,
        args: Vec<(String, String)>,
    },

    // ── Control ───────────────────────────────────────────────
    MessageMore,
    GameOver {
        cause: DeathCause,
        score: u64,
    },
    TurnEnd {
        turn_number: u32,
    },
}

// ── Message construction helpers ──────────────────────────────────────────

impl EngineEvent {
    /// Create a `Message` event with a key and no arguments.
    pub fn msg(key: &str) -> Self {
        EngineEvent::Message {
            key: key.to_string(),
            args: vec![],
        }
    }

    /// Create a `Message` event with a key and named arguments.
    pub fn msg_with(key: &str, args: Vec<(&str, String)>) -> Self {
        EngineEvent::Message {
            key: key.to_string(),
            args: args.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        }
    }
}

// ── Analytics (aggregated) layer ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CombatOutcome {
    AttackerWon,
    DefenderWon,
    Ongoing,
    BothDied,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuestMilestone {
    QuestAssigned,
    LeaderMet,
    ArtifactObtained,
    NemesisDefeated,
    QuestCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConductType {
    Foodless,
    Vegan,
    Vegetarian,
    Atheist,
    Weaponless,
    Pacifist,
    Illiterate,
    Polypileless,
    Polyselfless,
    Wishless,
    Artiwishless,
    Genocideless,
}

/// High-level analytics events for replay/stats tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalyticsEvent {
    CombatRound {
        attacker: Entity,
        defender: Entity,
        total_damage: u32,
        outcome: CombatOutcome,
    },
    TurnCompleted {
        turn: u32,
        action: PlayerAction,
    },
    FloorChanged {
        from: String,
        to: String,
    },
    QuestProgress {
        milestone: QuestMilestone,
    },
    ConductViolated {
        conduct: ConductType,
    },
}

/// Summarize a sequence of fine-grained engine events into analytics events.
///
/// This is a skeleton — real aggregation logic will be filled in as the
/// combat and quest systems are implemented.
pub fn summarize(events: &[EngineEvent]) -> Vec<AnalyticsEvent> {
    let mut analytics = Vec::new();

    // Track combat rounds: group consecutive hits between the same pair
    // into a single CombatRound.  (Skeleton — only handles simple cases.)
    let mut combat_damage: u32 = 0;
    let mut combat_attacker: Option<Entity> = None;
    let mut combat_defender: Option<Entity> = None;

    for event in events {
        match event {
            EngineEvent::MeleeHit {
                attacker,
                defender,
                damage,
                ..
            }
            | EngineEvent::RangedHit {
                attacker,
                defender,
                damage,
                ..
            } => {
                let same_pair =
                    combat_attacker == Some(*attacker) && combat_defender == Some(*defender);
                if !same_pair {
                    // Flush previous round.
                    flush_combat_round(
                        &mut analytics,
                        &mut combat_attacker,
                        &mut combat_defender,
                        &mut combat_damage,
                    );
                    combat_attacker = Some(*attacker);
                    combat_defender = Some(*defender);
                }
                combat_damage += damage;
            }
            EngineEvent::EntityDied { entity, .. } => {
                // If the defender in the current combat round just died,
                // close the round as AttackerWon.
                if combat_defender == Some(*entity)
                    && let (Some(a), Some(d)) = (combat_attacker, combat_defender)
                {
                    analytics.push(AnalyticsEvent::CombatRound {
                        attacker: a,
                        defender: d,
                        total_damage: combat_damage,
                        outcome: CombatOutcome::AttackerWon,
                    });
                    combat_attacker = None;
                    combat_defender = None;
                    combat_damage = 0;
                }
            }
            EngineEvent::LevelChanged {
                from_depth,
                to_depth,
                ..
            } => {
                analytics.push(AnalyticsEvent::FloorChanged {
                    from: from_depth.clone(),
                    to: to_depth.clone(),
                });
            }
            EngineEvent::TurnEnd { turn_number } => {
                // We don't have access to the original PlayerAction here,
                // so TurnCompleted events should be constructed at a higher
                // level.  This is a placeholder.
                let _ = turn_number;
            }
            _ => {}
        }
    }

    // Flush any remaining combat round.
    flush_combat_round(
        &mut analytics,
        &mut combat_attacker,
        &mut combat_defender,
        &mut combat_damage,
    );

    analytics
}

fn flush_combat_round(
    analytics: &mut Vec<AnalyticsEvent>,
    attacker: &mut Option<Entity>,
    defender: &mut Option<Entity>,
    damage: &mut u32,
) {
    if let (Some(a), Some(d)) = (*attacker, *defender) {
        analytics.push(AnalyticsEvent::CombatRound {
            attacker: a,
            defender: d,
            total_damage: *damage,
            outcome: CombatOutcome::Ongoing,
        });
    }
    *attacker = None;
    *defender = None;
    *damage = 0;
}
