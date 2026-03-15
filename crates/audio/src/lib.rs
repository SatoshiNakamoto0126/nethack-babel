use std::path::{Path, PathBuf};

use nethack_babel_engine::event::EngineEvent;

/// Sound effect types mapped to game events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundEffect {
    MeleeHit,
    MeleeMiss,
    ItemPickup,
    DoorOpen,
    DoorClose,
    DrinkPotion,
    ReadScroll,
    MonsterDeath,
    PlayerDeath,
    LevelUp,
    LowHpWarning,
    Bell,
    Fountain,
    Explosion,
    Thunder,
    Crumble,
    StairsUse,
    TrapTrigger,
}

impl SoundEffect {
    /// Return the filename associated with this sound effect.
    pub fn filename(self) -> &'static str {
        match self {
            SoundEffect::MeleeHit => "melee_hit.ogg",
            SoundEffect::MeleeMiss => "melee_miss.ogg",
            SoundEffect::ItemPickup => "item_pickup.ogg",
            SoundEffect::DoorOpen => "door_open.ogg",
            SoundEffect::DoorClose => "door_close.ogg",
            SoundEffect::DrinkPotion => "drink.ogg",
            SoundEffect::ReadScroll => "paper.ogg",
            SoundEffect::MonsterDeath => "monster_death.ogg",
            SoundEffect::PlayerDeath => "player_death.ogg",
            SoundEffect::LevelUp => "level_up.ogg",
            SoundEffect::LowHpWarning => "heartbeat.ogg",
            SoundEffect::Bell => "bell.ogg",
            SoundEffect::Fountain => "water.ogg",
            SoundEffect::Explosion => "explosion.ogg",
            SoundEffect::Thunder => "thunder.ogg",
            SoundEffect::Crumble => "crumble.ogg",
            SoundEffect::StairsUse => "stairs.ogg",
            SoundEffect::TrapTrigger => "trap.ogg",
        }
    }
}

// ---------------------------------------------------------------------------
// Standalone event-to-sound mapping
// ---------------------------------------------------------------------------

