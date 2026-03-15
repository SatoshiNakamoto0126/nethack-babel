//! Core game data schema definitions.
//!
//! These structs and enums model the static data tables from the original
//! NetHack C source (permonst, objclass, levl_typ_types, trap_types, etc.)
//! and are loaded from TOML data files at startup.

use arrayvec::ArrayVec;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

/// Helper macro to implement Serialize/Deserialize for bitflags types
/// by converting to/from their underlying integer representation.
macro_rules! impl_serde_for_bitflags {
    ($name:ident, $bits_type:ty) => {
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.bits().serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bits = <$bits_type>::deserialize(deserializer)?;
                $name::from_bits(bits).ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "invalid bits {:#x} for {}",
                        bits,
                        stringify!($name)
                    ))
                })
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Newtypes
// ---------------------------------------------------------------------------

/// Unique identifier for a monster species (index into the `mons[]` table).
/// Corresponds to `PM_xxx` enum values in the C source (`monst.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MonsterId(pub u16);

/// Unique identifier for an object type (index into the `objects[]` table).
/// Corresponds to `enum objects_nums` in `objclass.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectTypeId(pub u16);

/// Unique identifier for an artifact (index into the artifact table).
/// Corresponds to artifact indices from `artilist.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub u16);

/// Dungeon level identifier (branch + depth).
/// Corresponds to `d_level` in `dungeon.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DungeonLevel {
    /// Dungeon branch number.  From `d_level.dnum`.
    pub branch: i16,
    /// Level depth within the branch.  From `d_level.dlevel`.
    pub depth: i16,
}

/// Player role identifier.  Corresponds to role index from `role.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoleId(pub u8);

/// Player race identifier.  Corresponds to race index from `race.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RaceId(pub u8);

/// Gender.  Corresponds to `flags.female` and related gender tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Gender {
    /// Male (0)
    Male = 0,
    /// Female (1)
    Female = 1,
    /// Neuter (2) — for polymorphed forms
    Neuter = 2,
}

/// Alignment type.  Corresponds to `aligntyp` in C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i8)]
pub enum Alignment {
    /// A_CHAOTIC (-1)
    Chaotic = -1,
    /// A_NEUTRAL (0)
    Neutral = 0,
    /// A_LAWFUL (1)
    Lawful = 1,
}

/// Handedness.  Corresponds to `u.uhandedness` in C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Handedness {
    /// Right-handed (0)
    RightHanded = 0,
    /// Left-handed (1)
    LeftHanded = 1,
}

// ---------------------------------------------------------------------------
// Color
// ---------------------------------------------------------------------------

/// Display color.  Maps to CLR_xxx constants from `color.h`.
/// The 16 standard terminal colors used throughout NetHack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Color {
    /// CLR_BLACK (0)
    Black = 0,
    /// CLR_RED (1)
    Red = 1,
    /// CLR_GREEN (2)
    Green = 2,
    /// CLR_BROWN (3)
    Brown = 3,
    /// CLR_BLUE (4)
    Blue = 4,
    /// CLR_MAGENTA (5)
    Magenta = 5,
    /// CLR_CYAN (6)
    Cyan = 6,
    /// CLR_GRAY (7)
    Gray = 7,
    /// NO_COLOR (8)
    NoColor = 8,
    /// CLR_ORANGE (9)
    Orange = 9,
    /// CLR_BRIGHT_GREEN (10)
    BrightGreen = 10,
    /// CLR_YELLOW (11)
    Yellow = 11,
    /// CLR_BRIGHT_BLUE (12)
    BrightBlue = 12,
    /// CLR_BRIGHT_MAGENTA (13)
    BrightMagenta = 13,
    /// CLR_BRIGHT_CYAN (14)
    BrightCyan = 14,
    /// CLR_WHITE (15)
    White = 15,
}

// ---------------------------------------------------------------------------
// Monster definitions
// ---------------------------------------------------------------------------

/// Static definition of a monster species.
/// Mirrors C `struct permonst` from `monst.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterDef {
    /// Unique numeric identifier (`PM_xxx`).
    pub id: MonsterId,
    /// Display names (normal and, if applicable, gendered variants).
    pub names: MonsterNames,
    /// Map symbol character (e.g. `'D'` for dragons).  From `permonst.mlet`.
    pub symbol: char,
    /// Display color.  From `permonst.mcolor`.
    pub color: Color,
    /// Base experience level.  From `permonst.mlevel`.
    pub base_level: i8,
    /// Movement speed; 12 = `NORMAL_SPEED`.  From `permonst.mmove`.
    pub speed: i8,
    /// Natural armor class (lower is better).  From `permonst.ac`.
    pub armor_class: i8,
    /// Magic resistance percentage (0..100).  From `permonst.mr`.
    pub magic_resistance: u8,
    /// Intrinsic alignment (negative = chaotic, positive = lawful).  From `permonst.maligntyp`.
    pub alignment: i8,
    /// Overall difficulty rating.  From `permonst.difficulty`.
    pub difficulty: u8,
    /// Attack descriptors; maximum 6 per the C `NATTK` constant.  From `permonst.mattk[]`.
    pub attacks: ArrayVec<AttackDef, 6>,
    /// Generation and genocide flags.  From `permonst.geno`.
    pub geno_flags: GenoFlags,
    /// Relative generation frequency (1..7).  From low bits of `permonst.geno`.
    pub frequency: u8,
    /// Weight of the corpse in cn (1 cn = 0.1 lb).  From `permonst.cwt`.
    pub corpse_weight: u16,
    /// Nutritional value of the corpse.  From `permonst.cnutrit`.
    pub corpse_nutrition: u16,
    /// Sound type this monster makes.  From `permonst.msound`.
    pub sound: MonsterSound,
    /// Physical size category.  From `permonst.msize`.
    pub size: MonsterSize,
    /// Innate elemental resistances.  From `permonst.mresists`.
    pub resistances: ResistanceSet,
    /// Resistances conveyed by eating this monster.  From `permonst.mconveys`.
    pub conveys: ResistanceSet,
    /// Behavioral and physical flag bits (M1/M2/M3).  From `permonst.mflags1/2/3`.
    pub flags: MonsterFlags,
}

/// Monster display names.
/// The C source stores up to `NUM_MGENDERS` gendered name strings per `permonst`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterNames {
    /// Primary (male/neutral) name.  From `permonst.pmnames[MALE]`.
    pub male: String,
    /// Optional female-specific name (e.g. "priestess").  From `permonst.pmnames[FEMALE]`.
    /// `None` if same as `male`.
    pub female: Option<String>,
}

