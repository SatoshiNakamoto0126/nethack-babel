//! Monster data predicates — boolean queries on monster flags, resistances, and size.
//!
//! These correspond to the macros in NetHack's `mondata.h` and functions in
//! `mondata.c`.  Each predicate takes a `&MonsterDef` (or individual fields)
//! and returns a simple boolean.

use nethack_babel_data::schema::{MonsterDef, MonsterFlags, MonsterSize, ResistanceSet};

// ---------------------------------------------------------------------------
// M1 flag predicates
// ---------------------------------------------------------------------------

/// Can this monster fly? (`M1_FLY`)
pub fn is_flyer(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::FLY)
}

/// Can this monster swim? (`M1_SWIM`)
pub fn is_swimmer(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::SWIM)
}

/// Is this monster amorphous — can flow under doors? (`M1_AMORPHOUS`)
pub fn is_amorphous(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::AMORPHOUS)
}

/// Can this monster phase through rock? (`M1_WALLWALK`)
pub fn passes_walls(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::WALLWALK)
}

/// Can this monster cling to ceilings? (`M1_CLING`)
pub fn is_clinger(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::CLING)
}

/// Can this monster tunnel through rock? (`M1_TUNNEL`)
pub fn is_tunneler(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::TUNNEL)
}

/// Does this monster need a pick-axe to tunnel? (`M1_NEEDPICK`)
pub fn needspick(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NEEDPICK)
}

/// Does this monster hide under objects? (`M1_CONCEAL`)
pub fn hides_under(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::CONCEAL)
}

/// Can this monster hide (mimics, piercers)? (`M1_HIDE`)
pub fn is_hider(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::HIDE)
}

/// Can this monster survive underwater? (`M1_AMPHIBIOUS`)
pub fn is_amphibious(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::AMPHIBIOUS)
}

/// Does this monster not need to breathe? (`M1_BREATHLESS`)
pub fn is_breathless(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::BREATHLESS)
}

/// Cannot pick up objects? (`M1_NOTAKE`)
pub fn is_notake(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NOTAKE)
}

/// Has no eyes? (`M1_NOEYES`)
pub fn has_no_eyes(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NOEYES)
}

/// Has eyes? (inverse of `M1_NOEYES`)
pub fn has_eyes(mon: &MonsterDef) -> bool {
    !mon.flags.contains(MonsterFlags::NOEYES)
}

/// Has no hands? (`M1_NOHANDS`)
pub fn has_no_hands(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NOHANDS)
}

/// Has no limbs (includes no hands)? (`M1_NOLIMBS`)
pub fn has_no_limbs(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NOLIMBS)
}

/// Has no head? (`M1_NOHEAD`)
pub fn has_no_head(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NOHEAD)
}

/// Has a head? (inverse of `M1_NOHEAD`)
pub fn has_head(mon: &MonsterDef) -> bool {
    !mon.flags.contains(MonsterFlags::NOHEAD)
}

/// Is mindless (golem, zombie, mold)? (`M1_MINDLESS`)
pub fn is_mindless(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::MINDLESS)
}

/// Has humanoid body shape? (`M1_HUMANOID`)
pub fn is_humanoid(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::HUMANOID)
}

/// Has animal body? (`M1_ANIMAL`)
pub fn is_animal(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::ANIMAL)
}

/// Has serpentine body? (`M1_SLITHY`)
pub fn is_slithy(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::SLITHY)
}

/// Has thick hide or scales? (`M1_THICK_HIDE`)
pub fn has_thick_hide(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::THICK_HIDE)
}

/// Is non-corporeal (no solid or liquid body)? (`M1_UNSOLID`)
pub fn is_unsolid(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::UNSOLID)
}

/// Can lay eggs? (`M1_OVIPAROUS`)
pub fn lays_eggs(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::OVIPAROUS)
}

/// Regenerates hit points? (`M1_REGEN`)
pub fn regenerates(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::REGEN)
}