/// Map a single engine event to the sound effect it should produce.
///
/// Returns `None` for events that have no associated sound (e.g.
/// `TurnEnd`, `Message`, pure state queries).
///
/// This is a free function so callers that don't use `AudioPlayer` (e.g.
/// test harnesses, analytics) can still inspect the mapping.
pub fn event_to_sound(event: &EngineEvent) -> Option<SoundEffect> {
    match event {
        // Combat
        EngineEvent::MeleeHit { .. } => Some(SoundEffect::MeleeHit),
        EngineEvent::MeleeMiss { .. } => Some(SoundEffect::MeleeMiss),
        EngineEvent::RangedHit { .. } => Some(SoundEffect::MeleeHit),
        EngineEvent::RangedMiss { .. } => Some(SoundEffect::MeleeMiss),

        // Items
        EngineEvent::ItemPickedUp { .. } => Some(SoundEffect::ItemPickup),

        // Doors
        EngineEvent::DoorOpened { .. } => Some(SoundEffect::DoorOpen),
        EngineEvent::DoorClosed { .. } => Some(SoundEffect::DoorClose),
        EngineEvent::DoorBroken { .. } => Some(SoundEffect::Crumble),

        // Entity lifecycle
        EngineEvent::EntityDied { .. } => Some(SoundEffect::MonsterDeath),
        EngineEvent::LevelUp { .. } => Some(SoundEffect::LevelUp),

        // Environment
        EngineEvent::FountainDrank { .. } => Some(SoundEffect::Fountain),
        EngineEvent::TrapTriggered { .. } => Some(SoundEffect::TrapTrigger),
        EngineEvent::LevelChanged { .. } => Some(SoundEffect::StairsUse),

        // Everything else has no default sound.
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// AudioPlayer
// ---------------------------------------------------------------------------

pub struct AudioPlayer {
    enabled: bool,
    volume: f32,
    asset_path: PathBuf,
    // In a real implementation this would hold rodio::OutputStream and Sink.
    // For now we operate as a no-op that logs intent.
}

impl AudioPlayer {
    pub fn new(asset_path: impl Into<PathBuf>, enabled: bool, volume: u8) -> Self {
        Self {
            enabled,
            volume: volume as f32 / 100.0,
            asset_path: asset_path.into(),
        }
    }

    /// Play the given sound effect.
    ///
    /// Currently a no-op that prints to stderr; actual rodio integration will
    /// be added later.
    pub fn play(&self, effect: SoundEffect) {
        if !self.enabled {
            return;
        }
        let path = self.asset_path.join(effect.filename());
        // TODO: Actually play using rodio in a fire-and-forget thread.
        #[cfg(debug_assertions)]
        eprintln!(
            "[audio] Playing sound: {} (volume: {:.0}%)",
            path.display(),
            self.volume * 100.0,
        );
        let _ = path; // suppress unused warning in release
    }

    pub fn set_volume(&mut self, volume: u8) {
        self.volume = volume as f32 / 100.0;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn asset_path(&self) -> &Path {
        &self.asset_path
    }

    /// Map a single engine event to a sound effect and play it.
    pub fn play_for_event(&self, event: &EngineEvent) {
        if let Some(effect) = event_to_sound(event) {
            self.play(effect);
        }
    }

    /// Play all sounds for a batch of engine events.
    ///
    /// Iterates through `events` in order, mapping each to a sound via
    /// [`event_to_sound`] and playing it.  Events that don't map to a
    /// sound are silently skipped.
    pub fn play_for_events(&self, events: &[EngineEvent]) {
        if !self.enabled {
            return;
        }
        for event in events {
            self.play_for_event(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_engine::action::Position;
    use nethack_babel_engine::event::{DeathCause, TrapType};

    #[test]
    fn disabled_player_does_not_panic() {
        let player = AudioPlayer::new("/nonexistent", false, 50);
        player.play(SoundEffect::Bell);
    }

    #[test]
    fn volume_clamping() {
        let mut player = AudioPlayer::new("/tmp", true, 100);
        assert!((player.volume() - 1.0).abs() < f32::EPSILON);
        player.set_volume(0);
        assert!(player.volume().abs() < f32::EPSILON);
    }

    #[test]
    fn sound_effect_filenames() {
        assert_eq!(SoundEffect::MeleeHit.filename(), "melee_hit.ogg");
        assert_eq!(SoundEffect::Thunder.filename(), "thunder.ogg");
        assert_eq!(SoundEffect::StairsUse.filename(), "stairs.ogg");
        assert_eq!(SoundEffect::TrapTrigger.filename(), "trap.ogg");
    }

    // ── event_to_sound mapping tests ─────────────────────────────────

    #[test]
    fn combat_events_produce_sounds() {
        let world = hecs::World::new();
        // We can't easily create real entities without inserting them,
        // but we can spawn dummy entities for the event.
        let mut w = hecs::World::new();
        let e1 = w.spawn(());
        let e2 = w.spawn(());

        let hit = EngineEvent::MeleeHit {
            attacker: e1,
            defender: e2,
            weapon: None,
            damage: 5,
        };
        assert_eq!(event_to_sound(&hit), Some(SoundEffect::MeleeHit));

        let miss = EngineEvent::MeleeMiss {
            attacker: e1,
            defender: e2,
        };
        assert_eq!(event_to_sound(&miss), Some(SoundEffect::MeleeMiss));

        let _ = world; // suppress unused
    }

    #[test]
    fn door_events_produce_sounds() {
        let open = EngineEvent::DoorOpened {
            position: Position::new(5, 5),
        };
        assert_eq!(event_to_sound(&open), Some(SoundEffect::DoorOpen));

        let close = EngineEvent::DoorClosed {
            position: Position::new(5, 5),
        };
        assert_eq!(event_to_sound(&close), Some(SoundEffect::DoorClose));
    }

    #[test]
    fn trap_and_stairs_events_produce_sounds() {
        let mut w = hecs::World::new();
        let e = w.spawn(());

        let trap = EngineEvent::TrapTriggered {
            entity: e,
            trap_type: TrapType::Pit,
            position: Position::new(3, 3),
        };
        assert_eq!(event_to_sound(&trap), Some(SoundEffect::TrapTrigger));

        let stairs = EngineEvent::LevelChanged {
            entity: e,
            from_depth: "1".to_string(),
            to_depth: "2".to_string(),
        };
        assert_eq!(event_to_sound(&stairs), Some(SoundEffect::StairsUse));
    }

    #[test]
    fn silent_events_return_none() {
        let msg = EngineEvent::Message {
            key: "hello".to_string(),
            args: vec![],
        };
        assert_eq!(event_to_sound(&msg), None);

        let turn = EngineEvent::TurnEnd { turn_number: 42 };
        assert_eq!(event_to_sound(&turn), None);

        let more = EngineEvent::MessageMore;
        assert_eq!(event_to_sound(&more), None);
    }

    #[test]
    fn play_for_events_processes_batch() {
        let player = AudioPlayer::new("/tmp/sounds", false, 50);
        let mut w = hecs::World::new();
        let e1 = w.spawn(());
        let e2 = w.spawn(());

        let events = vec![
            EngineEvent::MeleeHit {
                attacker: e1,
                defender: e2,
                weapon: None,
                damage: 3,
            },
            EngineEvent::Message {
                key: "melee-hit-bare".to_string(),
                args: vec![],
            },
            EngineEvent::DoorOpened {
                position: Position::new(1, 1),
            },
            EngineEvent::LevelUp {
                entity: e1,
                new_level: 5,
            },
        ];

        // Should not panic even though disabled.
        player.play_for_events(&events);
    }

    #[test]
    fn entity_death_maps_to_monster_death() {
        let mut w = hecs::World::new();
        let e = w.spawn(());

        let death = EngineEvent::EntityDied {
            entity: e,
            killer: None,
            cause: DeathCause::KilledBy {
                killer_name: "a troll".to_string(),
            },
        };
        assert_eq!(event_to_sound(&death), Some(SoundEffect::MonsterDeath));
    }

    #[test]
    fn item_pickup_maps_correctly() {
        let mut w = hecs::World::new();
        let actor = w.spawn(());
        let item = w.spawn(());

        let pickup = EngineEvent::ItemPickedUp {
            actor,
            item,
            quantity: 1,
        };
        assert_eq!(event_to_sound(&pickup), Some(SoundEffect::ItemPickup));
    }
}