/// A single attack in a monster's attack array.
/// Mirrors one element of `permonst.mattk[]` (`struct attack` in C).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackDef {
    /// How the attack is delivered (claw, bite, etc.).  From `attack.aatyp` (`AT_xxx`).
    pub method: AttackMethod,
    /// What damage/effect the attack inflicts.  From `attack.adtyp` (`AD_xxx`).
    pub damage_type: DamageType,
    /// Damage dice expression.  From `attack.damn` (count) and `attack.damd` (sides).
    pub dice: DiceExpr,
}

/// Dice expression `NdM`: roll N dice with M sides each.
/// Stored as `(damn, damd)` in the C `struct attack`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DiceExpr {
    /// Number of dice to roll.  From `attack.damn`.
    pub count: u8,
    /// Number of sides per die.  From `attack.damd`.
    pub sides: u8,
}

// ---------------------------------------------------------------------------
// MonsterSound
// ---------------------------------------------------------------------------

/// Sound type a monster makes.
/// Corresponds to `enum ms_sounds` in `monflag.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum MonsterSound {
    /// MS_SILENT — makes no sound
    Silent = 0,
    /// MS_BARK — if full moon, may howl
    Bark = 1,
    /// MS_MEW — mews or hisses
    Mew = 2,
    /// MS_ROAR — roars
    Roar = 3,
    /// MS_BELLOW — adult male crocodiles
    Bellow = 4,
    /// MS_GROWL — growls
    Growl = 5,
    /// MS_SQEEK — squeaks, as a rodent
    Sqeek = 6,
    /// MS_SQAWK — squawks, as a bird
    Sqawk = 7,
    /// MS_CHIRP — baby crocodile
    Chirp = 8,
    /// MS_HISS — hisses
    Hiss = 9,
    /// MS_BUZZ — buzzes (killer bee)
    Buzz = 10,
    /// MS_GRUNT — grunts (or speaks own language)
    Grunt = 11,
    /// MS_NEIGH — neighs, as an equine
    Neigh = 12,
    /// MS_MOO — minotaurs, rothes
    Moo = 13,
    /// MS_WAIL — wails, as a tortured soul
    Wail = 14,
    /// MS_GURGLE — gurgles, as liquid or through saliva
    Gurgle = 15,
    /// MS_BURBLE — burbles (jabberwock)
    Burble = 16,
    /// MS_TRUMPET — trumpets (elephant); also MS_ANIMAL upper bound
    Trumpet = 17,
    /// MS_SHRIEK — wakes up others
    Shriek = 18,
    /// MS_BONES — rattles bones (skeleton)
    Bones = 19,
    /// MS_LAUGH — grins, smiles, giggles, and laughs
    Laugh = 20,
    /// MS_MUMBLE — says something or other
    Mumble = 21,
    /// MS_IMITATE — imitates others (leocrotta)
    Imitate = 22,
    /// MS_WERE — lycanthrope in human form
    Were = 23,
    /// MS_ORC — intelligent brutes
    Orc = 24,
    /// MS_HUMANOID — generic traveling companion
    Humanoid = 25,
    /// MS_ARREST — "Stop in the name of the law!" (Kops)
    Arrest = 26,
    /// MS_SOLDIER — army and watchmen expressions
    Soldier = 27,
    /// MS_GUARD — "Please drop that gold and follow me."
    Guard = 28,
    /// MS_DJINNI — "Thank you for freeing me!"
    Djinni = 29,
    /// MS_NURSE — "Take off your shirt, please."
    Nurse = 30,
    /// MS_SEDUCE — "Hello, sailor." (Nymphs)
    Seduce = 31,
    /// MS_VAMPIRE — vampiric seduction, Vlad's exclamations
    Vampire = 32,
    /// MS_BRIBE — asks for money, or berates you
    Bribe = 33,
    /// MS_CUSS — berates (demons) or intimidates (Wiz)
    Cuss = 34,
    /// MS_RIDER — astral level special monsters
    Rider = 35,
    /// MS_LEADER — your class leader
    Leader = 36,
    /// MS_NEMESIS — your nemesis
    Nemesis = 37,
    /// MS_GUARDIAN — your leader's guards
    Guardian = 38,
    /// MS_SELL — demand payment, complain about shoplifters
    Sell = 39,
    /// MS_ORACLE — do a consultation
    Oracle = 40,
    /// MS_PRIEST — ask for contribution; do cleansing
    Priest = 41,
    /// MS_SPELL — spellcaster not matching any of the above
    Spell = 42,
    /// MS_BOAST — giants
    Boast = 43,
    /// MS_GROAN — zombies groan
    Groan = 44,
}

// ---------------------------------------------------------------------------
// MonsterSize
// ---------------------------------------------------------------------------

/// Physical size category of a monster.
/// Corresponds to `MZ_xxx` constants in `monflag.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum MonsterSize {
    /// MZ_TINY (0) — less than 2 feet
    Tiny = 0,
    /// MZ_SMALL (1) — 2 to 4 feet
    Small = 1,
    /// MZ_MEDIUM (2) — 4 to 7 feet (also MZ_HUMAN)
    #[default]
    Medium = 2,
    /// MZ_LARGE (3) — 7 to 12 feet
    Large = 3,
    /// MZ_HUGE (4) — 12 to 25 feet
    Huge = 4,
    /// (5) — reserved / unused
    Enormous = 5,
    /// (6) — reserved / unused
    Colossal = 6,
    /// MZ_GIGANTIC (7) — off the scale
    Gigantic = 7,
}

// ---------------------------------------------------------------------------
// AttackMethod
// ---------------------------------------------------------------------------

/// How a monster delivers an attack.
/// Corresponds to `AT_xxx` defines in `monattk.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum AttackMethod {
    /// AT_NONE (0) — passive monster (e.g. acid blob)
    None = 0,
    /// AT_CLAW (1) — claw, punch, hit
    Claw = 1,
    /// AT_BITE (2) — bite
    Bite = 2,
    /// AT_KICK (3) — kick
    Kick = 3,
    /// AT_BUTT (4) — head butt (e.g. unicorn)
    Butt = 4,
    /// AT_TUCH (5) — touches
    Touch = 5,
    /// AT_STNG (6) — sting
    Sting = 6,
    /// AT_HUGS (7) — crushing bearhug
    Hug = 7,
    /// AT_SPIT (10) — spits substance (ranged)
    Spit = 10,
    /// AT_ENGL (11) — engulf (swallow or by a cloud)
    Engulf = 11,
    /// AT_BREA (12) — breath weapon (ranged)
    Breath = 12,
    /// AT_EXPL (13) — explodes (proximity)
    Explode = 13,
    /// AT_BOOM (14) — explodes when killed
    Boom = 14,
    /// AT_GAZE (15) — gaze attack (ranged)
    Gaze = 15,
    /// AT_TENT (16) — tentacles
    Tentacle = 16,
    /// AT_WEAP (254) — uses a wielded weapon
    Weapon = 254,
    /// AT_MAGC (255) — uses magic spell(s)
    MagicMissile = 255,
}