/// Can see invisible creatures? (`M1_SEE_INVIS`)
pub fn can_see_invisible(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::SEE_INVIS)
}

/// Can teleport? (`M1_TPORT`)
pub fn can_teleport(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::TPORT)
}

/// Controls where it teleports? (`M1_TPORT_CNTRL`)
pub fn has_teleport_control(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::TPORT_CNTRL)
}

/// Acidic to eat? (`M1_ACID`)
pub fn is_acidic(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::ACID)
}

/// Poisonous to eat? (`M1_POIS`)
pub fn is_poisonous(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::POIS)
}

/// Eats corpses? (`M1_CARNIVORE`)
pub fn is_carnivorous(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::CARNIVORE)
}

/// Eats fruits? (`M1_HERBIVORE`)
pub fn is_herbivorous(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::HERBIVORE)
}

/// Eats metal? (`M1_METALLIVORE`)
pub fn is_metallivorous(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::METALLIVORE)
}

// ---------------------------------------------------------------------------
// M2 flag predicates
// ---------------------------------------------------------------------------

/// Is walking dead? (`M2_UNDEAD`)
pub fn is_undead(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::UNDEAD)
}

/// Is a lycanthrope? (`M2_WERE`)
pub fn is_were(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::WERE)
}

/// Is a demon? (`M2_DEMON`)
pub fn is_demon(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::DEMON)
}

/// Is a human? (`M2_HUMAN`)
pub fn is_human(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::HUMAN)
}

/// Is an elf? (`M2_ELF`)
pub fn is_elf(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::ELF)
}

/// Is a dwarf? (`M2_DWARF`)
pub fn is_dwarf(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::DWARF)
}

/// Is a gnome? (`M2_GNOME`)
pub fn is_gnome(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::GNOME)
}

/// Is an orc? (`M2_ORC`)
pub fn is_orc(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::ORC)
}

/// Is a giant? (`M2_GIANT`)
pub fn is_giant(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::GIANT)
}

/// Has a proper name? (`M2_PNAME`)
pub fn type_is_pname(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::PNAME)
}

/// Is a lord to its kind? (`M2_LORD`)
pub fn is_lord(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::LORD)
}

/// Is an overlord to its kind? (`M2_PRINCE`)
pub fn is_prince(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::PRINCE)
}

/// Is a minion of a deity? (`M2_MINION`)
pub fn is_minion(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::MINION)
}

/// Is a guard or soldier? (`M2_MERC`)
pub fn is_mercenary(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::MERC)
}

/// Is a shapeshifting species? (`M2_SHAPESHIFTER`)
pub fn is_shapeshifter(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::SHAPESHIFTER)
}

/// Is always male? (`M2_MALE`)
pub fn is_male(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::MALE)
}

/// Is always female? (`M2_FEMALE`)
pub fn is_female(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::FEMALE)
}

/// Is neither male nor female? (`M2_NEUTER`)
pub fn is_neuter(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NEUTER)
}

/// Always starts hostile? (`M2_HOSTILE`)
pub fn is_always_hostile(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::HOSTILE)
}

/// Always starts peaceful? (`M2_PEACEFUL`)
pub fn is_always_peaceful(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::PEACEFUL)
}

/// Can be tamed by feeding? (`M2_DOMESTIC`)
pub fn is_domestic(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::DOMESTIC)
}

/// Wanders randomly? (`M2_WANDER`)
pub fn is_wanderer(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::WANDER)
}

/// Follows you to other levels? (`M2_STALK`)
pub fn is_stalker(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::STALK)
}

/// Extra-nasty monster? (`M2_NASTY`)
pub fn is_nasty(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NASTY)
}

/// Is strong? (`M2_STRONG`)
pub fn is_strong(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::STRONG)
}

/// Throws boulders? (`M2_ROCKTHROW`)
pub fn throws_rocks(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::ROCKTHROW)
}

/// Likes gold? (`M2_GREEDY`)
pub fn likes_gold(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::GREEDY)
}

