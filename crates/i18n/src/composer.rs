use fluent::FluentArgs;
use nethack_babel_engine::event::EngineEvent;
use nethack_babel_engine::world::GameWorld;

use crate::locale::LocaleManager;

/// Converts engine events into localized player-visible message strings.
pub struct MessageComposer<'a> {
    locale: &'a LocaleManager,
}

impl<'a> MessageComposer<'a> {
    pub fn new(locale: &'a LocaleManager) -> Self {
        Self { locale }
    }

    /// Convert a single engine event to localized message text.
    ///
    /// Returns `None` for events that don't produce player-visible messages
    /// (e.g. `TurnEnd`, `MessageMore`).
    pub fn compose(&self, event: &EngineEvent, world: &GameWorld) -> Option<String> {
        match event {
            EngineEvent::MeleeHit {
                attacker,
                defender,
                weapon,
                ..
            } => {
                let att_name = self.entity_display_name(*attacker, world);
                let def_name = self.entity_display_name(*defender, world);
                match weapon {
                    Some(w) => {
                        let wpn_en = world.entity_name(*w);
                        let wpn_name = self.locale.translate_object_name(&wpn_en).to_string();
                        let mut args = FluentArgs::new();
                        args.set("attacker", att_name.clone());
                        args.set("defender", def_name.clone());
                        args.set("weapon", wpn_name.clone());
                        let msg = self.locale.translate("melee-hit-weapon", Some(&args));
                        if msg == "melee-hit-weapon" {
                            Some(format!(
                                "{} hits {} with {}!",
                                att_name, def_name, wpn_name
                            ))
                        } else {
                            Some(msg)
                        }
                    }
                    None => {
                        let mut args = FluentArgs::new();
                        args.set("attacker", att_name.clone());
                        args.set("defender", def_name.clone());
                        let msg = self.locale.translate("melee-hit", Some(&args));
                        if msg == "melee-hit" {
                            Some(format!("{} hits {}!", att_name, def_name))
                        } else {
                            Some(msg)
                        }
                    }
                }
            }

            EngineEvent::MeleeMiss {
                attacker, defender, ..
            } => {
                let att_name = self.entity_display_name(*attacker, world);
                let def_name = self.entity_display_name(*defender, world);
                let mut args = FluentArgs::new();
                args.set("attacker", att_name.clone());
                args.set("defender", def_name.clone());
                let msg = self.locale.translate("melee-miss", Some(&args));
                if msg == "melee-miss" {
                    Some(format!("{} misses {}.", att_name, def_name))
                } else {
                    Some(msg)
                }
            }

            EngineEvent::EntityDied { entity, .. } => {
                let name = self.entity_display_name(*entity, world);
                let mut args = FluentArgs::new();
                args.set("entity", name.clone());
                let msg = self.locale.translate("entity-died", Some(&args));
                if msg == "entity-died" {
                    Some(format!("{} is killed!", name))
                } else {
                    Some(msg)
                }
            }

            EngineEvent::ItemPickedUp {
                actor,
                item,
                quantity,
            } => {
                let actor_name = self.entity_display_name(*actor, world);
                let item_name = world.entity_name(*item);
                let mut args = FluentArgs::new();
                args.set("actor", actor_name.clone());
                args.set("item", item_name.clone());
                args.set("quantity", *quantity as i64);
                let msg = self.locale.translate("item-picked-up", Some(&args));
                if msg == "item-picked-up" {
                    if world.is_player(*actor) {
                        Some(format!("You pick up {}.", item_name))
                    } else {
                        Some(format!("{} picks up {}.", actor_name, item_name))
                    }
                } else {
                    Some(msg)
                }
            }

            EngineEvent::HpChange {
                entity, amount, ..
            } => {
                if !world.is_player(*entity) {
                    return None;
                }
                let feeling = if *amount > 0 { "better" } else { "worse" };
                let mut args = FluentArgs::new();
                args.set("feeling", feeling.to_string());
                let msg = self.locale.translate("hp-change", Some(&args));
                if msg == "hp-change" {
                    Some(format!("You feel {}.", feeling))
                } else {
                    Some(msg)
                }
            }

            EngineEvent::LevelUp {
                entity, new_level, ..
            } => {
                if !world.is_player(*entity) {
                    return None;
                }
                let mut args = FluentArgs::new();
                args.set("level", *new_level as i64);
                let msg = self.locale.translate("level-up", Some(&args));
                if msg == "level-up" {
                    Some(format!("Welcome to experience level {}!", new_level))
                } else {
                    Some(msg)
                }
            }

            EngineEvent::Message { key, args } => {
                let mut fluent_args = FluentArgs::new();
                for (k, v) in args {
                    fluent_args.set(k.clone(), v.clone());
                }
                Some(self.locale.translate(key, Some(&fluent_args)))
            }

            // Non-message events.
            EngineEvent::MessageMore | EngineEvent::TurnEnd { .. } => None,

            // Generic fallback for all other event variants.
            other => Some(format!("Event: {}", event_variant_name(other))),
        }
    }