// ---------------------------------------------------------------------------
// DamageType
// ---------------------------------------------------------------------------

/// What type of damage or special effect an attack inflicts.
/// Corresponds to `AD_xxx` defines in `monattk.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum DamageType {
    /// AD_PHYS (0) — ordinary physical damage
    Physical = 0,
    /// AD_MAGM (1) — magic missiles
    MagicMissile = 1,
    /// AD_FIRE (2) — fire damage
    Fire = 2,
    /// AD_COLD (3) — frost damage
    Cold = 3,
    /// AD_SLEE (4) — sleep ray
    Sleep = 4,
    /// AD_DISN (5) — disintegration (death ray)
    Disintegrate = 5,
    /// AD_ELEC (6) — shock damage
    Electricity = 6,
    /// AD_DRST (7) — drains strength (poison)
    Poison = 7,
    /// AD_ACID (8) — acid damage
    Acid = 8,
    /// AD_SPC1 (9) — reserved for buzz() extension
    Spc1 = 9,
    /// AD_SPC2 (10) — reserved for buzz() extension
    Spc2 = 10,
    /// AD_BLND (11) — blinds (yellow light)
    Blind = 11,
    /// AD_STUN (12) — stuns
    Stun = 12,
    /// AD_SLOW (13) — slows
    Slow = 13,
    /// AD_PLYS (14) — paralyzes
    Paralyze = 14,
    /// AD_DRLI (15) — drains life levels (vampire)
    DrainLife = 15,
    /// AD_DREN (16) — drains magic energy
    DrainMagic = 16,
    /// AD_LEGS (17) — damages legs (xan)
    Legs = 17,
    /// AD_STON (18) — petrifies (Medusa, cockatrice)
    Stone = 18,
    /// AD_STCK (19) — sticks to you (mimic)
    Sticking = 19,
    /// AD_SGLD (20) — steals gold (leprechaun)
    GoldSteal = 20,
    /// AD_SITM (21) — steals item (nymphs)
    ItemSteal = 21,
    /// AD_SEDU (22) — seduces and steals multiple items
    Seduce = 22,
    /// AD_TLPT (23) — teleports you (quantum mechanic)
    Teleport = 23,
    /// AD_RUST (24) — rusts armor (rust monster)
    Rust = 24,
    /// AD_CONF (25) — confuses (umber hulk)
    Confuse = 25,
    /// AD_DGST (26) — digests opponent (trapper, etc.)
    Digest = 26,
    /// AD_HEAL (27) — heals opponent's wounds (nurse)
    Heal = 27,
    /// AD_WRAP (28) — special "stick" for eels
    Wrap = 28,
    /// AD_WERE (29) — confers lycanthropy
    Lycanthropy = 29,
    /// AD_DRDX (30) — drains dexterity (quasit)
    DrainDex = 30,
    /// AD_DRCO (31) — drains constitution
    DrainCon = 31,
    /// AD_DRIN (32) — drains intelligence (mind flayer)
    DrainInt = 32,
    /// AD_DISE (33) — confers diseases
    Disease = 33,
    /// AD_DCAY (34) — decays organics (brown pudding)
    Decay = 34,
    /// AD_SSEX (35) — succubus seduction (extended)
    SSuccubus = 35,
    /// AD_HALU (36) — causes hallucination
    Hallucinate = 36,
    /// AD_DETH (37) — for Death only
    Death = 37,
    /// AD_PEST (38) — for Pestilence only
    Pestilence = 38,
    /// AD_FAMN (39) — for Famine only
    Famine = 39,
    /// AD_SLIM (40) — turns you into green slime
    Slime = 40,
    /// AD_ENCH (41) — removes enchantment (disenchanter)
    Disenchant = 41,
    /// AD_CORR (42) — corrodes armor (black pudding)
    Corrode = 42,
    /// AD_POLY (43) — polymorphs the target (genetic engineer)
    Polymorph = 43,
    /// AD_CLRC (240) — random clerical spell
    ClericSpell = 240,
    /// AD_SPEL (241) — random magic spell
    MagicSpell = 241,
    /// AD_RBRE (242) — random breath weapon
    RandomBreath = 242,
    /// AD_SAMU (252) — hits, may steal Amulet (Wizard)
    StealAmulet = 252,
    /// AD_CURS (253) — random curse (e.g. gremlin)
    Curse = 253,
}

// ---------------------------------------------------------------------------
// GenoFlags
// ---------------------------------------------------------------------------

bitflags! {
    /// Generation and genocide flags for a monster species.
    /// Corresponds to `G_xxx` defines in `monflag.h`, stored in `permonst.geno`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

    pub struct GenoFlags: u16 {
        /// G_GENOD (0x02) — has been genocided (runtime flag in `mvitals`)
        const G_GENOD    = 0x0002;
        /// G_EXTINCT (0x01) — population control; create no more (runtime)
        const G_EXTINCT  = 0x0001;
        /// G_NOCORPSE (0x10) — no corpse left ever
        const G_NOCORPSE = 0x0010;
        /// G_GENO (0x20) — can be genocided
        const G_GENO     = 0x0020;
        /// G_LGROUP (0x40) — appears in large groups normally
        const G_LGROUP   = 0x0040;
        /// G_SGROUP (0x80) — appears in small groups normally
        const G_SGROUP   = 0x0080;
        /// G_NOGEN (0x200) — generated only specially
        const G_NOGEN    = 0x0200;
        /// G_HELL (0x400) — generated only in "hell" (Gehennom)
        const G_HELL     = 0x0400;
        /// G_NOHELL (0x800) — not generated in "hell"
        const G_NOHELL   = 0x0800;
        /// G_UNIQ (0x1000) — generated only once (unique monster)
        const G_UNIQ     = 0x1000;
    }
}

impl_serde_for_bitflags!(GenoFlags, u16);

// ---------------------------------------------------------------------------
// ResistanceSet
// ---------------------------------------------------------------------------

bitflags! {
    /// Set of elemental resistances.
    /// Corresponds to `MR_xxx` defines in `monflag.h`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

    pub struct ResistanceSet: u8 {
        /// MR_FIRE (0x01) — resists fire
        const FIRE        = 0x01;
        /// MR_COLD (0x02) — resists cold
        const COLD        = 0x02;
        /// MR_SLEEP (0x04) — resists sleep
        const SLEEP       = 0x04;
        /// MR_DISINT (0x08) — resists disintegration
        const DISINTEGRATE = 0x08;
        /// MR_ELEC (0x10) — resists electricity
        const SHOCK       = 0x10;
        /// MR_POISON (0x20) — resists poison
        const POISON      = 0x20;
        /// MR_ACID (0x40) — resists acid
        const ACID        = 0x40;
        /// MR_STONE (0x80) — resists petrification
        const STONE       = 0x80;
    }
}

impl_serde_for_bitflags!(ResistanceSet, u8);

