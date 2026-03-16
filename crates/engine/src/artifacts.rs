//! Artifact system for NetHack Babel.
//!
//! Implements all 33 artifacts from NetHack 3.7 with their special attack
//! bonuses, defenses, invoke abilities, touch-blast damage, gift probabilities,
//! and wish mechanics.
//!
//! Reference: `specs/artifact.md` (extracted from artilist.h, artifact.c,
//! uhitm.c, fountain.c, pray.c, objnam.c).

use bitflags::bitflags;
use rand::Rng;

use nethack_babel_data::{Alignment, ArtifactId, DamageType, MonsterFlags, ObjectTypeId, RoleId};

use crate::event::EngineEvent;

// ---------------------------------------------------------------------------
// ArtifactFlags (SPFX)
// ---------------------------------------------------------------------------

bitflags! {
    /// Special-effect flags for artifacts.
    /// Mirrors `SPFX_xxx` defines from `artifact.h`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ArtifactFlags: u32 {
        const NOGEN   = 0x0000_0001;
        const RESTR   = 0x0000_0002;
        const INTEL   = 0x0000_0004;
        const SPEAK   = 0x0000_0008;
        const SEEK    = 0x0000_0010;
        const WARN    = 0x0000_0020;
        const ATTK    = 0x0000_0040;
        const DEFN    = 0x0000_0080;
        const DRLI    = 0x0000_0100;
        const SEARCH  = 0x0000_0200;
        const BEHEAD  = 0x0000_0400;
        const HALRES  = 0x0000_0800;
        const ESP     = 0x0000_1000;
        const STLTH   = 0x0000_2000;
        const REGEN   = 0x0000_4000;
        const EREGEN  = 0x0000_8000;
        const HSPDAM  = 0x0001_0000;
        const HPHDAM  = 0x0002_0000;
        const TCTRL   = 0x0004_0000;
        const LUCK    = 0x0008_0000;
        const DMONS   = 0x0010_0000;
        const DCLAS   = 0x0020_0000;
        const DFLAG1  = 0x0040_0000;
        const DFLAG2  = 0x0080_0000;
        const DALIGN  = 0x0100_0000;
        const XRAY    = 0x0200_0000;
        const REFLECT = 0x0400_0000;
        const PROTECT = 0x0800_0000;

        /// Mask: any of the five bonus-target modes.
        const DBONUS  = Self::DMONS.bits()
                      | Self::DCLAS.bits()
                      | Self::DFLAG1.bits()
                      | Self::DFLAG2.bits()
                      | Self::DALIGN.bits();
    }
}

// ---------------------------------------------------------------------------
// Target specification
// ---------------------------------------------------------------------------

/// What type of monster an artifact's attack bonus applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactTarget {
    /// Bonus applies to all targets (AD_PHYS with no flag-based restriction).
    All,
    /// Bonus applies to monsters matching an M2 flag (e.g. M2_ORC, M2_DEMON).
    MonsterFlag(MonsterFlags),
    /// Bonus applies to monsters with a given map symbol character
    /// (e.g. 'D' for S_DRAGON, 'O' for S_OGRE, 'T' for S_TROLL).
    MonsterSymbol(char),
    /// Bonus applies to monsters of a different alignment than the artifact.
    NonAligned,
}

// ---------------------------------------------------------------------------
// Attack type
// ---------------------------------------------------------------------------

/// The damage type of an artifact's special attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactAttackType {
    Physical,
    Fire,
    Cold,
    Electricity,
    Stun,
    DrainLife,
    Poison,
}

// ---------------------------------------------------------------------------
// Defense specification
// ---------------------------------------------------------------------------

/// Resistances conferred by an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactDefenses {
    /// Defense conferred while wielded/worn (defn.adtyp).
    pub wielded: Option<DamageType>,
    /// Defense conferred while carried (cary.adtyp).
    pub carried: Option<DamageType>,
}

impl ArtifactDefenses {
    const fn none() -> Self {
        Self {
            wielded: None,
            carried: None,
        }
    }
    const fn wielded(dt: DamageType) -> Self {
        Self {
            wielded: Some(dt),
            carried: None,
        }
    }
    const fn carried_only(dt: DamageType) -> Self {
        Self {
            wielded: None,
            carried: Some(dt),
        }
    }
    #[allow(dead_code)]
    const fn both(w: DamageType, c: DamageType) -> Self {
        Self {
            wielded: Some(w),
            carried: Some(c),
        }
    }
}

// ---------------------------------------------------------------------------
// Invoke power
// ---------------------------------------------------------------------------

/// Artifact invocation ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvokePower {
    Invis,
    Levitation,
    Conflict,
    Healing,
    EnergyBoost,
    Enlightening,
    Untrap,
    ChargeObj,
    LevTele,
    CreatePortal,
    CreateAmmo,
    Banish,
    FlingPoison,
    Snowstorm,
    Firestorm,
    BlindingRay,
}

// ---------------------------------------------------------------------------
// Dice expression
// ---------------------------------------------------------------------------

/// A simple dice expression: `count`d`sides`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiceExpr {
    pub count: u8,
    pub sides: u8,
}

impl DiceExpr {
    pub const fn new(count: u8, sides: u8) -> Self {
        Self { count, sides }
    }

    /// Roll this dice expression, returning 0 if sides == 0.
    pub fn roll<R: Rng>(self, rng: &mut R) -> i32 {
        if self.sides == 0 || self.count == 0 {
            return 0;
        }
        let mut total = 0i32;
        for _ in 0..self.count {
            total += rng.random_range(1..=self.sides as i32);
        }
        total
    }
}

// ---------------------------------------------------------------------------
// ArtifactDef
// ---------------------------------------------------------------------------

/// Static definition of an artifact.
/// Mirrors C `struct artifact` from `artilist.h`.
#[derive(Debug, Clone)]
pub struct ArtifactDef {
    pub id: ArtifactId,
    pub name: &'static str,
    /// Base weapon/armor type (index into objects[]).
    pub base_item: ObjectTypeId,
    /// Required alignment (None = any alignment, i.e. A_NONE).
    pub alignment: Option<Alignment>,
    /// Role restriction (None = no restriction).
    pub role: Option<RoleId>,
    /// To-hit bonus dice (damn) vs matching target type.
    pub attack_bonus: DiceExpr,
    /// Extra damage dice (damd) vs matching target type.
    pub damage_bonus: DiceExpr,
    /// Attack element type.
    pub attack_type: ArtifactAttackType,
    /// What type of monster the attack bonus applies to.
    pub target: ArtifactTarget,
    /// Conferred resistances.
    pub defenses: ArtifactDefenses,
    /// Invocation ability.
    pub invoke: Option<InvokePower>,
    /// Special effect flags (spfx).
    pub spfx: ArtifactFlags,
    /// Carry-only special effect flags (cspfx).
    pub cspfx: ArtifactFlags,
}

// ---------------------------------------------------------------------------
// Well-known role IDs (from role.c ordering)
// ---------------------------------------------------------------------------

// These constants map to the canonical role indices used in NetHack.
const ROLE_ARCHEOLOGIST: RoleId = RoleId(0);
const ROLE_BARBARIAN: RoleId = RoleId(1);
const ROLE_CAVE_DWELLER: RoleId = RoleId(2);
const ROLE_HEALER: RoleId = RoleId(3);
const ROLE_KNIGHT: RoleId = RoleId(4);
const ROLE_MONK: RoleId = RoleId(5);
const ROLE_CLERIC: RoleId = RoleId(6);
const ROLE_RANGER: RoleId = RoleId(7);
const ROLE_ROGUE: RoleId = RoleId(8);
const ROLE_SAMURAI: RoleId = RoleId(9);
const ROLE_TOURIST: RoleId = RoleId(10);
const ROLE_VALKYRIE: RoleId = RoleId(11);
const ROLE_WIZARD: RoleId = RoleId(12);

// ---------------------------------------------------------------------------
// Well-known base item ObjectTypeIds (from objects.h ordering)
// ---------------------------------------------------------------------------