    /// Convert multiple events, filtering out non-message events.
    pub fn compose_all(&self, events: &[EngineEvent], world: &GameWorld) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| self.compose(e, world))
            .collect()
    }

    // ── private ──────────────────────────────────────────────────

    /// Display name for an entity, using "You" for the player.
    ///
    /// For non-player entities, translates monster names via the locale's
    /// entity translation tables.
    fn entity_display_name(&self, entity: hecs::Entity, world: &GameWorld) -> String {
        if world.is_player(entity) {
            let msg = self.locale.translate("you", None);
            if msg == "you" {
                "You".to_string()
            } else {
                msg
            }
        } else {
            let en_name = world.entity_name(entity);
            self.locale.translate_monster_name(&en_name).to_string()
        }
    }
}

/// Extract a short variant name from an EngineEvent for debug display.
fn event_variant_name(event: &EngineEvent) -> &'static str {
    match event {
        EngineEvent::MeleeHit { .. } => "MeleeHit",
        EngineEvent::MeleeMiss { .. } => "MeleeMiss",
        EngineEvent::RangedHit { .. } => "RangedHit",
        EngineEvent::RangedMiss { .. } => "RangedMiss",
        EngineEvent::ExtraDamage { .. } => "ExtraDamage",
        EngineEvent::StatusApplied { .. } => "StatusApplied",
        EngineEvent::StatusRemoved { .. } => "StatusRemoved",
        EngineEvent::PassiveAttack { .. } => "PassiveAttack",
        EngineEvent::ItemPickedUp { .. } => "ItemPickedUp",
        EngineEvent::ItemDropped { .. } => "ItemDropped",
        EngineEvent::ItemWielded { .. } => "ItemWielded",
        EngineEvent::ItemWorn { .. } => "ItemWorn",
        EngineEvent::ItemRemoved { .. } => "ItemRemoved",
        EngineEvent::ItemDamaged { .. } => "ItemDamaged",
        EngineEvent::ItemDestroyed { .. } => "ItemDestroyed",
        EngineEvent::ItemIdentified { .. } => "ItemIdentified",
        EngineEvent::ItemCharged { .. } => "ItemCharged",
        EngineEvent::EntityDied { .. } => "EntityDied",
        EngineEvent::EntityMoved { .. } => "EntityMoved",
        EngineEvent::EntityTeleported { .. } => "EntityTeleported",
        EngineEvent::HungerChange { .. } => "HungerChange",
        EngineEvent::LevelUp { .. } => "LevelUp",
        EngineEvent::HpChange { .. } => "HpChange",
        EngineEvent::PwChange { .. } => "PwChange",
        EngineEvent::DoorOpened { .. } => "DoorOpened",
        EngineEvent::DoorClosed { .. } => "DoorClosed",
        EngineEvent::DoorLocked { .. } => "DoorLocked",
        EngineEvent::DoorBroken { .. } => "DoorBroken",
        EngineEvent::TrapTriggered { .. } => "TrapTriggered",
        EngineEvent::TrapRevealed { .. } => "TrapRevealed",
        EngineEvent::FountainDrank { .. } => "FountainDrank",
        EngineEvent::AltarPrayed { .. } => "AltarPrayed",
        EngineEvent::LevelChanged { .. } => "LevelChanged",
        EngineEvent::MonsterGenerated { .. } => "MonsterGenerated",
        EngineEvent::Message { .. } => "Message",
        EngineEvent::MessageMore => "MessageMore",
        EngineEvent::GameOver { .. } => "GameOver",
        EngineEvent::TurnEnd { .. } => "TurnEnd",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::LocaleManager;
    use nethack_babel_engine::action::Position;
    use nethack_babel_engine::event::DeathCause;
    use nethack_babel_engine::world::{GameWorld, Name};

    fn setup_world() -> GameWorld {
        GameWorld::new(Position::new(0, 0))
    }

    #[test]
    fn test_melee_hit_no_weapon() {
        let locale = LocaleManager::new();
        let composer = MessageComposer::new(&locale);
        let mut world = setup_world();
        let goblin = world.spawn((Name("goblin".to_string()),));

        let event = EngineEvent::MeleeHit {
            attacker: world.player(),
            defender: goblin,
            weapon: None,
            damage: 5,
        };
        let msg = composer.compose(&event, &world).unwrap();
        assert_eq!(msg, "You hits goblin!");
    }

    #[test]
    fn test_entity_died() {
        let locale = LocaleManager::new();
        let composer = MessageComposer::new(&locale);
        let mut world = setup_world();
        let goblin = world.spawn((Name("goblin".to_string()),));

        let event = EngineEvent::EntityDied {
            entity: goblin,
            killer: Some(world.player()),
            cause: DeathCause::KilledBy {
                killer_name: "you".to_string(),
            },
        };
        let msg = composer.compose(&event, &world).unwrap();
        assert_eq!(msg, "goblin is killed!");
    }

    #[test]
    fn test_turn_end_returns_none() {
        let locale = LocaleManager::new();
        let composer = MessageComposer::new(&locale);
        let world = setup_world();

        let event = EngineEvent::TurnEnd { turn_number: 1 };
        assert!(composer.compose(&event, &world).is_none());
    }

    #[test]
    fn test_level_up() {
        let locale = LocaleManager::new();
        let composer = MessageComposer::new(&locale);
        let world = setup_world();

        let event = EngineEvent::LevelUp {
            entity: world.player(),
            new_level: 5,
        };
        let msg = composer.compose(&event, &world).unwrap();
        assert_eq!(msg, "Welcome to experience level 5!");
    }

    #[test]
    fn test_cjk_melee_hit() {
        use crate::locale::LanguageManifest;
        let mut locale = LocaleManager::new();
        let manifest = LanguageManifest {
            code: "zh-CN".to_string(),
            name: "\u{7b80}\u{4f53}\u{4e2d}\u{6587}".to_string(),
            name_en: "Simplified Chinese".to_string(),
            author: "test".to_string(),
            version: "0.1.0".to_string(),
            fallback: Some("en".to_string()),
            is_cjk: true,
            has_articles: false,
            has_plural: false,
            has_verb_conj: false,
            has_gender: false,
            possessive: Some("\u{7684}".to_string()),
            quote_left: Some("\u{300c}".to_string()),
            quote_right: Some("\u{300d}".to_string()),
        };
        let ftl = "you = \u{4f60}\nmelee-hit-weapon = { $attacker }\u{7528}{ $weapon }\u{51fb}\u{4e2d}\u{4e86}{ $defender }\u{ff01}\n";
        locale
            .load_locale("zh-CN", manifest, &[("messages.ftl", ftl)])
            .unwrap();
        // Load entity translations.
        let monsters_toml = "[translations]\n\"giant ant\" = \"\u{5de8}\u{8681}\"\n";
        let objects_toml = "[translations]\n\"long sword\" = \"\u{957f}\u{5251}\"\n";
        locale
            .load_entity_translations("zh-CN", monsters_toml, objects_toml)
            .unwrap();
        locale.set_language("zh-CN").unwrap();

        let composer = MessageComposer::new(&locale);
        let mut world = setup_world();
        let ant = world.spawn((Name("giant ant".to_string()),));
        let sword = world.spawn((Name("long sword".to_string()),));

        let event = EngineEvent::MeleeHit {
            attacker: world.player(),
            defender: ant,
            weapon: Some(sword),
            damage: 5,
        };
        let msg = composer.compose(&event, &world).unwrap();
        // Should contain translated names.
        assert!(
            msg.contains("\u{5de8}\u{8681}"),
            "Expected \u{5de8}\u{8681} in '{}'",
            msg
        ); // 巨蚁
        assert!(
            msg.contains("\u{957f}\u{5251}"),
            "Expected \u{957f}\u{5251} in '{}'",
            msg
        ); // 长剑
        assert!(
            !msg.contains("giant ant"),
            "Should not contain English 'giant ant' in '{}'",
            msg
        );
        assert!(
            !msg.contains("long sword"),
            "Should not contain English 'long sword' in '{}'",
            msg
        );
    }
}