// ---------------------------------------------------------------------------
// MonsterFlags
// ---------------------------------------------------------------------------

bitflags! {
    /// Behavioral and physical flag bits for monsters.
    /// Combines M1_xxx, M2_xxx, and M3_xxx from `monflag.h` into a single u128
    /// (M1 in bits 0..31, M2 in bits 32..63, M3 in bits 64..95).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

    pub struct MonsterFlags: u128 {
        // ---- M1 flags (bits 0..31) ----
        /// M1_FLY — can fly or float
        const FLY           = 0x0000_0001;
        /// M1_SWIM — can traverse water
        const SWIM          = 0x0000_0002;
        /// M1_AMORPHOUS — can flow under doors
        const AMORPHOUS     = 0x0000_0004;
        /// M1_WALLWALK — can phase through rock
        const WALLWALK      = 0x0000_0008;
        /// M1_CLING — can cling to ceiling
        const CLING         = 0x0000_0010;
        /// M1_TUNNEL — can tunnel through rock
        const TUNNEL        = 0x0000_0020;
        /// M1_NEEDPICK — needs pick to tunnel
        const NEEDPICK      = 0x0000_0040;
        /// M1_CONCEAL — hides under objects
        const CONCEAL       = 0x0000_0080;
        /// M1_HIDE — mimics, blends in with ceiling
        const HIDE          = 0x0000_0100;
        /// M1_AMPHIBIOUS — can survive underwater
        const AMPHIBIOUS    = 0x0000_0200;
        /// M1_BREATHLESS — doesn't need to breathe
        const BREATHLESS    = 0x0000_0400;
        /// M1_NOTAKE — cannot pick up objects
        const NOTAKE        = 0x0000_0800;
        /// M1_NOEYES — no eyes to gaze into or blind
        const NOEYES        = 0x0000_1000;
        /// M1_NOHANDS — no hands to handle things
        const NOHANDS       = 0x0000_2000;
        /// M1_NOLIMBS — no arms/legs to kick/wear on (includes NOHANDS)
        const NOLIMBS       = 0x0000_6000;
        /// M1_NOHEAD — no head to behead
        const NOHEAD        = 0x0000_8000;
        /// M1_MINDLESS — has no mind (golem, zombie, mold)
        const MINDLESS      = 0x0001_0000;
        /// M1_HUMANOID — has humanoid head/arms/torso
        const HUMANOID      = 0x0002_0000;
        /// M1_ANIMAL — has animal body
        const ANIMAL        = 0x0004_0000;
        /// M1_SLITHY — has serpent body
        const SLITHY        = 0x0008_0000;
        /// M1_UNSOLID — has no solid or liquid body
        const UNSOLID       = 0x0010_0000;
        /// M1_THICK_HIDE — has thick hide or scales
        const THICK_HIDE    = 0x0020_0000;
        /// M1_OVIPAROUS — can lay eggs
        const OVIPAROUS     = 0x0040_0000;
        /// M1_REGEN — regenerates hit points
        const REGEN         = 0x0080_0000;
        /// M1_SEE_INVIS — can see invisible creatures
        const SEE_INVIS     = 0x0100_0000;
        /// M1_TPORT — can teleport
        const TPORT         = 0x0200_0000;
        /// M1_TPORT_CNTRL — controls where it teleports to
        const TPORT_CNTRL   = 0x0400_0000;
        /// M1_ACID — acidic to eat
        const ACID          = 0x0800_0000;
        /// M1_POIS — poisonous to eat
        const POIS          = 0x1000_0000;
        /// M1_CARNIVORE — eats corpses
        const CARNIVORE     = 0x2000_0000;
        /// M1_HERBIVORE — eats fruits
        const HERBIVORE     = 0x4000_0000;
        /// M1_OMNIVORE — eats both (CARNIVORE | HERBIVORE)
        const OMNIVORE      = 0x6000_0000;
        /// M1_METALLIVORE — eats metal
        const METALLIVORE   = 0x8000_0000;

        // ---- M2 flags (bits 32..63) ----
        /// M2_NOPOLY — players may not polymorph into one
        const NOPOLY        = 0x0000_0001_0000_0000;
        /// M2_UNDEAD — is walking dead
        const UNDEAD        = 0x0000_0002_0000_0000;
        /// M2_WERE — is a lycanthrope
        const WERE          = 0x0000_0004_0000_0000;
        /// M2_HUMAN — is a human
        const HUMAN         = 0x0000_0008_0000_0000;
        /// M2_ELF — is an elf
        const ELF           = 0x0000_0010_0000_0000;
        /// M2_DWARF — is a dwarf
        const DWARF         = 0x0000_0020_0000_0000;
        /// M2_GNOME — is a gnome
        const GNOME         = 0x0000_0040_0000_0000;
        /// M2_ORC — is an orc
        const ORC           = 0x0000_0080_0000_0000;
        /// M2_DEMON — is a demon
        const DEMON         = 0x0000_0100_0000_0000;
        /// M2_MERC — is a guard or soldier
        const MERC          = 0x0000_0200_0000_0000;
        /// M2_LORD — is a lord to its kind
        const LORD          = 0x0000_0400_0000_0000;
        /// M2_PRINCE — is an overlord to its kind
        const PRINCE        = 0x0000_0800_0000_0000;
        /// M2_MINION — is a minion of a deity
        const MINION        = 0x0000_1000_0000_0000;
        /// M2_GIANT — is a giant
        const GIANT         = 0x0000_2000_0000_0000;
        /// M2_SHAPESHIFTER — is a shapeshifting species
        const SHAPESHIFTER  = 0x0000_4000_0000_0000;
        /// M2_MALE — always male
        const MALE          = 0x0001_0000_0000_0000;
        /// M2_FEMALE — always female
        const FEMALE        = 0x0002_0000_0000_0000;
        /// M2_NEUTER — neither male nor female
        const NEUTER        = 0x0004_0000_0000_0000;
        /// M2_PNAME — monster name is a proper name
        const PNAME         = 0x0008_0000_0000_0000;
        /// M2_HOSTILE — always starts hostile
        const HOSTILE       = 0x0010_0000_0000_0000;
        /// M2_PEACEFUL — always starts peaceful
        const PEACEFUL      = 0x0020_0000_0000_0000;
        /// M2_DOMESTIC — can be tamed by feeding
        const DOMESTIC      = 0x0040_0000_0000_0000;
        /// M2_WANDER — wanders randomly
        const WANDER        = 0x0080_0000_0000_0000;
        /// M2_STALK — follows you to other levels
        const STALK         = 0x0100_0000_0000_0000;
        /// M2_NASTY — extra-nasty monster (more xp)
        const NASTY         = 0x0200_0000_0000_0000;
        /// M2_STRONG — strong (or big) monster
        const STRONG        = 0x0400_0000_0000_0000;
        /// M2_ROCKTHROW — throws boulders
        const ROCKTHROW     = 0x0800_0000_0000_0000;
        /// M2_GREEDY — likes gold
        const GREEDY        = 0x1000_0000_0000_0000;
        /// M2_JEWELS — likes gems
        const JEWELS        = 0x2000_0000_0000_0000;
        /// M2_COLLECT — picks up weapons and food
        const COLLECT       = 0x4000_0000_0000_0000;
        /// M2_MAGIC — picks up magic items
        const MAGIC         = 0x8000_0000_0000_0000;

        // ---- M3 flags (bits 64..95) ----
        /// M3_WANTSAMUL — would like to steal the amulet
        const WANTSAMUL     = 0x0001_0000_0000_0000_0000;
        /// M3_WANTSBELL — wants the bell
        const WANTSBELL     = 0x0002_0000_0000_0000_0000;
        /// M3_WANTSBOOK — wants the book
        const WANTSBOOK     = 0x0004_0000_0000_0000_0000;
        /// M3_WANTSCAND — wants the candelabrum
        const WANTSCAND     = 0x0008_0000_0000_0000_0000;
        /// M3_WANTSARTI — wants the quest artifact
        const WANTSARTI     = 0x0010_0000_0000_0000_0000;
        /// M3_WANTSALL — wants any major artifact (0x001f shifted)
        const WANTSALL      = 0x001f_0000_0000_0000_0000;
        /// M3_WAITFORU — waits to see you or get attacked
        const WAITFORU      = 0x0040_0000_0000_0000_0000;
        /// M3_CLOSE — lets you close unless attacked
        const CLOSE         = 0x0080_0000_0000_0000_0000;
        /// M3_COVETOUS — wants something (composite, same as WANTSALL)
        const COVETOUS      = 0x001f_0000_0000_0000_0000;
        /// M3_WAITMASK — waiting behavior mask (WAITFORU | CLOSE)
        const WAITMASK      = 0x00c0_0000_0000_0000_0000;
        /// M3_INFRAVISION — has infravision
        const INFRAVISION   = 0x0100_0000_0000_0000_0000;
        /// M3_INFRAVISIBLE — visible by infravision
        const INFRAVISIBLE  = 0x0200_0000_0000_0000_0000;
        /// M3_DISPLACES — moves monsters out of its way
        const DISPLACES     = 0x0400_0000_0000_0000_0000;
    }
}