/// Likes gems? (`M2_JEWELS`)
pub fn likes_gems(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::JEWELS)
}

/// Picks up weapons and food? (`M2_COLLECT`)
pub fn likes_objs(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::COLLECT)
}

/// Picks up magic items? (`M2_MAGIC`)
pub fn likes_magic(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::MAGIC)
}

/// Players may not polymorph into this? (`M2_NOPOLY`)
pub fn no_poly(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::NOPOLY)
}

// ---------------------------------------------------------------------------
// M3 flag predicates
// ---------------------------------------------------------------------------

/// Wants something of value (quest artifacts, etc.)? (`M3_COVETOUS`)
pub fn is_covetous(mon: &MonsterDef) -> bool {
    mon.flags.intersects(MonsterFlags::COVETOUS)
}

/// Has infravision? (`M3_INFRAVISION`)
pub fn has_infravision(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::INFRAVISION)
}

/// Visible by infravision? (`M3_INFRAVISIBLE`)
pub fn is_infravisible(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::INFRAVISIBLE)
}

/// Moves monsters out of its way? (`M3_DISPLACES`)
pub fn is_displacer(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::DISPLACES)
}

/// Waits to see you or get attacked? (`M3_WAITFORU`)
pub fn waits_for_you(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::WAITFORU)
}

/// Lets you close unless attacked? (`M3_CLOSE`)
pub fn lets_you_close(mon: &MonsterDef) -> bool {
    mon.flags.contains(MonsterFlags::CLOSE)
}

// ---------------------------------------------------------------------------
// Size predicates
// ---------------------------------------------------------------------------

/// Smaller than `MZ_SMALL` (i.e., tiny).
pub fn verysmall(mon: &MonsterDef) -> bool {
    (mon.size as u8) < (MonsterSize::Small as u8)
}

/// At least `MZ_LARGE`.
pub fn bigmonst(mon: &MonsterDef) -> bool {
    (mon.size as u8) >= (MonsterSize::Large as u8)
}

// ---------------------------------------------------------------------------
// Resistance predicates
// ---------------------------------------------------------------------------

/// Resists fire?
pub fn resists_fire(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::FIRE)
}

/// Resists cold?
pub fn resists_cold(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::COLD)
}

/// Resists sleep?
pub fn resists_sleep(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::SLEEP)
}

/// Resists disintegration?
pub fn resists_disintegration(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::DISINTEGRATE)
}

/// Resists electricity?
pub fn resists_electricity(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::SHOCK)
}

/// Resists poison?
pub fn resists_poison(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::POISON)
}

/// Resists acid?
pub fn resists_acid(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::ACID)
}

/// Resists petrification?
pub fn resists_stone(mon: &MonsterDef) -> bool {
    mon.resistances.contains(ResistanceSet::STONE)
}

// ---------------------------------------------------------------------------
// Compound predicates
// ---------------------------------------------------------------------------

/// Cannot drown (swimmer, amphibious, or breathless).
pub fn cant_drown(mon: &MonsterDef) -> bool {
    is_swimmer(mon) || is_amphibious(mon) || is_breathless(mon)
}

/// Non-corporeal — ghosts (symbol `S_GHOST` is character `' '` but we check `X`).
/// In NetHack C: `noncorporeal(ptr) ((ptr)->mlet == S_GHOST)`.
pub fn is_noncorporeal(mon: &MonsterDef) -> bool {
    mon.symbol == ' '
}

/// Not grounded — can fly, float, or cling.
pub fn is_not_grounded(mon: &MonsterDef) -> bool {
    is_flyer(mon) || is_clinger(mon)
}

/// Is a player-like monster (humanoid with hands).
pub fn is_player_like(mon: &MonsterDef) -> bool {
    is_humanoid(mon) && !has_no_hands(mon)
}

/// Can pick up items (has hands and is not notake).
pub fn can_pickup(mon: &MonsterDef) -> bool {
    !is_notake(mon) && !has_no_hands(mon)
}