// These are placeholder indices matching the typical NetHack objects[] table.
// In a real build they would be generated from data files.
const OBJ_LONG_SWORD: ObjectTypeId = ObjectTypeId(28);
const OBJ_RUNESWORD: ObjectTypeId = ObjectTypeId(29);
const OBJ_WAR_HAMMER: ObjectTypeId = ObjectTypeId(30);
const OBJ_BATTLE_AXE: ObjectTypeId = ObjectTypeId(31);
const OBJ_ORCISH_DAGGER: ObjectTypeId = ObjectTypeId(3);
const OBJ_ELVEN_BROADSWORD: ObjectTypeId = ObjectTypeId(25);
const OBJ_ELVEN_DAGGER: ObjectTypeId = ObjectTypeId(2);
const OBJ_ATHAME: ObjectTypeId = ObjectTypeId(4);
const OBJ_BROADSWORD: ObjectTypeId = ObjectTypeId(24);
const OBJ_SILVER_MACE: ObjectTypeId = ObjectTypeId(32);
const OBJ_SILVER_SABER: ObjectTypeId = ObjectTypeId(33);
const OBJ_KATANA: ObjectTypeId = ObjectTypeId(27);
const OBJ_MORNING_STAR: ObjectTypeId = ObjectTypeId(34);
const OBJ_MACE: ObjectTypeId = ObjectTypeId(35);
const OBJ_QUARTERSTAFF: ObjectTypeId = ObjectTypeId(36);
const OBJ_TSURUGI: ObjectTypeId = ObjectTypeId(37);
const OBJ_BOW: ObjectTypeId = ObjectTypeId(38);
const OBJ_CRYSTAL_BALL: ObjectTypeId = ObjectTypeId(100);
const OBJ_LUCKSTONE: ObjectTypeId = ObjectTypeId(101);
const OBJ_MIRROR: ObjectTypeId = ObjectTypeId(102);
const OBJ_LENSES: ObjectTypeId = ObjectTypeId(103);
const OBJ_HELM_OF_BRILLIANCE: ObjectTypeId = ObjectTypeId(104);
const OBJ_SKELETON_KEY: ObjectTypeId = ObjectTypeId(105);
const OBJ_CREDIT_CARD: ObjectTypeId = ObjectTypeId(106);
const OBJ_AMULET_OF_ESP: ObjectTypeId = ObjectTypeId(107);

// ---------------------------------------------------------------------------
// Quest artifact base flags (all quest artifacts share these)
// ---------------------------------------------------------------------------

const QUEST_BASE: ArtifactFlags = ArtifactFlags::NOGEN
    .union(ArtifactFlags::RESTR)
    .union(ArtifactFlags::INTEL);

// ---------------------------------------------------------------------------
// The 33 artifacts
// ---------------------------------------------------------------------------