impl_serde_for_bitflags!(MonsterFlags, u128);

// ---------------------------------------------------------------------------
// Object definitions
// ---------------------------------------------------------------------------

/// Static definition of an object type.
/// Mirrors C `struct objclass` from `objclass.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDef {
    /// Unique numeric identifier (index into `objects[]`).
    pub id: ObjectTypeId,
    /// Actual name of the object.  From `obj_descr[].oc_name`.
    pub name: String,
    /// Description when name is unknown (randomized appearance).  From `obj_descr[].oc_descr`.
    pub appearance: Option<String>,
    /// Object class category.  From `objclass.oc_class`.
    pub class: ObjectClass,
    /// Display color.  From `objclass.oc_color`.
    pub color: Color,
    /// Primary material composition.  From `objclass.oc_material`.
    pub material: Material,
    /// Encumbrance weight in cn (1 cn = 0.1 lb).  From `objclass.oc_weight`.
    pub weight: u16,
    /// Base cost in shops (zorkmids).  From `objclass.oc_cost`.
    pub cost: i16,
    /// Nutritional value (for food items).  From `objclass.oc_nutrition`.
    pub nutrition: u16,
    /// Generation probability weight (used in `mkobj()`).  From `objclass.oc_prob`.
    pub prob: u16,
    /// Whether this object is inherently magical.  From `objclass.oc_magic`.
    pub is_magic: bool,
    /// Whether otherwise-equal instances merge into stacks.  From `objclass.oc_merge`.
    pub is_mergeable: bool,
    /// Whether this object may have +n or (n) charges.  From `objclass.oc_charged`.
    pub is_charged: bool,
    /// Whether this is a special one-of-a-kind object.  From `objclass.oc_unique`.
    pub is_unique: bool,
    /// Whether this object cannot be wished for.  From `objclass.oc_nowish`.
    pub is_nowish: bool,
    /// For weapons: requires two hands.  For armor: bulky.  From `objclass.oc_big`.
    pub is_bimanual: bool,
    /// Whether the armor is bulky (alias for `oc_big` in armor context).
    pub is_bulky: bool,
    /// Whether the gem/ring is hard (tough).  From `objclass.oc_tough`.
    pub is_tough: bool,
    /// Weapon-specific information (damage dice, hit bonus, skill).  Present if class is Weapon.
    pub weapon: Option<WeaponInfo>,
    /// Armor-specific information (AC, magic cancellation, category).  Present if class is Armor.
    pub armor: Option<ArmorInfo>,
    /// Spellbook-specific information (spell level, direction, skill).  Present if class is Spellbook.
    pub spellbook: Option<SpellbookInfo>,
    /// Property conferred when worn/wielded (e.g. FireRes).  From `objclass.oc_oprop`.
    pub conferred_property: Option<Property>,
    /// Delay when using this object.  From `objclass.oc_delay`.
    pub use_delay: i8,
}

/// Weapon-specific fields from `struct objclass`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponInfo {
    /// Weapon skill category.  From `objclass.oc_skill` (alias for `oc_subtyp`).
    pub skill: WeaponSkill,
    /// "To hit" bonus.  From `objclass.oc_hitbon` (alias for `oc_oc1`).
    pub hit_bonus: i8,
    /// Damage dice vs. small monsters.  From `objclass.oc_wsdam`.
    pub damage_small: i8,
    /// Damage dice vs. large monsters.  From `objclass.oc_wldam`.
    pub damage_large: i8,
    /// Bitmask of PIERCE/SLASH/WHACK modes.  From `objclass.oc_dir` (overloaded).
    pub strike_mode: StrikeMode,
}

/// Armor-specific fields from `struct objclass`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArmorInfo {
    /// What body slot this armor occupies.  From `objclass.oc_armcat` (alias for `oc_subtyp`).
    pub category: ArmorCategory,
    /// Base armor class bonus.  From `objclass.a_ac` (alias for `oc_oc1`).
    pub ac_bonus: i8,
    /// Magic cancellation level.  From `objclass.a_can` (alias for `oc_oc2`).
    pub magic_cancel: i8,
}

/// Spellbook-specific fields from `struct objclass`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpellbookInfo {
    /// Spell difficulty level.  From `objclass.oc_level` (alias for `oc_oc2`).
    pub spell_level: i8,
    /// Zap direction/style.  From `objclass.oc_dir`.
    pub direction: SpellDirection,
}

// ---------------------------------------------------------------------------
// Material
// ---------------------------------------------------------------------------