/// Vulnerable to silver (undead, demon, or were-creature).
pub fn hates_silver(mon: &MonsterDef) -> bool {
    is_undead(mon) || is_demon(mon) || is_were(mon)
}

/// Can open doors (humanoid with hands and not mindless).
pub fn can_open_doors(mon: &MonsterDef) -> bool {
    is_humanoid(mon) && !has_no_hands(mon) && !is_mindless(mon)
}

/// Cannot wield weapons (no hands or very small).
pub fn cant_wield(mon: &MonsterDef) -> bool {
    has_no_hands(mon) || verysmall(mon)
}

/// Is a normal (non-lord, non-prince) demon.
pub fn is_ndemon(mon: &MonsterDef) -> bool {
    is_demon(mon) && !is_lord(mon) && !is_prince(mon)
}

/// Is a demon lord.
pub fn is_dlord(mon: &MonsterDef) -> bool {
    is_demon(mon) && is_lord(mon)
}

/// Is a demon prince.
pub fn is_dprince(mon: &MonsterDef) -> bool {
    is_demon(mon) && is_prince(mon)
}

/// Passes through rocks (phases through walls and is solid).
pub fn passes_rocks(mon: &MonsterDef) -> bool {
    passes_walls(mon) && !is_unsolid(mon)
}

/// Makes no sound.
pub fn is_silent(mon: &MonsterDef) -> bool {
    mon.sound == nethack_babel_data::schema::MonsterSound::Silent
}