/// Complete artifact table: 20 ordinary + 13 quest artifacts.
pub static ARTIFACTS: [ArtifactDef; 33] = [
    // ── 1: Excalibur ───────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(1),
        name: "Excalibur",
        base_item: OBJ_LONG_SWORD,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_KNIGHT),
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(1, 10),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::DrainLife),
        invoke: None,
        spfx: ArtifactFlags::NOGEN
            .union(ArtifactFlags::RESTR)
            .union(ArtifactFlags::SEEK)
            .union(ArtifactFlags::DEFN)
            .union(ArtifactFlags::INTEL)
            .union(ArtifactFlags::SEARCH),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 2: Stormbringer ────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(2),
        name: "Stormbringer",
        base_item: OBJ_RUNESWORD,
        alignment: Some(Alignment::Chaotic),
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(1, 2),
        attack_type: ArtifactAttackType::DrainLife,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::DrainLife),
        invoke: None,
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::ATTK)
            .union(ArtifactFlags::DEFN)
            .union(ArtifactFlags::INTEL)
            .union(ArtifactFlags::DRLI),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 3: Mjollnir ────────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(3),
        name: "Mjollnir",
        base_item: OBJ_WAR_HAMMER,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_VALKYRIE),
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(1, 24),
        attack_type: ArtifactAttackType::Electricity,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::ATTK),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 4: Cleaver ─────────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(4),
        name: "Cleaver",
        base_item: OBJ_BATTLE_AXE,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_BARBARIAN),
        attack_bonus: DiceExpr::new(1, 3),
        damage_bonus: DiceExpr::new(1, 6),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR,
        cspfx: ArtifactFlags::empty(),
    },
    // ── 5: Grimtooth ───────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(5),
        name: "Grimtooth",
        base_item: OBJ_ORCISH_DAGGER,
        alignment: Some(Alignment::Chaotic),
        role: None, // race=Orc, but no role
        attack_bonus: DiceExpr::new(1, 2),
        damage_bonus: DiceExpr::new(1, 6),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::ELF),
        defenses: ArtifactDefenses::wielded(DamageType::Poison),
        invoke: Some(InvokePower::FlingPoison),
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::WARN)
            .union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 6: Orcrist ─────────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(6),
        name: "Orcrist",
        base_item: OBJ_ELVEN_BROADSWORD,
        alignment: Some(Alignment::Chaotic),
        role: None, // race=Elf
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0), // damd=0 -> double base damage
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::ORC),
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::WARN.union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 7: Sting ───────────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(7),
        name: "Sting",
        base_item: OBJ_ELVEN_DAGGER,
        alignment: Some(Alignment::Chaotic),
        role: None, // race=Elf
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0), // damd=0 -> double base damage
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::ORC),
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::WARN.union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 8: Magicbane ───────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(8),
        name: "Magicbane",
        base_item: OBJ_ATHAME,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_WIZARD),
        attack_bonus: DiceExpr::new(1, 3),
        damage_bonus: DiceExpr::new(1, 4),
        attack_type: ArtifactAttackType::Stun,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::MagicMissile),
        invoke: None,
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::ATTK)
            .union(ArtifactFlags::DEFN),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 9: Frost Brand ─────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(9),
        name: "Frost Brand",
        base_item: OBJ_LONG_SWORD,
        alignment: None, // A_NONE
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0), // damd=0 -> double base damage
        attack_type: ArtifactAttackType::Cold,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::Cold),
        invoke: Some(InvokePower::Snowstorm),
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::ATTK)
            .union(ArtifactFlags::DEFN),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 10: Fire Brand ─────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(10),
        name: "Fire Brand",
        base_item: OBJ_LONG_SWORD,
        alignment: None,
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Fire,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::Fire),
        invoke: Some(InvokePower::Firestorm),
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::ATTK)
            .union(ArtifactFlags::DEFN),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 11: Dragonbane ─────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(11),
        name: "Dragonbane",
        base_item: OBJ_BROADSWORD,
        alignment: None,
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterSymbol('D'),
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::DCLAS)
            .union(ArtifactFlags::REFLECT),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 12: Demonbane ──────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(12),
        name: "Demonbane",
        base_item: OBJ_SILVER_MACE,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_CLERIC),
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::DEMON),
        defenses: ArtifactDefenses::none(),
        invoke: Some(InvokePower::Banish),
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 13: Werebane ───────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(13),
        name: "Werebane",
        base_item: OBJ_SILVER_SABER,
        alignment: None,
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::WERE),
        defenses: ArtifactDefenses::wielded(DamageType::Lycanthropy),
        invoke: None,
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 14: Grayswandir ────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(14),
        name: "Grayswandir",
        base_item: OBJ_SILVER_SABER,
        alignment: Some(Alignment::Lawful),
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0), // double base damage vs all
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::HALRES),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 15: Giantslayer ────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(15),
        name: "Giantslayer",
        base_item: OBJ_LONG_SWORD,
        alignment: Some(Alignment::Neutral),
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::GIANT),
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 16: Ogresmasher ────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(16),
        name: "Ogresmasher",
        base_item: OBJ_WAR_HAMMER,
        alignment: None,
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterSymbol('O'),
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::DCLAS),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 17: Trollsbane ─────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(17),
        name: "Trollsbane",
        base_item: OBJ_MORNING_STAR,
        alignment: None,
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterSymbol('T'),
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR
            .union(ArtifactFlags::DCLAS)
            .union(ArtifactFlags::REGEN),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 18: Vorpal Blade ───────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(18),
        name: "Vorpal Blade",
        base_item: OBJ_LONG_SWORD,
        alignment: Some(Alignment::Neutral),
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(1, 1),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::BEHEAD),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 19: Snickersnee ────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(19),
        name: "Snickersnee",
        base_item: OBJ_KATANA,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_SAMURAI),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(1, 8),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: ArtifactFlags::RESTR,
        cspfx: ArtifactFlags::empty(),
    },
    // ── 20: Sunsword ───────────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(20),
        name: "Sunsword",
        base_item: OBJ_LONG_SWORD,
        alignment: Some(Alignment::Lawful),
        role: None,
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::UNDEAD),
        defenses: ArtifactDefenses::wielded(DamageType::Blind),
        invoke: Some(InvokePower::BlindingRay),
        spfx: ArtifactFlags::RESTR.union(ArtifactFlags::DFLAG2),
        cspfx: ArtifactFlags::empty(),
    },
    // ────────────────────────────────────────────────────────────
    // Quest artifacts (21..33)
    // All have NOGEN | RESTR | INTEL as base spfx, gen_spe=0, gift_value=12
    // ────────────────────────────────────────────────────────────

    // ── 21: The Orb of Detection ───────────────────────────────
    ArtifactDef {
        id: ArtifactId(21),
        name: "The Orb of Detection",
        base_item: OBJ_CRYSTAL_BALL,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_ARCHEOLOGIST),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::carried_only(DamageType::MagicMissile),
        invoke: Some(InvokePower::Invis),
        spfx: QUEST_BASE,
        cspfx: ArtifactFlags::ESP.union(ArtifactFlags::HSPDAM),
    },
    // ── 22: The Heart of Ahriman ───────────────────────────────
    ArtifactDef {
        id: ArtifactId(22),
        name: "The Heart of Ahriman",
        base_item: OBJ_LUCKSTONE,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_BARBARIAN),
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: Some(InvokePower::Levitation),
        spfx: QUEST_BASE,
        cspfx: ArtifactFlags::STLTH,
    },
    // ── 23: The Sceptre of Might ───────────────────────────────
    ArtifactDef {
        id: ArtifactId(23),
        name: "The Sceptre of Might",
        base_item: OBJ_MACE,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_CAVE_DWELLER),
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::NonAligned,
        defenses: ArtifactDefenses::wielded(DamageType::MagicMissile),
        invoke: Some(InvokePower::Conflict),
        spfx: QUEST_BASE.union(ArtifactFlags::DALIGN),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 24: The Staff of Aesculapius ───────────────────────────
    ArtifactDef {
        id: ArtifactId(24),
        name: "The Staff of Aesculapius",
        base_item: OBJ_QUARTERSTAFF,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_HEALER),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::DrainLife,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::DrainLife),
        invoke: Some(InvokePower::Healing),
        spfx: QUEST_BASE
            .union(ArtifactFlags::ATTK)
            .union(ArtifactFlags::DRLI)
            .union(ArtifactFlags::REGEN),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 25: The Magic Mirror of Merlin ─────────────────────────
    ArtifactDef {
        id: ArtifactId(25),
        name: "The Magic Mirror of Merlin",
        base_item: OBJ_MIRROR,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_KNIGHT),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::carried_only(DamageType::MagicMissile),
        invoke: None,
        spfx: QUEST_BASE.union(ArtifactFlags::SPEAK),
        cspfx: ArtifactFlags::ESP,
    },
    // ── 26: The Eyes of the Overworld ──────────────────────────
    ArtifactDef {
        id: ArtifactId(26),
        name: "The Eyes of the Overworld",
        base_item: OBJ_LENSES,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_MONK),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::MagicMissile),
        invoke: Some(InvokePower::Enlightening),
        spfx: QUEST_BASE.union(ArtifactFlags::XRAY),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 27: The Mitre of Holiness ──────────────────────────────
    ArtifactDef {
        id: ArtifactId(27),
        name: "The Mitre of Holiness",
        base_item: OBJ_HELM_OF_BRILLIANCE,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_CLERIC),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::MonsterFlag(MonsterFlags::UNDEAD),
        defenses: ArtifactDefenses::carried_only(DamageType::Fire),
        invoke: Some(InvokePower::EnergyBoost),
        spfx: QUEST_BASE
            .union(ArtifactFlags::DFLAG2)
            .union(ArtifactFlags::PROTECT),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 28: The Longbow of Diana ───────────────────────────────
    ArtifactDef {
        id: ArtifactId(28),
        name: "The Longbow of Diana",
        base_item: OBJ_BOW,
        alignment: Some(Alignment::Chaotic),
        role: Some(ROLE_RANGER),
        attack_bonus: DiceExpr::new(1, 5),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: Some(InvokePower::CreateAmmo),
        spfx: QUEST_BASE.union(ArtifactFlags::REFLECT),
        cspfx: ArtifactFlags::ESP,
    },
    // ── 29: The Master Key of Thievery ─────────────────────────
    ArtifactDef {
        id: ArtifactId(29),
        name: "The Master Key of Thievery",
        base_item: OBJ_SKELETON_KEY,
        alignment: Some(Alignment::Chaotic),
        role: Some(ROLE_ROGUE),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: Some(InvokePower::Untrap),
        spfx: QUEST_BASE.union(ArtifactFlags::SPEAK),
        cspfx: ArtifactFlags::WARN
            .union(ArtifactFlags::TCTRL)
            .union(ArtifactFlags::HPHDAM),
    },
    // ── 30: The Tsurugi of Muramasa ────────────────────────────
    ArtifactDef {
        id: ArtifactId(30),
        name: "The Tsurugi of Muramasa",
        base_item: OBJ_TSURUGI,
        alignment: Some(Alignment::Lawful),
        role: Some(ROLE_SAMURAI),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(1, 8),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: None,
        spfx: QUEST_BASE
            .union(ArtifactFlags::BEHEAD)
            .union(ArtifactFlags::LUCK)
            .union(ArtifactFlags::PROTECT),
        cspfx: ArtifactFlags::empty(),
    },
    // ── 31: The Platinum Yendorian Express Card ────────────────
    ArtifactDef {
        id: ArtifactId(31),
        name: "The Platinum Yendorian Express Card",
        base_item: OBJ_CREDIT_CARD,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_TOURIST),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::carried_only(DamageType::MagicMissile),
        invoke: Some(InvokePower::ChargeObj),
        spfx: QUEST_BASE.union(ArtifactFlags::DEFN),
        cspfx: ArtifactFlags::ESP.union(ArtifactFlags::HSPDAM),
    },
    // ── 32: The Orb of Fate ────────────────────────────────────
    ArtifactDef {
        id: ArtifactId(32),
        name: "The Orb of Fate",
        base_item: OBJ_CRYSTAL_BALL,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_VALKYRIE),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::none(),
        invoke: Some(InvokePower::LevTele),
        spfx: QUEST_BASE.union(ArtifactFlags::LUCK),
        cspfx: ArtifactFlags::WARN
            .union(ArtifactFlags::HSPDAM)
            .union(ArtifactFlags::HPHDAM),
    },
    // ── 33: The Eye of the Aethiopica ──────────────────────────
    ArtifactDef {
        id: ArtifactId(33),
        name: "The Eye of the Aethiopica",
        base_item: OBJ_AMULET_OF_ESP,
        alignment: Some(Alignment::Neutral),
        role: Some(ROLE_WIZARD),
        attack_bonus: DiceExpr::new(0, 0),
        damage_bonus: DiceExpr::new(0, 0),
        attack_type: ArtifactAttackType::Physical,
        target: ArtifactTarget::All,
        defenses: ArtifactDefenses::wielded(DamageType::MagicMissile),
        invoke: Some(InvokePower::CreatePortal),
        spfx: QUEST_BASE,
        cspfx: ArtifactFlags::EREGEN.union(ArtifactFlags::HSPDAM),
    },
];

// ---------------------------------------------------------------------------
// Lookup helpers
// ---------------------------------------------------------------------------

/// Look up an artifact definition by its ID.
pub fn get_artifact(id: ArtifactId) -> Option<&'static ArtifactDef> {
    ARTIFACTS.iter().find(|a| a.id == id)
}