/// Object material type.
/// Corresponds to `enum obj_material_types` in `objclass.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Material {
    /// NO_MATERIAL (0)
    NoMaterial = 0,
    /// LIQUID (1) — currently only for venom
    Liquid = 1,
    /// WAX (2)
    Wax = 2,
    /// VEGGY (3) — foodstuffs
    Veggy = 3,
    /// FLESH (4) — foodstuffs
    Flesh = 4,
    /// PAPER (5)
    Paper = 5,
    /// CLOTH (6)
    Cloth = 6,
    /// LEATHER (7)
    Leather = 7,
    /// WOOD (8)
    Wood = 8,
    /// BONE (9)
    Bone = 9,
    /// DRAGON_HIDE (10) — not leather!
    DragonHide = 10,
    /// IRON (11) — Fe, includes steel
    Iron = 11,
    /// METAL (12) — Sn, etc.
    Metal = 12,
    /// COPPER (13) — Cu, includes brass
    Copper = 13,
    /// SILVER (14) — Ag
    Silver = 14,
    /// GOLD (15) — Au
    Gold = 15,
    /// PLATINUM (16) — Pt
    Platinum = 16,
    /// MITHRIL (17)
    Mithril = 17,
    /// PLASTIC (18)
    Plastic = 18,
    /// GLASS (19)
    Glass = 19,
    /// GEMSTONE (20)
    Gemstone = 20,
    /// MINERAL (21)
    Mineral = 21,
}

// ---------------------------------------------------------------------------
// ObjectClass
// ---------------------------------------------------------------------------

/// Object class category.
/// Corresponds to `enum objclass_classes` in `objclass.h` (generated via `defsym.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ObjectClass {
    /// RANDOM_CLASS (0) — used for generating random objects
    Random = 0,
    /// ILLOBJ_CLASS (1) — illegal/strange object
    IllegalObject = 1,
    /// WEAPON_CLASS (2)
    Weapon = 2,
    /// ARMOR_CLASS (3)
    Armor = 3,
    /// RING_CLASS (4)
    Ring = 4,
    /// AMULET_CLASS (5)
    Amulet = 5,
    /// TOOL_CLASS (6)
    Tool = 6,
    /// FOOD_CLASS (7)
    Food = 7,
    /// POTION_CLASS (8)
    Potion = 8,
    /// SCROLL_CLASS (9)
    Scroll = 9,
    /// SPBOOK_CLASS (10)
    Spellbook = 10,
    /// WAND_CLASS (11)
    Wand = 11,
    /// COIN_CLASS (12)
    Coin = 12,
    /// GEM_CLASS (13)
    Gem = 13,
    /// ROCK_CLASS (14)
    Rock = 14,
    /// BALL_CLASS (15)
    Ball = 15,
    /// CHAIN_CLASS (16)
    Chain = 16,
    /// VENOM_CLASS (17)
    Venom = 17,
}

// ---------------------------------------------------------------------------
// ArmorCategory
// ---------------------------------------------------------------------------

/// Sub-category for armor objects.
/// Corresponds to `enum obj_armor_types` in `objclass.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ArmorCategory {
    /// ARM_SUIT (0) — body armor
    Suit = 0,
    /// ARM_SHIELD (1)
    Shield = 1,
    /// ARM_HELM (2)
    Helm = 2,
    /// ARM_GLOVES (3)
    Gloves = 3,
    /// ARM_BOOTS (4)
    Boots = 4,
    /// ARM_CLOAK (5)
    Cloak = 5,
    /// ARM_SHIRT (6)
    Shirt = 6,
}

// ---------------------------------------------------------------------------
// WeaponSkill
// ---------------------------------------------------------------------------

/// Weapon/spell skill type.
/// Corresponds to `enum p_skills` in `skills.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum WeaponSkill {
    /// P_NONE (0) — no skill applicable
    None = 0,
    /// P_DAGGER (1)
    Dagger = 1,
    /// P_KNIFE (2)
    Knife = 2,
    /// P_AXE (3)
    Axe = 3,
    /// P_PICK_AXE (4)
    PickAxe = 4,
    /// P_SHORT_SWORD (5)
    ShortSword = 5,
    /// P_BROAD_SWORD (6)
    BroadSword = 6,
    /// P_LONG_SWORD (7)
    LongSword = 7,
    /// P_TWO_HANDED_SWORD (8)
    TwoHandedSword = 8,
    /// P_SABER (9) — curved sword, includes scimitar
    Saber = 9,
    /// P_CLUB (10) — heavy-shafted bludgeon
    Club = 10,
    /// P_MACE (11)
    Mace = 11,
    /// P_MORNING_STAR (12) — spiked bludgeon
    MorningStar = 12,
    /// P_FLAIL (13) — two pieces hinged or chained together
    Flail = 13,
    /// P_HAMMER (14) — heavy head on the end
    Hammer = 14,
    /// P_QUARTERSTAFF (15) — long-shafted bludgeon
    Quarterstaff = 15,
    /// P_POLEARMS (16) — attack two or three steps away
    Polearms = 16,
    /// P_SPEAR (17) — includes javelin
    Spear = 17,
    /// P_TRIDENT (18)
    Trident = 18,
    /// P_LANCE (19)
    Lance = 19,
    /// P_BOW (20) — launcher
    Bow = 20,
    /// P_SLING (21)
    Sling = 21,
    /// P_CROSSBOW (22)
    Crossbow = 22,
    /// P_DART (23) — hand-thrown missile
    Dart = 23,
    /// P_SHURIKEN (24)
    Shuriken = 24,
    /// P_BOOMERANG (25)
    Boomerang = 25,
    /// P_WHIP (26) — flexible, one-handed
    Whip = 26,
    /// P_UNICORN_HORN (27) — last weapon, two-handed
    UnicornHorn = 27,
    /// P_ATTACK_SPELL (28)
    AttackSpell = 28,
    /// P_HEALING_SPELL (29)
    HealingSpell = 29,
    /// P_DIVINATION_SPELL (30)
    DivineSpell = 30,
    /// P_ENCHANTMENT_SPELL (31)
    EnchantSpell = 31,
    /// P_CLERIC_SPELL (32)
    ClericSpell = 32,
    /// P_ESCAPE_SPELL (33)
    EscapeSpell = 33,
    /// P_MATTER_SPELL (34)
    MatterSpell = 34,
    /// P_BARE_HANDED_COMBAT (35) — weaponless; gloves are ok
    BareHanded = 35,
    /// P_TWO_WEAPON_COMBAT (36) — pair of weapons, one in each hand
    TwoWeapon = 36,
    /// P_RIDING (37) — how well you control your steed
    Riding = 37,
}

// ---------------------------------------------------------------------------
// StrikeMode
// ---------------------------------------------------------------------------