/// Is nonliving (undead or golem-like).
pub fn is_nonliving(mon: &MonsterDef) -> bool {
    is_undead(mon) || mon.symbol == '\'' // S_GOLEM
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use nethack_babel_data::schema::*;

    fn make_mon(flags: MonsterFlags) -> MonsterDef {
        make_mon_full(flags, ResistanceSet::empty(), MonsterSize::Medium, 'h', MonsterSound::Silent)
    }

    fn make_mon_full(
        flags: MonsterFlags,
        resistances: ResistanceSet,
        size: MonsterSize,
        symbol: char,
        sound: MonsterSound,
    ) -> MonsterDef {
        use arrayvec::ArrayVec;
        MonsterDef {
            id: MonsterId(0),
            names: MonsterNames {
                male: "test monster".to_string(),
                female: None,
            },
            symbol,
            color: Color::White,
            base_level: 1,
            speed: 12,
            armor_class: 10,
            magic_resistance: 0,
            alignment: 0,
            difficulty: 1,
            attacks: ArrayVec::new(),
            geno_flags: GenoFlags::empty(),
            frequency: 1,
            corpse_weight: 100,
            corpse_nutrition: 100,
            sound,
            size,
            resistances,
            conveys: ResistanceSet::empty(),
            flags,
        }
    }

    // ---- M1 flag tests ----

    #[test]
    fn test_is_flyer() {
        assert!(is_flyer(&make_mon(MonsterFlags::FLY)));
        assert!(!is_flyer(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_is_swimmer() {
        assert!(is_swimmer(&make_mon(MonsterFlags::SWIM)));
        assert!(!is_swimmer(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_is_amorphous() {
        assert!(is_amorphous(&make_mon(MonsterFlags::AMORPHOUS)));
        assert!(!is_amorphous(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_passes_walls() {
        assert!(passes_walls(&make_mon(MonsterFlags::WALLWALK)));
        assert!(!passes_walls(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_is_tunneler() {
        assert!(is_tunneler(&make_mon(MonsterFlags::TUNNEL)));
        assert!(!is_tunneler(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_needspick() {
        assert!(needspick(&make_mon(MonsterFlags::NEEDPICK)));
        assert!(!needspick(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_hides_under() {
        assert!(hides_under(&make_mon(MonsterFlags::CONCEAL)));
        assert!(!hides_under(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_has_eyes() {
        assert!(has_eyes(&make_mon(MonsterFlags::empty())));
        assert!(!has_eyes(&make_mon(MonsterFlags::NOEYES)));
        assert!(has_no_eyes(&make_mon(MonsterFlags::NOEYES)));
    }

    #[test]
    fn test_has_hands_and_limbs() {
        let no_hands = make_mon(MonsterFlags::NOHANDS);
        assert!(has_no_hands(&no_hands));
        let no_limbs = make_mon(MonsterFlags::NOLIMBS);
        assert!(has_no_limbs(&no_limbs));
        assert!(has_no_hands(&no_limbs)); // NOLIMBS includes NOHANDS
    }

    #[test]
    fn test_has_head() {
        assert!(has_head(&make_mon(MonsterFlags::empty())));
        assert!(!has_head(&make_mon(MonsterFlags::NOHEAD)));
        assert!(has_no_head(&make_mon(MonsterFlags::NOHEAD)));
    }

    #[test]
    fn test_is_mindless() {
        assert!(is_mindless(&make_mon(MonsterFlags::MINDLESS)));
        assert!(!is_mindless(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_body_shape() {
        assert!(is_humanoid(&make_mon(MonsterFlags::HUMANOID)));
        assert!(is_animal(&make_mon(MonsterFlags::ANIMAL)));
        assert!(is_slithy(&make_mon(MonsterFlags::SLITHY)));
    }

    #[test]
    fn test_regenerates() {
        assert!(regenerates(&make_mon(MonsterFlags::REGEN)));
        assert!(!regenerates(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_vision_and_teleport() {
        assert!(can_see_invisible(&make_mon(MonsterFlags::SEE_INVIS)));
        assert!(can_teleport(&make_mon(MonsterFlags::TPORT)));
        assert!(has_teleport_control(&make_mon(MonsterFlags::TPORT_CNTRL)));
    }

    #[test]
    fn test_dietary() {
        assert!(is_acidic(&make_mon(MonsterFlags::ACID)));
        assert!(is_poisonous(&make_mon(MonsterFlags::POIS)));
        assert!(is_carnivorous(&make_mon(MonsterFlags::CARNIVORE)));
        assert!(is_herbivorous(&make_mon(MonsterFlags::HERBIVORE)));
        assert!(is_metallivorous(&make_mon(MonsterFlags::METALLIVORE)));
    }

    // ---- M2 flag tests ----

    #[test]
    fn test_is_undead() {
        assert!(is_undead(&make_mon(MonsterFlags::UNDEAD)));
        assert!(!is_undead(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_is_demon() {
        assert!(is_demon(&make_mon(MonsterFlags::DEMON)));
        assert!(!is_demon(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_race_flags() {
        assert!(is_human(&make_mon(MonsterFlags::HUMAN)));
        assert!(is_elf(&make_mon(MonsterFlags::ELF)));
        assert!(is_dwarf(&make_mon(MonsterFlags::DWARF)));
        assert!(is_gnome(&make_mon(MonsterFlags::GNOME)));
        assert!(is_orc(&make_mon(MonsterFlags::ORC)));
        assert!(is_giant(&make_mon(MonsterFlags::GIANT)));
    }

    #[test]
    fn test_disposition_flags() {
        assert!(is_always_hostile(&make_mon(MonsterFlags::HOSTILE)));
        assert!(is_always_peaceful(&make_mon(MonsterFlags::PEACEFUL)));
        assert!(is_domestic(&make_mon(MonsterFlags::DOMESTIC)));
        assert!(is_wanderer(&make_mon(MonsterFlags::WANDER)));
        assert!(is_stalker(&make_mon(MonsterFlags::STALK)));
        assert!(is_nasty(&make_mon(MonsterFlags::NASTY)));
    }

    #[test]
    fn test_hierarchy_flags() {
        assert!(type_is_pname(&make_mon(MonsterFlags::PNAME)));
        assert!(is_lord(&make_mon(MonsterFlags::LORD)));
        assert!(is_prince(&make_mon(MonsterFlags::PRINCE)));
        assert!(is_minion(&make_mon(MonsterFlags::MINION)));
    }

    #[test]
    fn test_collection_flags() {
        assert!(likes_gold(&make_mon(MonsterFlags::GREEDY)));
        assert!(likes_gems(&make_mon(MonsterFlags::JEWELS)));
        assert!(likes_objs(&make_mon(MonsterFlags::COLLECT)));
        assert!(likes_magic(&make_mon(MonsterFlags::MAGIC)));
    }

    // ---- M3 flag tests ----

    #[test]
    fn test_is_covetous() {
        assert!(is_covetous(&make_mon(MonsterFlags::WANTSAMUL)));
        assert!(is_covetous(&make_mon(MonsterFlags::WANTSBELL)));
        assert!(is_covetous(&make_mon(MonsterFlags::WANTSBOOK)));
        assert!(is_covetous(&make_mon(MonsterFlags::WANTSCAND)));
        assert!(is_covetous(&make_mon(MonsterFlags::WANTSARTI)));
        assert!(!is_covetous(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_infravision() {
        assert!(has_infravision(&make_mon(MonsterFlags::INFRAVISION)));
        assert!(is_infravisible(&make_mon(MonsterFlags::INFRAVISIBLE)));
    }

    #[test]
    fn test_is_displacer() {
        assert!(is_displacer(&make_mon(MonsterFlags::DISPLACES)));
        assert!(!is_displacer(&make_mon(MonsterFlags::empty())));
    }

    // ---- Size predicates ----

    #[test]
    fn test_verysmall() {
        let tiny = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Tiny, 'h', MonsterSound::Silent,
        );
        assert!(verysmall(&tiny));
        let small = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Small, 'h', MonsterSound::Silent,
        );
        assert!(!verysmall(&small));
    }

    #[test]
    fn test_bigmonst() {
        let large = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Large, 'h', MonsterSound::Silent,
        );
        assert!(bigmonst(&large));
        let medium = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Medium, 'h', MonsterSound::Silent,
        );
        assert!(!bigmonst(&medium));
    }

    // ---- Resistance tests ----

    #[test]
    fn test_resistances() {
        let fire_mon = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::FIRE,
            MonsterSize::Medium, 'h', MonsterSound::Silent,
        );
        assert!(resists_fire(&fire_mon));
        assert!(!resists_cold(&fire_mon));

        let cold_mon = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::COLD,
            MonsterSize::Medium, 'h', MonsterSound::Silent,
        );
        assert!(resists_cold(&cold_mon));
        assert!(!resists_fire(&cold_mon));

        let multi = make_mon_full(
            MonsterFlags::empty(),
            ResistanceSet::SLEEP | ResistanceSet::SHOCK | ResistanceSet::POISON,
            MonsterSize::Medium, 'h', MonsterSound::Silent,
        );
        assert!(resists_sleep(&multi));
        assert!(resists_electricity(&multi));
        assert!(resists_poison(&multi));
        assert!(!resists_disintegration(&multi));
        assert!(!resists_acid(&multi));
        assert!(!resists_stone(&multi));
    }

    #[test]
    fn test_resists_disint_acid_stone() {
        let mon = make_mon_full(
            MonsterFlags::empty(),
            ResistanceSet::DISINTEGRATE | ResistanceSet::ACID | ResistanceSet::STONE,
            MonsterSize::Medium, 'h', MonsterSound::Silent,
        );
        assert!(resists_disintegration(&mon));
        assert!(resists_acid(&mon));
        assert!(resists_stone(&mon));
    }

    // ---- Compound predicate tests ----

    #[test]
    fn test_is_player_like() {
        // Humanoid with hands = player-like
        let humanoid = make_mon(MonsterFlags::HUMANOID);
        assert!(is_player_like(&humanoid));

        // Humanoid but no hands = not player-like
        let no_hands = make_mon(MonsterFlags::HUMANOID | MonsterFlags::NOHANDS);
        assert!(!is_player_like(&no_hands));

        // Not humanoid = not player-like
        let animal = make_mon(MonsterFlags::ANIMAL);
        assert!(!is_player_like(&animal));
    }

    #[test]
    fn test_can_pickup() {
        // Normal humanoid can pick up
        let normal = make_mon(MonsterFlags::HUMANOID);
        assert!(can_pickup(&normal));

        // Notake cannot
        let notake = make_mon(MonsterFlags::NOTAKE);
        assert!(!can_pickup(&notake));

        // No hands cannot
        let nohands = make_mon(MonsterFlags::NOHANDS);
        assert!(!can_pickup(&nohands));
    }

    #[test]
    fn test_hates_silver() {
        assert!(hates_silver(&make_mon(MonsterFlags::UNDEAD)));
        assert!(hates_silver(&make_mon(MonsterFlags::DEMON)));
        assert!(hates_silver(&make_mon(MonsterFlags::WERE)));
        assert!(!hates_silver(&make_mon(MonsterFlags::HUMAN)));
    }

    #[test]
    fn test_can_open_doors() {
        // Humanoid with hands and mind
        let smart_humanoid = make_mon(MonsterFlags::HUMANOID);
        assert!(can_open_doors(&smart_humanoid));

        // Mindless humanoid cannot
        let mindless = make_mon(MonsterFlags::HUMANOID | MonsterFlags::MINDLESS);
        assert!(!can_open_doors(&mindless));

        // No hands cannot
        let nohands = make_mon(MonsterFlags::HUMANOID | MonsterFlags::NOHANDS);
        assert!(!can_open_doors(&nohands));
    }

    #[test]
    fn test_cant_wield() {
        // No hands
        assert!(cant_wield(&make_mon(MonsterFlags::NOHANDS)));

        // Very small
        let tiny = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Tiny, 'h', MonsterSound::Silent,
        );
        assert!(cant_wield(&tiny));

        // Normal humanoid can wield
        assert!(!cant_wield(&make_mon(MonsterFlags::HUMANOID)));
    }

    #[test]
    fn test_cant_drown() {
        assert!(cant_drown(&make_mon(MonsterFlags::SWIM)));
        assert!(cant_drown(&make_mon(MonsterFlags::AMPHIBIOUS)));
        assert!(cant_drown(&make_mon(MonsterFlags::BREATHLESS)));
        assert!(!cant_drown(&make_mon(MonsterFlags::empty())));
    }

    #[test]
    fn test_demon_hierarchy() {
        let ndemon = make_mon(MonsterFlags::DEMON);
        assert!(is_ndemon(&ndemon));
        assert!(!is_dlord(&ndemon));
        assert!(!is_dprince(&ndemon));

        let dlord = make_mon(MonsterFlags::DEMON | MonsterFlags::LORD);
        assert!(!is_ndemon(&dlord));
        assert!(is_dlord(&dlord));
        assert!(!is_dprince(&dlord));

        let dprince = make_mon(MonsterFlags::DEMON | MonsterFlags::PRINCE);
        assert!(!is_ndemon(&dprince));
        assert!(!is_dlord(&dprince));
        assert!(is_dprince(&dprince));
    }

    #[test]
    fn test_passes_rocks() {
        // Passes walls and solid = passes rocks
        let solid_phaser = make_mon(MonsterFlags::WALLWALK);
        assert!(passes_rocks(&solid_phaser));

        // Passes walls but unsolid = does not pass rocks
        let ghost = make_mon(MonsterFlags::WALLWALK | MonsterFlags::UNSOLID);
        assert!(!passes_rocks(&ghost));
    }

    #[test]
    fn test_is_silent() {
        let silent = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Medium, 'h', MonsterSound::Silent,
        );
        assert!(is_silent(&silent));

        let noisy = make_mon_full(
            MonsterFlags::empty(), ResistanceSet::empty(),
            MonsterSize::Medium, 'h', MonsterSound::Bark,
        );
        assert!(!is_silent(&noisy));
    }
}