/// Look up an artifact definition by name (case-sensitive).
pub fn find_artifact_by_name(name: &str) -> Option<&'static ArtifactDef> {
    ARTIFACTS.iter().find(|a| a.name == name)
}

/// Returns `true` if the artifact is a quest artifact (has NOGEN | RESTR | INTEL
/// and a role restriction and id >= 21).
pub fn is_quest_artifact(def: &ArtifactDef) -> bool {
    def.spfx.contains(ArtifactFlags::NOGEN)
        && def.spfx.contains(ArtifactFlags::RESTR)
        && def.spfx.contains(ArtifactFlags::INTEL)
        && def.role.is_some()
        && def.id.0 >= 21
}

// ---------------------------------------------------------------------------
// Defender info (abstracted for pure-function testing)
// ---------------------------------------------------------------------------

/// Information about a defender relevant to artifact bonus checks.
#[derive(Debug, Clone)]
pub struct DefenderInfo {
    /// Monster M1/M2/M3 flags.
    pub flags: MonsterFlags,
    /// Monster map symbol character (e.g. 'D', 'O', 'T').
    pub symbol: char,
    /// Monster alignment.
    pub alignment: Option<Alignment>,
    /// Whether the monster resists fire.
    pub resists_fire: bool,
    /// Whether the monster resists cold.
    pub resists_cold: bool,
    /// Whether the monster resists electricity.
    pub resists_elec: bool,
    /// Whether the monster resists magic.
    pub resists_magic: bool,
    /// Whether the monster resists drain life.
    pub resists_drain: bool,
    /// Whether the monster resists stun (always false for most).
    pub resists_stun: bool,
    /// Whether the monster resists poison.
    pub resists_poison: bool,
}

impl DefenderInfo {
    /// Create a minimal defender with no resistances and no flags.
    pub fn basic(symbol: char) -> Self {
        Self {
            flags: MonsterFlags::empty(),
            symbol,
            alignment: None,
            resists_fire: false,
            resists_cold: false,
            resists_elec: false,
            resists_magic: false,
            resists_drain: false,
            resists_stun: false,
            resists_poison: false,
        }
    }
}

// ---------------------------------------------------------------------------
// spec_applies — check if an artifact's attack bonus applies to a target
// ---------------------------------------------------------------------------

/// Check if an artifact's special attack/damage applies to the given defender.
///
/// Follows the logic from `spec_applies()` in `artifact.c`.
pub fn spec_applies(artifact: &ArtifactDef, defender: &DefenderInfo) -> bool {
    let has_dbonus = artifact.spfx.intersects(ArtifactFlags::DBONUS);
    let has_attk = artifact.spfx.contains(ArtifactFlags::ATTK);

    if !has_dbonus && !has_attk {
        // No special bonus flags: only physical attacks apply.
        return artifact.attack_type == ArtifactAttackType::Physical;
    }

    // Check target-type match (DBONUS flags).
    if artifact.spfx.contains(ArtifactFlags::DCLAS) {
        // Match by monster symbol.
        if let ArtifactTarget::MonsterSymbol(sym) = artifact.target
            && defender.symbol != sym
        {
            return false;
        }
    }

    if artifact.spfx.contains(ArtifactFlags::DFLAG2) {
        // Match by M2 flag.
        if let ArtifactTarget::MonsterFlag(flag) = artifact.target
            && !defender.flags.intersects(flag)
        {
            return false;
        }
    }

    if artifact.spfx.contains(ArtifactFlags::DALIGN) {
        // Match non-aligned targets.
        if let (Some(art_align), Some(def_align)) = (artifact.alignment, defender.alignment)
            && art_align == def_align
        {
            return false;
        }
    }

    // If ATTK, check element resistance.
    if has_attk {
        match artifact.attack_type {
            ArtifactAttackType::Fire => {
                if defender.resists_fire {
                    return false;
                }
            }
            ArtifactAttackType::Cold => {
                if defender.resists_cold {
                    return false;
                }
            }
            ArtifactAttackType::Electricity => {
                if defender.resists_elec {
                    return false;
                }
            }
            ArtifactAttackType::Stun => {
                if defender.resists_magic {
                    return false;
                }
            }
            ArtifactAttackType::DrainLife => {
                if defender.resists_drain {
                    return false;
                }
            }
            ArtifactAttackType::Poison => {
                if defender.resists_poison {
                    return false;
                }
            }
            ArtifactAttackType::Physical => {}
        }
    }

    true
}

// ---------------------------------------------------------------------------
// spec_abon — attack (to-hit) bonus
// ---------------------------------------------------------------------------