bitflags! {
    /// Weapon strike mode bitmask.
    /// Corresponds to PIERCE/SLASH/WHACK defines in `objclass.h`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]

    pub struct StrikeMode: u8 {
        /// PIERCE (1) — pointed weapon punctures target
        const PIERCE = 1;
        /// SLASH (2) — sharp weapon cuts target
        const SLASH  = 2;
        /// WHACK (4) — blunt weapon bashes target
        const WHACK  = 4;
    }
}

impl_serde_for_bitflags!(StrikeMode, u8);

// ---------------------------------------------------------------------------
// SpellDirection
// ---------------------------------------------------------------------------

/// Zap style for wands and spells.
/// Corresponds to NODIR/IMMEDIATE/RAY defines in `objclass.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SpellDirection {
    /// No direction needed (0 — default/unset)
    NoDir = 1,
    /// IMMEDIATE (2) — directional beam that does not ricochet
    Immediate = 2,
    /// RAY (3) — beam that bounces off walls
    Ray = 3,
}

// ---------------------------------------------------------------------------
// Property
// ---------------------------------------------------------------------------

/// Intrinsic/extrinsic property that can be conferred by objects or monsters.
/// Corresponds to `enum prop_types` in `prop.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Property {
    /// FIRE_RES (1) — fire resistance
    FireRes = 1,
    /// COLD_RES (2) — cold resistance
    ColdRes = 2,
    /// SLEEP_RES (3) — sleep resistance
    SleepRes = 3,
    /// DISINT_RES (4) — disintegration resistance
    DisintRes = 4,
    /// SHOCK_RES (5) — shock resistance
    ShockRes = 5,
    /// POISON_RES (6) — poison resistance
    PoisonRes = 6,
    /// ACID_RES (7) — acid resistance
    AcidRes = 7,
    /// STONE_RES (8) — petrification resistance
    StoneRes = 8,
    /// DRAIN_RES (9) — drain resistance
    DrainRes = 9,
    /// SICK_RES (10) — sickness resistance
    SickRes = 10,
    /// INVULNERABLE (11)
    Invulnerable = 11,
    /// ANTIMAGIC (12)
    Antimagic = 12,
    /// STUNNED (13)
    Stunned = 13,
    /// CONFUSION (14)
    Confusion = 14,
    /// BLINDED (15)
    Blinded = 15,
    /// DEAF (16)
    Deaf = 16,
    /// SICK (17)
    Sick = 17,
    /// STONED (18)
    Stoned = 18,
    /// STRANGLED (19)
    Strangled = 19,
    /// VOMITING (20)
    Vomiting = 20,
    /// GLIB (21) — slippery fingers
    Glib = 21,
    /// SLIMED (22)
    Slimed = 22,
    /// HALLUC (23) — hallucinating
    Halluc = 23,
    /// HALLUC_RES (24) — hallucination resistance
    HallucRes = 24,
    /// FUMBLING (25)
    Fumbling = 25,
    /// WOUNDED_LEGS (26)
    WoundedLegs = 26,
    /// SLEEPY (27)
    Sleepy = 27,
    /// HUNGER (28) — hunger acceleration
    Hunger = 28,
    /// SEE_INVIS (29)
    SeeInvis = 29,
    /// TELEPAT (30) — telepathy
    Telepat = 30,
    /// WARNING (31)
    Warning = 31,
    /// WARN_OF_MON (32)
    WarnOfMon = 32,
    /// WARN_UNDEAD (33)
    WarnUndead = 33,
    /// SEARCHING (34)
    Searching = 34,
    /// CLAIRVOYANT (35)
    Clairvoyant = 35,
    /// INFRAVISION (36)
    Infravision = 36,
    /// DETECT_MONSTERS (37)
    DetectMonsters = 37,
    /// BLND_RES (38) — blindness resistance
    BlindRes = 38,
    /// ADORNED (39) — charisma bonus from gems
    Adorned = 39,
    /// INVIS (40) — invisible
    Invis = 40,
    /// DISPLACED (41) — displaced image
    Displaced = 41,
    /// STEALTH (42)
    Stealth = 42,
    /// AGGRAVATE_MONSTER (43)
    AggravateMonster = 43,
    /// CONFLICT (44)
    Conflict = 44,
    /// JUMPING (45)
    Jumping = 45,
    /// TELEPORT (46)
    Teleport = 46,
    /// TELEPORT_CONTROL (47)
    TeleportControl = 47,
    /// LEVITATION (48)
    Levitation = 48,
    /// FLYING (49)
    Flying = 49,
    /// WWALKING (50) — water walking
    Wwalking = 50,
    /// SWIMMING (51)
    Swimming = 51,
    /// MAGICAL_BREATHING (52)
    MagicalBreathing = 52,
    /// PASSES_WALLS (53) — phasing
    PassesWalls = 53,
    /// SLOW_DIGESTION (54)
    SlowDigestion = 54,
    /// HALF_SPDAM (55) — half spell damage
    HalfSpdam = 55,
    /// HALF_PHDAM (56) — half physical damage
    HalfPhdam = 56,
    /// REGENERATION (57)
    Regeneration = 57,
    /// ENERGY_REGENERATION (58)
    EnergyRegeneration = 58,
    /// PROTECTION (59) — divine protection
    Protection = 59,
    /// PROT_FROM_SHAPE_CHANGERS (60)
    ProtFromShapeChangers = 60,
    /// POLYMORPH (61)
    Polymorph = 61,
    /// POLYMORPH_CONTROL (62)
    PolymorphControl = 62,
    /// UNCHANGING (63)
    Unchanging = 63,
    /// FAST (64) — speed
    Fast = 64,
    /// REFLECTING (65)
    Reflecting = 65,
    /// FREE_ACTION (66)
    FreeAction = 66,
    /// FIXED_ABIL (67) — fixed abilities
    FixedAbil = 67,
    /// LIFESAVED (68) — life saving
    Lifesaved = 68,
}

// ---------------------------------------------------------------------------
// EquipSlot
// ---------------------------------------------------------------------------

bitflags! {
    /// Equipment slots bitmask.
    /// Corresponds to `W_xxx` defines in `prop.h`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct EquipSlot: u32 {
        /// W_ARM (0x00000001) — body armor
        const ARM     = 0x0000_0001;
        /// W_ARMC (0x00000002) — cloak
        const ARMC    = 0x0000_0002;
        /// W_ARMH (0x00000004) — helmet/hat
        const ARMH    = 0x0000_0004;
        /// W_ARMS (0x00000008) — shield
        const ARMS    = 0x0000_0008;
        /// W_ARMG (0x00000010) — gloves/gauntlets
        const ARMG    = 0x0000_0010;
        /// W_ARMF (0x00000020) — footwear
        const ARMF    = 0x0000_0020;
        /// W_ARMU (0x00000040) — undershirt
        const ARMU    = 0x0000_0040;
        /// W_WEP (0x00000100) — wielded weapon
        const WEP     = 0x0000_0100;
        /// W_QUIVER (0x00000200) — quiver for firing ammo
        const QUIVER  = 0x0000_0200;
        /// W_SWAPWEP (0x00000400) — secondary weapon
        const SWAPWEP = 0x0000_0400;
        /// W_ART (0x00001000) — carrying artifact (not really worn)
        const ART     = 0x0000_1000;
        /// W_ARTI (0x00002000) — invoked artifact (not really worn)
        const ARTI    = 0x0000_2000;
        /// W_AMUL (0x00010000) — amulet
        const AMUL    = 0x0001_0000;
        /// W_RINGL (0x00020000) — left ring
        const RINGL   = 0x0002_0000;
        /// W_RINGR (0x00040000) — right ring
        const RINGR   = 0x0004_0000;
        /// W_TOOL (0x00080000) — eyewear (blindfold, lenses)
        const TOOL    = 0x0008_0000;
        /// W_SADDLE (0x00100000) — riding saddle
        const SADDLE  = 0x0010_0000;
        /// W_BALL (0x00200000) — punishment ball
        const BALL    = 0x0020_0000;
        /// W_CHAIN (0x00400000) — punishment chain
        const CHAIN   = 0x0040_0000;
    }
}

impl_serde_for_bitflags!(EquipSlot, u32);

// ---------------------------------------------------------------------------
// TerrainType
// ---------------------------------------------------------------------------

/// Map terrain type.
/// Corresponds to `enum levl_typ_types` in `rm.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TerrainType {
    /// STONE (0) — solid rock
    Stone = 0,
    /// VWALL (1) — vertical wall
    VWall = 1,
    /// HWALL (2) — horizontal wall
    HWall = 2,
    /// TLCORNER (3) — top-left corner
    TLCorner = 3,
    /// TRCORNER (4) — top-right corner
    TRCorner = 4,
    /// BLCORNER (5) — bottom-left corner
    BLCorner = 5,
    /// BRCORNER (6) — bottom-right corner
    BRCorner = 6,
    /// CROSSWALL (7) — cross wall (for mazes and special levels)
    CrossWall = 7,
    /// TUWALL (8) — T-wall up
    TUWall = 8,
    /// TDWALL (9) — T-wall down
    TDWall = 9,
    /// TLWALL (10) — T-wall left
    TLWall = 10,
    /// TRWALL (11) — T-wall right
    TRWall = 11,
    /// DBWALL (12) — drawbridge wall
    DbWall = 12,
    /// TREE (13)
    Tree = 13,
    /// SDOOR (14) — secret door
    SecretDoor = 14,
    /// SCORR (15) — secret corridor
    SecretCorridor = 15,
    /// POOL (16) — water pool
    Pool = 16,
    /// MOAT (17) — pool that doesn't boil
    Moat = 17,
    /// WATER (18) — open water
    Water = 18,
    /// DRAWBRIDGE_UP (19) — raised drawbridge
    DrawbridgeUp = 19,
    /// LAVAPOOL (20)
    LavaPool = 20,
    /// LAVAWALL (21)
    LavaWall = 21,
    /// IRONBARS (22)
    IronBars = 22,
    /// DOOR (23)
    Door = 23,
    /// CORR (24) — corridor
    Corridor = 24,
    /// ROOM (25) — room floor
    Room = 25,
    /// STAIRS (26)
    Stairs = 26,
    /// LADDER (27)
    Ladder = 27,
    /// FOUNTAIN (28)
    Fountain = 28,
    /// THRONE (29)
    Throne = 29,
    /// SINK (30)
    Sink = 30,
    /// GRAVE (31)
    Grave = 31,
    /// ALTAR (32)
    Altar = 32,
    /// ICE (33)
    Ice = 33,
    /// DRAWBRIDGE_DOWN (34) — lowered drawbridge
    DrawbridgeDown = 34,
    /// AIR (35)
    Air = 35,
    /// CLOUD (36)
    Cloud = 36,
}

// ---------------------------------------------------------------------------
// MapCell
// ---------------------------------------------------------------------------

/// A single cell in the dungeon map.
/// Represents `struct rm` (level.locations[x][y]) from `rm.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapCell {
    /// Terrain type at this location.  From `rm.typ`.
    pub terrain: TerrainType,
    /// Display glyph index for rendering.  From `rm.glyph`.
    pub glyph: i32,
    /// Whether this cell has been seen by the hero.  From `rm.seenv`.
    pub seen: bool,
    /// Whether this cell is currently lit.  From `rm.lit`.
    pub lit: bool,
    /// Whether this cell has been mapped/remembered.  From `rm.waslit`.
    pub waslit: bool,
}

// ---------------------------------------------------------------------------
// TrapType
// ---------------------------------------------------------------------------

/// Type of trap on the dungeon map.
/// Corresponds to `enum trap_types` in `trap.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TrapType {
    /// NO_TRAP (0)
    NoTrap = 0,
    /// ARROW_TRAP (1)
    ArrowTrap = 1,
    /// DART_TRAP (2)
    DartTrap = 2,
    /// ROCKTRAP (3)
    RockTrap = 3,
    /// SQKY_BOARD (4) — squeaky board
    SqueakyBoard = 4,
    /// BEAR_TRAP (5)
    BearTrap = 5,
    /// LANDMINE (6)
    Landmine = 6,
    /// ROLLING_BOULDER_TRAP (7)
    RollingBoulderTrap = 7,
    /// SLP_GAS_TRAP (8) — sleeping gas trap
    SleepingGasTrap = 8,
    /// RUST_TRAP (9)
    RustTrap = 9,
    /// FIRE_TRAP (10)
    FireTrap = 10,
    /// PIT (11)
    Pit = 11,
    /// SPIKED_PIT (12)
    SpikedPit = 12,
    /// HOLE (13)
    Hole = 13,
    /// TRAPDOOR (14)
    TrapDoor = 14,
    /// TELEP_TRAP (15) — teleportation trap
    TeleportTrap = 15,
    /// LEVEL_TELEP (16) — level teleporter
    LevelTeleport = 16,
    /// MAGIC_PORTAL (17)
    MagicPortal = 17,
    /// WEB (18)
    Web = 18,
    /// STATUE_TRAP (19)
    StatueTrap = 19,
    /// MAGIC_TRAP (20)
    MagicTrap = 20,
    /// ANTI_MAGIC (21) — anti-magic field
    AntiMagic = 21,
    /// POLY_TRAP (22) — polymorph trap
    PolyTrap = 22,
    /// VIBRATING_SQUARE (23) — not a real trap; shown as one after discovery
    VibratingSquare = 23,
    /// TRAPPED_DOOR (24) — part of door, not a map trap
    TrappedDoor = 24,
    /// TRAPPED_CHEST (25) — part of object, not a map trap
    TrappedChest = 25,
}