/// Calculate the artifact's to-hit bonus against the given defender.
///
/// Returns `rnd(damn)` if the artifact's attack applies, else 0.
/// Matches the C `spec_abon()` logic.
pub fn spec_abon<R: Rng>(artifact: &ArtifactDef, defender: &DefenderInfo, rng: &mut R) -> i8 {
    if artifact.attack_bonus.sides > 0 && spec_applies(artifact, defender) {
        rng.random_range(1..=artifact.attack_bonus.sides as i8)
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// spec_dbon — damage bonus
// ---------------------------------------------------------------------------

/// Calculate the artifact's extra damage against the given defender.
///
/// `base_weapon_damage` is the already-computed base weapon damage (for the
/// damd==0 doubling case).
///
/// Matches the C `spec_dbon()` logic, including the Grimtooth special case.
pub fn spec_dbon<R: Rng>(
    artifact: &ArtifactDef,
    defender: &DefenderInfo,
    base_weapon_damage: i32,
    rng: &mut R,
) -> i32 {
    // No attack at all?
    if artifact.attack_type == ArtifactAttackType::Physical
        && artifact.attack_bonus.sides == 0
        && artifact.damage_bonus.sides == 0
    {
        return 0;
    }

    // Grimtooth special case: always applies regardless of spec_applies.
    let applies = if artifact.id == ArtifactId(5) {
        // Grimtooth
        true
    } else {
        spec_applies(artifact, defender)
    };

    if applies {
        if artifact.damage_bonus.sides > 0 {
            // Roll damage dice.
            rng.random_range(1..=artifact.damage_bonus.sides as i32)
        } else {
            // damd == 0 means "double base damage" — return max(base, 1).
            base_weapon_damage.max(1)
        }
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// touch_artifact — blast damage for cross-aligned pickup
// ---------------------------------------------------------------------------

/// Information about the entity attempting to touch an artifact.
#[derive(Debug, Clone)]
pub struct ToucherInfo {
    /// The toucher's alignment.
    pub alignment: Alignment,
    /// The toucher's role (if any).
    pub role: Option<RoleId>,
    /// Whether the toucher has Antimagic resistance.
    pub has_antimagic: bool,
    /// The toucher's alignment record (negative = bad alignment).
    pub alignment_record: i32,
}

/// Compute the blast damage when an entity tries to touch (pick up / wield)
/// an artifact they shouldn't.
///
/// Returns a list of engine events (damage, messages) and the blast damage
/// amount. Returns empty events if no blast occurs.
pub fn touch_artifact<R: Rng>(
    artifact: &ArtifactDef,
    toucher: &ToucherInfo,
    rng: &mut R,
) -> (Vec<EngineEvent>, i32) {
    let mut events = Vec::new();

    let self_willed = artifact.spfx.contains(ArtifactFlags::INTEL);

    // Check alignment mismatch.
    let badalign = if artifact.spfx.contains(ArtifactFlags::RESTR) {
        match artifact.alignment {
            Some(art_align) => art_align != toucher.alignment || toucher.alignment_record < 0,
            None => false, // A_NONE artifacts never blast for alignment
        }
    } else {
        false
    };

    // Check class mismatch (for INTEL artifacts).
    let badclass = if self_willed {
        match artifact.role {
            Some(role) => role != toucher.role.unwrap_or(RoleId(255)),
            None => false,
        }
    } else {
        false
    };

    // Determine if blast triggers.
    let blast = if (badclass || badalign) && self_willed {
        true
    } else if badalign {
        // Non-INTEL: 25% chance to blast.
        rng.random_range(0..4) == 0
    } else {
        false
    };

    if !blast {
        return (events, 0);
    }

    // Calculate damage: d(Antimagic?2:4, INTEL?10:4)
    let dice_count = if toucher.has_antimagic { 2 } else { 4 };
    let dice_sides = if self_willed { 10 } else { 4 };

    let mut damage = 0i32;
    for _ in 0..dice_count {
        damage += rng.random_range(1..=dice_sides);
    }

    events.push(EngineEvent::msg("artifact-invoke"));

    (events, damage)
}

// ---------------------------------------------------------------------------
// invoke_artifact — invoke an artifact's special power
// ---------------------------------------------------------------------------

/// State needed for invoke cooldown tracking.
#[derive(Debug, Clone)]
pub struct InvokeState {
    /// The turn at which the artifact can next be invoked (obj.age equivalent).
    pub cooldown_until: u32,
}

/// Calculate rnz(i) — NetHack's biased random with expected value ~ i.
///
/// The algorithm: 50% chance of halving i repeatedly, then rn2(i) * 2 if
/// i was never halved, or rn2(i) otherwise.
pub fn rnz<R: Rng>(rng: &mut R, i: u32) -> u32 {
    let val = i;
    let mut x = val;
    let mut tmp = 1000u32;

    // With 50% probability per iteration, halve the value.
    tmp += rng.random_range(0..1000);
    tmp %= 1000;
    while tmp < 500 {
        x >>= 1;
        if x == 0 {
            x = 1;
        }
        tmp = rng.random_range(0..1000);
    }

    // Final random portion.
    if x == val {
        // Never halved: result is [0, 2*x).
        let r = rng.random_range(0..x.max(1));
        r * 2
    } else {
        // Halved at least once: result is [0, x).
        rng.random_range(0..x.max(1))
    }
}

/// Attempt to invoke an artifact's special power.
///
/// Returns engine events describing what happened. The invoke may fail if
/// the artifact is on cooldown.
///
/// `current_turn` is the current game turn.
/// `invoke_state` tracks the per-artifact cooldown.
pub fn invoke_artifact<R: Rng>(
    artifact: &ArtifactDef,
    current_turn: u32,
    invoke_state: &mut InvokeState,
    rng: &mut R,
) -> Vec<EngineEvent> {
    let mut events = Vec::new();

    let power = match artifact.invoke {
        Some(p) => p,
        None => {
            events.push(EngineEvent::msg("artifact-invoke-fail"));
            return events;
        }
    };

    // Check cooldown.
    if invoke_state.cooldown_until > current_turn {
        events.push(EngineEvent::msg("artifact-invoke-fail"));
        // Penalty: extend cooldown by d(3,10).
        let penalty: u32 = (0..3).map(|_| rng.random_range(1..=10u32)).sum();
        invoke_state.cooldown_until += penalty;
        return events;
    }

    // Set cooldown: current_turn + rnz(100).
    invoke_state.cooldown_until = current_turn + rnz(rng, 100);

    // Emit a generic invoke event. In a full implementation, each power
    // would have its own effect resolution.
    events.push(EngineEvent::msg("artifact-invoke"));

    match power {
        InvokePower::Healing => {
            events.push(EngineEvent::msg("artifact-invoke-heal"));
        }
        InvokePower::EnergyBoost => {
            events.push(EngineEvent::msg("artifact-invoke-energy"));
        }
        InvokePower::Enlightening => {
            events.push(EngineEvent::msg("artifact-invoke-enlighten"));
        }
        InvokePower::Conflict => {
            events.push(EngineEvent::msg("artifact-invoke-conflict"));
        }
        InvokePower::Invis => {
            events.push(EngineEvent::msg("artifact-invoke-invisible"));
        }
        InvokePower::Levitation => {
            events.push(EngineEvent::msg("artifact-invoke-levitate"));
        }
        InvokePower::Untrap => {
            events.push(EngineEvent::msg("artifact-invoke-untrap"));
        }
        InvokePower::ChargeObj => {
            events.push(EngineEvent::msg("artifact-invoke-charge"));
        }
        InvokePower::LevTele => {
            events.push(EngineEvent::msg("artifact-invoke-teleport"));
        }
        InvokePower::CreatePortal => {
            events.push(EngineEvent::msg("artifact-invoke-portal"));
        }
        InvokePower::CreateAmmo => {
            events.push(EngineEvent::msg("artifact-invoke-arrows"));
        }
        InvokePower::Banish => {
            events.push(EngineEvent::msg("artifact-invoke-brandish"));
        }
        InvokePower::FlingPoison => {
            events.push(EngineEvent::msg("artifact-invoke-venom"));
        }
        InvokePower::Snowstorm => {
            events.push(EngineEvent::msg("artifact-invoke-cold"));
        }
        InvokePower::Firestorm => {
            events.push(EngineEvent::msg("artifact-invoke-fire"));
        }
        InvokePower::BlindingRay => {
            events.push(EngineEvent::msg("artifact-invoke-light"));
        }
    }

    events
}

// ---------------------------------------------------------------------------
// artifact_gift — sacrifice gift probability
// ---------------------------------------------------------------------------

/// Determine if a sacrifice should produce an artifact gift, and if so which one.
///
/// Probability: `1/(6 + 2*ugifts*nartifacts)`.
///
/// `ugifts` is the number of previous gifts received.
/// `nartifacts` is the number of artifacts that currently exist in the game.
/// `altar_align` is the alignment of the altar being sacrificed at.
/// `player_role` is the player's role.
///
/// Returns `Some(ArtifactId)` if a gift is bestowed, `None` otherwise.
pub fn artifact_gift<R: Rng>(
    ugifts: u32,
    nartifacts: u32,
    altar_align: Alignment,
    player_role: RoleId,
    existing_artifacts: &[ArtifactId],
    rng: &mut R,
) -> Option<ArtifactId> {
    // Probability check: 1/(6 + 2*ugifts*nartifacts).
    let denominator = 6 + 2 * ugifts * nartifacts;
    if rng.random_range(0..denominator.max(1)) != 0 {
        return None;
    }

    // Build candidate list.
    let mut role_artifact: Option<ArtifactId> = None;
    let mut candidates: Vec<ArtifactId> = Vec::new();

    for art in &ARTIFACTS {
        // Skip existing artifacts.
        if existing_artifacts.contains(&art.id) {
            continue;
        }

        // Skip quest artifacts (NOGEN).
        if art.spfx.contains(ArtifactFlags::NOGEN) {
            continue;
        }

        // Check if this is the player's role-specific artifact.
        if let Some(art_role) = art.role
            && art_role == player_role
        {
            role_artifact = Some(art.id);
            break; // Role artifacts take priority.
        }

        // Check alignment compatibility.
        let align_ok = match art.alignment {
            Some(a) => a == altar_align,
            None => true, // A_NONE matches any.
        };

        if align_ok {
            candidates.push(art.id);
        }
    }

    // Role artifact overrides everything.
    if let Some(id) = role_artifact {
        return Some(id);
    }

    // Pick a random candidate.
    if candidates.is_empty() {
        None
    } else {
        let idx = rng.random_range(0..candidates.len());
        Some(candidates[idx])
    }
}

// ---------------------------------------------------------------------------
// try_wish_artifact — wish success probability
// ---------------------------------------------------------------------------

/// Determine if wishing for an artifact succeeds.
///
/// Failure probability: `(N-2)/N` where N = `nartifact_exist` (total number
/// of artifacts that exist in the game, including the one just created).
///
/// Quest artifacts can never be wished for.
pub fn try_wish_artifact<R: Rng>(
    artifact: &ArtifactDef,
    nartifact_exist: u32,
    rng: &mut R,
) -> bool {
    // Quest artifacts: always fail.
    if is_quest_artifact(artifact) {
        return false;
    }

    // If 2 or fewer artifacts exist: always succeeds.
    if nartifact_exist <= 2 {
        return true;
    }

    // rn2(N) > 1 means failure.
    // Success when rn2(N) <= 1, i.e. probability 2/N.
    let roll = rng.random_range(0..nartifact_exist);
    roll <= 1
}

// ---------------------------------------------------------------------------
// Excalibur creation (fountain dip)
// ---------------------------------------------------------------------------

/// Result of attempting to dip a long sword in a fountain for Excalibur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExcaliburResult {
    /// Excalibur was successfully created.
    Success,
    /// Player is not lawful — sword gets cursed instead.
    Cursed,
    /// The RNG roll failed (nothing special happened).
    NoEffect,
    /// Preconditions not met (wrong weapon, too low level, etc.).
    Invalid,
}

/// Attempt to create Excalibur by dipping a long sword in a fountain.
///
/// Requirements:
/// - `weapon_otyp` must be LONG_SWORD
/// - `player_level` >= 5
/// - Excalibur must not already exist
/// - Single, non-artifact weapon
///
/// If Lawful: success creates blessed, erodeproof Excalibur.
/// If not Lawful: sword is cursed, possible spe reduction.
pub fn try_create_excalibur<R: Rng>(
    weapon_otyp: ObjectTypeId,
    player_level: u8,
    player_alignment: Alignment,
    is_knight: bool,
    excalibur_exists: bool,
    rng: &mut R,
) -> ExcaliburResult {
    // Precondition checks.
    if weapon_otyp != OBJ_LONG_SWORD {
        return ExcaliburResult::Invalid;
    }
    if player_level < 5 {
        return ExcaliburResult::Invalid;
    }
    if excalibur_exists {
        return ExcaliburResult::Invalid;
    }

    // Roll check: knight 1/6, others 1/30.
    let chance = if is_knight { 6 } else { 30 };
    if rng.random_range(0..chance) != 0 {
        return ExcaliburResult::NoEffect;
    }

    // Alignment check.
    if player_alignment == Alignment::Lawful {
        ExcaliburResult::Success
    } else {
        ExcaliburResult::Cursed
    }
}

// ---------------------------------------------------------------------------
// Magicbane special effects
// ---------------------------------------------------------------------------

/// The four tiers of Magicbane special effects (highest to lowest priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MagicbaneEffect {
    /// Cancels the target, potentially drains 1 max Pw from player.
    Cancel,
    /// Frightens the target (3 turns flee, 50% resist for monsters).
    Scare,
    /// Stuns the target.
    Stun,
    /// Detects monsters (probing).
    Probe,
}

/// Maximum die roll for Magicbane effects to trigger.
const MB_MAX_DIEROLL: i32 = 8;

/// Determine which Magicbane effect triggers and the extra damage.
///
/// `spe` is the weapon's enchantment level.
/// `dieroll` is the attack die roll (1..20).
/// `spec_dbon_applies` is whether the spec_dbon check passed.
///
/// Returns `(effect, extra_damage)` or `None` if no special effect triggers.
pub fn magicbane_effect<R: Rng>(
    spe: i8,
    dieroll: i32,
    spec_dbon_applies: bool,
    rng: &mut R,
) -> (Option<MagicbaneEffect>, i32) {
    // Only trigger if dieroll is within the effect window.
    let adjusted_dieroll = if spec_dbon_applies {
        dieroll
    } else {
        dieroll + 1
    };

    if adjusted_dieroll > MB_MAX_DIEROLL {
        return (None, 0);
    }

    // Scare threshold.
    let mut scare_dieroll = MB_MAX_DIEROLL / 2; // = 4
    {
        let mut divisor = spe.max(0) as i32 / 3;
        while divisor > 0 {
            scare_dieroll /= 2;
            divisor -= 1;
        }
    }

    // Stun check.
    let stun_threshold = if spec_dbon_applies { 11 } else { 7 };
    let do_stun = (spe.max(0) as i32) < rng.random_range(0..stun_threshold);

    // Accumulate damage and determine effect.
    let mut extra_damage = 0i32;

    // Base: always +1d4 (probe).
    extra_damage += rng.random_range(1..=4);

    if do_stun {
        extra_damage += rng.random_range(1..=4);
    }

    let has_scare = adjusted_dieroll <= scare_dieroll;
    if has_scare {
        extra_damage += rng.random_range(1..=4);
    }

    let has_cancel = adjusted_dieroll <= scare_dieroll / 2;
    if has_cancel {
        extra_damage += rng.random_range(1..=4);
    }

    // Determine the highest-priority effect.
    let effect = if has_cancel {
        Some(MagicbaneEffect::Cancel)
    } else if has_scare {
        Some(MagicbaneEffect::Scare)
    } else if do_stun {
        Some(MagicbaneEffect::Stun)
    } else {
        Some(MagicbaneEffect::Probe)
    };

    (effect, extra_damage)
}

// ---------------------------------------------------------------------------
// Vorpal Blade beheading
// ---------------------------------------------------------------------------

/// Whether Vorpal Blade's beheading activates.
///
/// Triggers on dieroll == 1 or if the target is a Jabberwock (symbol 'J').
pub fn vorpal_triggers(dieroll: i32, target_symbol: char) -> bool {
    dieroll == 1 || target_symbol == 'J'
}

/// Vorpal Blade beheading damage calculation.
///
/// Returns `Some(fatal_damage)` for lethal decapitation, `None` if the
/// target is headless, noncorporeal, or amorphous (no special damage).
///
/// `target_hp` is the target's current HP.
/// `has_head` is whether the target has a head (no NOHEAD flag).
/// `noncorporeal` is whether the target is noncorporeal (UNSOLID).
/// `amorphous` is whether the target is amorphous.
pub fn vorpal_damage(
    target_hp: i32,
    has_head: bool,
    noncorporeal: bool,
    amorphous: bool,
) -> Option<i32> {
    if !has_head {
        return None; // "misses wildly"
    }
    if noncorporeal || amorphous {
        return None; // "slices through neck" but no fatal damage
    }
    // Fatal: 2 * hp + 200
    Some(2 * target_hp + 200)
}

// ---------------------------------------------------------------------------
// Tsurugi bisection
// ---------------------------------------------------------------------------

/// Tsurugi of Muramasa bisection damage.
///
/// Returns the modified damage or fatal damage.
///
/// `is_big` — target is a large (bigmonst) creature.
/// `base_damage` — the already-computed base damage.
/// `target_hp` — target's current HP.
pub fn tsurugi_bisect(is_big: bool, base_damage: i32, target_hp: i32) -> i32 {
    if is_big {
        // Large monster: double damage, not fatal.
        base_damage * 2
    } else {
        // Small monster: instant kill.
        2 * target_hp + 200
    }
}

// ---------------------------------------------------------------------------
// Artifact confers resistance check
// ---------------------------------------------------------------------------

/// Check if an artifact confers a specific defense type when wielded.
pub fn artifact_confers_when_wielded(artifact: &ArtifactDef, dt: DamageType) -> bool {
    artifact.defenses.wielded == Some(dt)
}

/// Check if an artifact confers a specific defense type when carried.
pub fn artifact_confers_when_carried(artifact: &ArtifactDef, dt: DamageType) -> bool {
    artifact.defenses.carried == Some(dt)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed-seed RNG for deterministic tests.
    fn test_rng() -> rand_pcg::Pcg64 {
        use rand::SeedableRng;
        rand_pcg::Pcg64::seed_from_u64(42)
    }

    // ── Test 1: spec_abon gives bonus vs correct target ───────

    #[test]
    fn spec_abon_excalibur_vs_any_target() {
        let mut rng = test_rng();
        let excalibur = get_artifact(ArtifactId(1)).unwrap();

        // Excalibur applies to all targets (AD_PHYS, no DBONUS flag restriction).
        let defender = DefenderInfo::basic('g'); // gnome

        let bonus = spec_abon(excalibur, &defender, &mut rng);
        // Should be in 1..=5 (d5 attack bonus).
        assert!(bonus >= 1 && bonus <= 5, "excalibur abon={}", bonus);
    }

    #[test]
    fn spec_abon_orcrist_vs_orc() {
        let mut rng = test_rng();
        let orcrist = get_artifact(ArtifactId(6)).unwrap();

        let mut orc_defender = DefenderInfo::basic('o');
        orc_defender.flags = MonsterFlags::ORC;

        let bonus = spec_abon(orcrist, &orc_defender, &mut rng);
        assert!(bonus >= 1 && bonus <= 5, "orcrist vs orc abon={}", bonus);
    }

    #[test]
    fn spec_abon_orcrist_vs_non_orc() {
        let mut rng = test_rng();
        let orcrist = get_artifact(ArtifactId(6)).unwrap();

        let elf_defender = DefenderInfo::basic('h'); // not an orc
        let bonus = spec_abon(orcrist, &elf_defender, &mut rng);
        assert_eq!(bonus, 0, "orcrist vs non-orc should be 0");
    }

    // ── Test 2: spec_dbon dice vs matching target ─────────────

    #[test]
    fn spec_dbon_excalibur_rolls_d10() {
        let mut rng = test_rng();
        let excalibur = get_artifact(ArtifactId(1)).unwrap();
        let defender = DefenderInfo::basic('g');

        let damage = spec_dbon(excalibur, &defender, 8, &mut rng);
        assert!(damage >= 1 && damage <= 10, "excalibur dbon={}", damage);
    }

    #[test]
    fn spec_dbon_frost_brand_doubles_base_vs_non_resistant() {
        let mut rng = test_rng();
        let frost_brand = get_artifact(ArtifactId(9)).unwrap();

        let defender = DefenderInfo::basic('g'); // no cold resist
        let base_dmg = 7;
        let damage = spec_dbon(frost_brand, &defender, base_dmg, &mut rng);
        // damd == 0 -> return max(base, 1) = 7 (doubling effect).
        assert_eq!(damage, 7, "frost brand doubles base damage");
    }

    #[test]
    fn spec_dbon_frost_brand_zero_vs_cold_resistant() {
        let mut rng = test_rng();
        let frost_brand = get_artifact(ArtifactId(9)).unwrap();

        let mut defender = DefenderInfo::basic('D');
        defender.resists_cold = true;

        let damage = spec_dbon(frost_brand, &defender, 7, &mut rng);
        assert_eq!(damage, 0, "frost brand vs cold-resistant should be 0");
    }

    // ── Test 3: Grimtooth special case (always applies) ───────

    #[test]
    fn spec_dbon_grimtooth_vs_non_elf() {
        let mut rng = test_rng();
        let grimtooth = get_artifact(ArtifactId(5)).unwrap();

        let defender = DefenderInfo::basic('H'); // hill giant, not an elf
        let damage = spec_dbon(grimtooth, &defender, 4, &mut rng);
        // Grimtooth always applies: d6 damage.
        assert!(damage >= 1 && damage <= 6, "grimtooth dbon={}", damage);
    }

    // ── Test 4: Cross-aligned touch blast damage ──────────────

    #[test]
    fn touch_blast_intel_no_antimagic() {
        let mut rng = test_rng();
        let excalibur = get_artifact(ArtifactId(1)).unwrap();

        // Chaotic wizard trying to touch lawful Excalibur (INTEL).
        let toucher = ToucherInfo {
            alignment: Alignment::Chaotic,
            role: Some(ROLE_WIZARD),
            has_antimagic: false,
            alignment_record: 10,
        };

        let (events, damage) = touch_artifact(excalibur, &toucher, &mut rng);
        // Should blast: d(4, 10) = 4..40.
        assert!(!events.is_empty(), "should produce blast events");
        assert!(
            damage >= 4 && damage <= 40,
            "blast damage should be 4d10: got {}",
            damage
        );
    }

    #[test]
    fn touch_blast_intel_with_antimagic() {
        let mut rng = test_rng();
        let excalibur = get_artifact(ArtifactId(1)).unwrap();

        let toucher = ToucherInfo {
            alignment: Alignment::Chaotic,
            role: Some(ROLE_WIZARD),
            has_antimagic: true,
            alignment_record: 10,
        };

        let (events, damage) = touch_artifact(excalibur, &toucher, &mut rng);
        assert!(!events.is_empty(), "should blast even with antimagic");
        // d(2, 10) = 2..20.
        assert!(
            damage >= 2 && damage <= 20,
            "with antimagic: 2d10, got {}",
            damage
        );
    }

    #[test]
    fn touch_no_blast_matching_alignment() {
        let mut rng = test_rng();
        let excalibur = get_artifact(ArtifactId(1)).unwrap();

        // Lawful knight — should not blast.
        let toucher = ToucherInfo {
            alignment: Alignment::Lawful,
            role: Some(ROLE_KNIGHT),
            has_antimagic: false,
            alignment_record: 10,
        };

        let (events, damage) = touch_artifact(excalibur, &toucher, &mut rng);
        assert_eq!(damage, 0, "matching alignment should not blast");
        assert!(events.is_empty());
    }

    // ── Test 5: Gift probability formula ──────────────────────

    #[test]
    fn gift_probability_first_gift() {
        // With ugifts=0: denominator = 6 + 2*0*N = 6.
        // So ~1/6 chance. We run many trials and check roughly.
        let mut rng = test_rng();
        let mut successes = 0;
        let trials = 6000;

        for _ in 0..trials {
            if artifact_gift(0, 0, Alignment::Lawful, ROLE_KNIGHT, &[], &mut rng).is_some() {
                successes += 1;
            }
        }

        // Expect ~1000 successes (1/6 of 6000). Allow wide margin.
        assert!(
            successes > 500 && successes < 1800,
            "first gift ~1/6 rate: got {}/{}",
            successes,
            trials
        );
    }

    // ── Test 6: Wish probability formula ──────────────────────

    #[test]
    fn wish_always_succeeds_with_2_or_fewer_artifacts() {
        let mut rng = test_rng();
        let grayswandir = get_artifact(ArtifactId(14)).unwrap();

        for n in 1..=2 {
            assert!(
                try_wish_artifact(grayswandir, n, &mut rng),
                "should always succeed with {} artifacts",
                n
            );
        }
    }

    #[test]
    fn wish_never_succeeds_for_quest_artifact() {
        let mut rng = test_rng();
        let orb = get_artifact(ArtifactId(21)).unwrap(); // Orb of Detection

        for _ in 0..100 {
            assert!(
                !try_wish_artifact(orb, 1, &mut rng),
                "quest artifact wish should always fail"
            );
        }
    }

    #[test]
    fn wish_probability_with_3_artifacts() {
        let mut rng = test_rng();
        let grayswandir = get_artifact(ArtifactId(14)).unwrap();
        let mut successes = 0;
        let trials = 3000;

        for _ in 0..trials {
            if try_wish_artifact(grayswandir, 3, &mut rng) {
                successes += 1;
            }
        }

        // Expect ~2/3 success rate = ~2000.
        assert!(
            successes > 1500 && successes < 2500,
            "with 3 artifacts: ~2/3 success rate, got {}/{}",
            successes,
            trials
        );
    }

    // ── Test 7: Excalibur creation ────────────────────────────

    #[test]
    fn excalibur_creation_lawful_knight() {
        let mut rng = test_rng();
        let mut found = false;

        // Try many times since it's 1/6 for a knight.
        for _ in 0..100 {
            let result =
                try_create_excalibur(OBJ_LONG_SWORD, 5, Alignment::Lawful, true, false, &mut rng);
            if result == ExcaliburResult::Success {
                found = true;
                break;
            }
        }
        assert!(found, "lawful knight should eventually create Excalibur");
    }

    #[test]
    fn excalibur_creation_low_level() {
        let mut rng = test_rng();
        let result = try_create_excalibur(
            OBJ_LONG_SWORD,
            4, // too low
            Alignment::Lawful,
            true,
            false,
            &mut rng,
        );
        assert_eq!(result, ExcaliburResult::Invalid);
    }

    #[test]
    fn excalibur_creation_non_lawful() {
        let mut rng = test_rng();
        let mut cursed_found = false;

        for _ in 0..200 {
            let result = try_create_excalibur(
                OBJ_LONG_SWORD,
                5,
                Alignment::Neutral,
                false, // not knight -> 1/30
                false,
                &mut rng,
            );
            if result == ExcaliburResult::Cursed {
                cursed_found = true;
                break;
            }
        }
        assert!(cursed_found, "non-lawful should get cursed sword");
    }

    // ── Test 8: Invoke cooldown prevents rapid use ────────────

    #[test]
    fn invoke_cooldown_prevents_rapid_use() {
        let mut rng = test_rng();
        let frost_brand = get_artifact(ArtifactId(9)).unwrap();
        let mut state = InvokeState { cooldown_until: 0 };

        // First invoke should succeed.
        let events1 = invoke_artifact(frost_brand, 100, &mut state, &mut rng);
        assert!(
            events1.iter().any(
                |e| matches!(e, EngineEvent::Message { key, .. } if key.contains("artifact-invoke"))
            ),
            "first invoke should succeed"
        );

        // Cooldown should now be set beyond turn 100.
        assert!(
            state.cooldown_until > 100,
            "cooldown should be set: {}",
            state.cooldown_until
        );

        // Second invoke at same turn should fail.
        let events2 = invoke_artifact(frost_brand, 101, &mut state, &mut rng);
        assert!(
            events2.iter().any(|e| matches!(e, EngineEvent::Message { key, .. } if key.contains("artifact-invoke-fail"))),
            "rapid invoke should be rejected"
        );
    }

    // ── Test 9: Magicbane special effects ─────────────────────

    #[test]
    fn magicbane_effect_probe_at_high_dieroll() {
        let mut rng = test_rng();
        // dieroll=7 (within MB_MAX_DIEROLL=8), spe=0, applies=true.
        let (effect, dmg) = magicbane_effect(0, 7, true, &mut rng);
        // At dieroll=7, scare_dieroll=4: 7 > 4, so no scare/cancel.
        // Stun is probabilistic. Effect should be Probe or Stun.
        assert!(effect.is_some());
        assert!(dmg > 0);
    }

    #[test]
    fn magicbane_effect_scare_at_low_dieroll() {
        let mut rng = test_rng();
        // dieroll=2, spe=0, scare_dieroll=4: 2 <= 4 -> scare.
        // cancel: 2 <= 4/2=2 -> yes, cancel!
        let (effect, dmg) = magicbane_effect(0, 2, true, &mut rng);
        assert!(
            effect == Some(MagicbaneEffect::Cancel) || effect == Some(MagicbaneEffect::Scare),
            "low dieroll should trigger scare or cancel"
        );
        assert!(dmg >= 3, "should have multiple d4 bonus: {}", dmg);
    }

    #[test]
    fn magicbane_no_effect_beyond_threshold() {
        let mut rng = test_rng();
        // dieroll=9 (> MB_MAX_DIEROLL=8) with applies=true.
        let (effect, dmg) = magicbane_effect(0, 9, true, &mut rng);
        assert!(effect.is_none(), "dieroll > 8 should not trigger");
        assert_eq!(dmg, 0);
    }

    // ── Test 10: Quest artifact restriction by role ───────────

    #[test]
    fn quest_artifact_identified() {
        let orb = get_artifact(ArtifactId(21)).unwrap();
        assert!(
            is_quest_artifact(orb),
            "Orb of Detection is a quest artifact"
        );
        assert_eq!(orb.role, Some(ROLE_ARCHEOLOGIST));
    }

    #[test]
    fn ordinary_artifact_is_not_quest() {
        let excalibur = get_artifact(ArtifactId(1)).unwrap();
        assert!(
            !is_quest_artifact(excalibur),
            "Excalibur is not a quest artifact (id < 21)"
        );
    }

    // ── Test 11: Artifact confers resistance when wielded ─────

    #[test]
    fn excalibur_confers_drain_resistance() {
        let excalibur = get_artifact(ArtifactId(1)).unwrap();
        assert!(artifact_confers_when_wielded(
            excalibur,
            DamageType::DrainLife
        ));
        assert!(!artifact_confers_when_wielded(excalibur, DamageType::Fire));
    }

    #[test]
    fn frost_brand_confers_cold_resistance() {
        let frost = get_artifact(ArtifactId(9)).unwrap();
        assert!(artifact_confers_when_wielded(frost, DamageType::Cold));
    }

    #[test]
    fn orb_of_detection_confers_magic_resistance_when_carried() {
        let orb = get_artifact(ArtifactId(21)).unwrap();
        assert!(artifact_confers_when_carried(orb, DamageType::MagicMissile));
        assert!(!artifact_confers_when_wielded(
            orb,
            DamageType::MagicMissile
        ));
    }

    // ── Test 12: Vorpal Blade beheading ───────────────────────

    #[test]
    fn vorpal_triggers_on_dieroll_1() {
        assert!(vorpal_triggers(1, 'g'));
    }

    #[test]
    fn vorpal_triggers_vs_jabberwock() {
        assert!(vorpal_triggers(15, 'J'));
    }

    #[test]
    fn vorpal_fatal_damage() {
        let dmg = vorpal_damage(50, true, false, false);
        assert_eq!(dmg, Some(300)); // 2*50 + 200
    }

    #[test]
    fn vorpal_no_damage_headless() {
        let dmg = vorpal_damage(50, false, false, false);
        assert_eq!(dmg, None);
    }

    // ── Test 13: Tsurugi bisection ────────────────────────────

    #[test]
    fn tsurugi_bisect_big_monster() {
        let dmg = tsurugi_bisect(true, 10, 50);
        assert_eq!(dmg, 20); // double damage, not fatal
    }

    #[test]
    fn tsurugi_bisect_small_monster() {
        let dmg = tsurugi_bisect(false, 10, 50);
        assert_eq!(dmg, 300); // 2*50 + 200, fatal
    }

    // ── Test 14: All 33 artifacts are present ─────────────────

    #[test]
    fn artifact_table_has_33_entries() {
        assert_eq!(ARTIFACTS.len(), 33);
    }

    #[test]
    fn all_artifacts_have_unique_ids() {
        let mut ids: Vec<u16> = ARTIFACTS.iter().map(|a| a.id.0).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 33);
    }

    // ── Test 15: Lookup helpers ───────────────────────────────

    #[test]
    fn find_by_name() {
        let sb = find_artifact_by_name("Stormbringer").unwrap();
        assert_eq!(sb.id, ArtifactId(2));
    }

    #[test]
    fn find_by_id() {
        let art = get_artifact(ArtifactId(18)).unwrap();
        assert_eq!(art.name, "Vorpal Blade");
    }

    // ── Test 16: DiceExpr roll ────────────────────────────────

    #[test]
    fn dice_expr_roll_zero_sides() {
        let mut rng = test_rng();
        let d = DiceExpr::new(3, 0);
        assert_eq!(d.roll(&mut rng), 0);
    }

    #[test]
    fn dice_expr_roll_range() {
        let mut rng = test_rng();
        let d = DiceExpr::new(2, 6);
        for _ in 0..100 {
            let r = d.roll(&mut rng);
            assert!(r >= 2 && r <= 12, "2d6 out of range: {}", r);
        }
    }

    // ── Test 17: spec_applies with element resistance ─────────

    #[test]
    fn spec_applies_mjollnir_vs_elec_resistant() {
        let mjollnir = get_artifact(ArtifactId(3)).unwrap();
        let mut defender = DefenderInfo::basic('g');
        defender.resists_elec = true;

        assert!(!spec_applies(mjollnir, &defender));
    }

    #[test]
    fn spec_applies_mjollnir_vs_non_resistant() {
        let mjollnir = get_artifact(ArtifactId(3)).unwrap();
        let defender = DefenderInfo::basic('g');

        assert!(spec_applies(mjollnir, &defender));
    }

    // ── Test 18: Sceptre of Might vs non-aligned ──────────────

    #[test]
    fn sceptre_applies_vs_different_alignment() {
        let sceptre = get_artifact(ArtifactId(23)).unwrap();
        let mut defender = DefenderInfo::basic('h');
        defender.alignment = Some(Alignment::Chaotic); // Sceptre is Lawful

        assert!(spec_applies(sceptre, &defender));
    }

    #[test]
    fn sceptre_does_not_apply_vs_same_alignment() {
        let sceptre = get_artifact(ArtifactId(23)).unwrap();
        let mut defender = DefenderInfo::basic('h');
        defender.alignment = Some(Alignment::Lawful); // Same as Sceptre

        assert!(!spec_applies(sceptre, &defender));
    }
}
