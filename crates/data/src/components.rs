//! ECS component structs for runtime game state.
//!
//! These components represent the mutable, per-instance data for objects,
//! monsters, and the player.  They are distinct from the static definitions
//! in [`crate::schema`] which describe *types* of things.

use serde::{Deserialize, Serialize};

use crate::schema::{
    Alignment, ArtifactId, DungeonLevel, EquipSlot, Gender, Handedness, MonsterId, ObjectClass,
    ObjectTypeId, RaceId, RoleId, TerrainType, TrapType, WeaponSkill,
};

// ===========================================================================
// Object components
// ===========================================================================

/// Core identity of an object instance.
/// Corresponds to the essential fields of C `struct obj`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectCore {
    /// Object type from the `objects[]` table.  From `obj.otyp`.
    pub otyp: ObjectTypeId,
    /// Object class character.  From `obj.oclass`.
    pub object_class: ObjectClass,
    /// Stack quantity (for mergeable objects).  From `obj.quan` (long in C).
    pub quantity: i32,
    /// Encumbrance weight.  From `obj.owt`.
    pub weight: u32,
    /// Creation date / fuel / timer.  From `obj.age`.
    pub age: i64,
    /// Inventory letter assignment.  From `obj.invlet`.
    pub inv_letter: Option<char>,
    /// Artifact identity, if this object is an artifact.  From `obj.oartifact`.
    pub artifact: Option<ArtifactId>,
}

/// Blessed/uncursed/cursed status of an object.
/// Derived from `obj.blessed` and `obj.cursed` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucStatus {
    /// Whether the object is cursed.  From `obj.cursed`.
    pub cursed: bool,
    /// Whether the object is blessed.  From `obj.blessed`.
    pub blessed: bool,
    /// Whether the BUC status is known to the player.  From `obj.bknown`.
    pub bknown: bool,
}

/// What the player knows about an object.
/// Derived from various knowledge flags in C `struct obj`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeState {
    /// Whether the enchantment/charges are known.  From `obj.known`.
    pub known: bool,
    /// Whether the appearance has been observed.  From `obj.dknown`.
    pub dknown: bool,
    /// Whether rustproofing/erodeproofing is known.  From `obj.rknown`.
    pub rknown: bool,
    /// Whether container contents are known.  From `obj.cknown`.
    pub cknown: bool,
    /// Whether lock state is known.  From `obj.lknown`.
    pub lknown: bool,
    /// Whether trap state is known.  From `obj.tknown`.
    pub tknown: bool,
}

/// Where an object currently resides.
/// Replaces the C `obj.where` field and the `OBJ_xxx` constants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectLocation {
    /// OBJ_FREE — object is free (being manipulated, not on any list).
    Free,
    /// OBJ_FLOOR — object is on the floor at (x, y).
    Floor {
        /// X coordinate on the level map.
        x: i16,
        /// Y coordinate on the level map.
        y: i16,
        /// Branch/depth of the dungeon level containing this floor object.
        level: DungeonLevel,
    },
    /// OBJ_CONTAINED — object is inside a container.
    Contained {
        /// ECS entity of the containing object.
        container_id: u32,
    },
    /// OBJ_INVENT — object is in the hero's inventory.
    Inventory,
    /// OBJ_MINVENT — object is in a monster's inventory.
    MonsterInventory {
        /// ECS entity or ID of the carrying monster.
        carrier_id: u32,
    },
    /// OBJ_MIGRATING — object is migrating between levels.
    Migrating,
    /// OBJ_BURIED — object is buried.
    Buried,
    /// OBJ_ONBILL — object is on a shopkeeper's bill.
    OnBill,
}

/// Enchantment level of an object.
/// Corresponds to `obj.spe` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Enchantment {
    /// Current enchantment value (+N or -N).  From `obj.spe` (-99..99).
    pub spe: i8,
}

/// Erosion state of an object.
/// Corresponds to `obj.oeroded`, `obj.oeroded2`, `obj.oerodeproof`, `obj.greased` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Erosion {
    /// Primary erosion level (rust for metal, rot for organic; 0..3).  From `obj.oeroded`.
    pub eroded: u8,
    /// Secondary erosion level (corrosion for metal, burn for organic; 0..3).  From `obj.oeroded2`.
    pub eroded2: u8,
    /// Whether the object is immune to erosion.  From `obj.oerodeproof`.
    pub erodeproof: bool,
    /// Whether the object has been greased.  From `obj.greased`.
    pub greased: bool,
}

/// Shop-related state for an object.
/// Derived from `obj.unpaid` and `obj.no_charge` in C.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopState {
    /// Whether the object is unpaid merchandise.  From `obj.unpaid`.
    pub unpaid: bool,
    /// Whether a shopkeeper has waived the charge.  From `obj.no_charge`.
    pub no_charge: bool,
}

/// State for objects that are containers.
/// Derived from `obj.olocked`, `obj.obroken`, `obj.otrapped` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerState {
    /// Whether the container is locked.  From `obj.olocked`.
    pub locked: bool,
    /// Whether the lock has been broken.  From `obj.obroken`.
    pub broken_lock: bool,
    /// Whether this is a trapped container.  From `obj.otrapped`.
    pub trapped: bool,
}

/// Light source state for an object that emits light.
/// Derived from `obj.lamplit` and `obj.recharged` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightSource {
    /// Whether the light source is currently lit.  From `obj.lamplit`.
    pub lit: bool,
    /// Number of times recharged (0..7).  From `obj.recharged`.
    pub recharged: u8,
}

/// Extra data for corpses, eggs, and tins.
/// Derived from `obj.corpsenm` and `obj.oeaten` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpseData {
    /// Monster type this corpse/tin came from.  From `obj.corpsenm`.
    pub monster_type: MonsterId,
    /// Remaining nutrition (partially eaten).  From `obj.oeaten`.
    pub eaten: u32,
}

/// Extra data attached to an object (corresponds to `oextra` in C).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectExtra {
    /// Individual name given to this object.  From `ONAME(obj)`.
    pub name: Option<String>,
    /// Monster contained within (e.g. statues, figurines).  From `OMONST(obj)`.
    pub contained_monster: Option<u32>,
}

// ===========================================================================
// Player components
// ===========================================================================

/// Player's current position on the map.
/// Corresponds to position and movement fields in `struct you`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPosition {
    /// X coordinate on the current level.  From `u.ux`.
    pub x: i16,
    /// Y coordinate on the current level.  From `u.uy`.
    pub y: i16,
    /// Previous X coordinate.  From `u.ux0`.
    pub prev_x: i16,
    /// Previous Y coordinate.  From `u.uy0`.
    pub prev_y: i16,
    /// Current dungeon level.  From `u.uz`.
    pub level: DungeonLevel,
    /// Previous dungeon level.  From `u.uz0`.
    pub prev_level: DungeonLevel,
    /// Travel destination, if traveling.  From `u.tx`, `u.ty`.
    pub travel_dest: Option<(i16, i16)>,
    /// Movement points remaining this turn.  From `u.umovement`.
    pub movement_points: i16,
    /// Whether the player has moved this turn.  From `u.umoved`.
    pub moved_this_turn: bool,
}

/// Player's name, role, race, gender, and alignment.
/// Corresponds to `svp.plname`, role/race/gender/alignment in `you` struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerIdentity {
    /// Player-chosen name.  From `svp.plname`.
    pub name: String,
    /// Role identifier.  From `u.urole.mnum`.
    pub role: RoleId,
    /// Race identifier.  From `u.urace.mnum`.
    pub race: RaceId,
    /// Current gender.  From `flags.female`.
    pub gender: Gender,
    /// Current alignment.  From `u.ualign`.
    pub alignment: Alignment,
    /// Alignment base values (original and converted).  From `u.ualignbase[CONVERT]`.
    pub alignment_base: [Alignment; 2],
    /// Dominant hand preference.  From `u.uhandedness`.
    pub handedness: Handedness,
}

/// Player's experience level and points.
/// Corresponds to `u.ulevel`, `u.ulevelmax`, `u.ulevelpeak`, `u.uexp`, `u.urexp` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerLevel {
    /// Current experience level (1..30).  From `u.ulevel`.
    pub level: u8,
    /// Maximum experience level attained.  From `u.ulevelmax`.
    pub level_max: u8,
    /// Peak experience level (for score purposes).  From `u.ulevelpeak`.
    pub level_peak: u8,
    /// Total accumulated experience points.  From `u.uexp`.
    pub experience: i64,
    /// Score-relevant experience points.  From `u.urexp`.
    pub score_experience: i64,
}

/// Player's current and maximum HP/power.
/// Corresponds to `u.uhp`, `u.uhpmax`, `u.uen`, `u.uenmax` and related fields in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerVitals {
    /// Current hit points.  From `u.uhp`.
    pub hp: i32,
    /// Maximum hit points.  From `u.uhpmax`.
    pub hp_max: i32,
    /// Peak hit points ever attained.  From `u.uhppeak`.
    pub hp_peak: i32,
    /// Current magical energy (power/mana).  From `u.uen`.
    pub pw: i32,
    /// Maximum magical energy.  From `u.uenmax`.
    pub pw_max: i32,
    /// Peak power ever attained.  From `u.uenpeak`.
    pub pw_peak: i32,
    /// Per-level HP increments for level drain recovery.  From `u.uhpinc[MAXULEV]`.
    pub hp_inc: [i16; 30],
    /// Per-level PW increments for level drain recovery.  From `u.ueninc[MAXULEV]`.
    pub pw_inc: [i16; 30],
}

/// Player's six ability scores plus all modifier tracks.
/// Corresponds to `u.acurr`, `u.amax`, `u.abon`, `u.aexe`, `u.atemp`, `u.atime` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAttributes {
    /// Current attribute values.  From `u.acurr`.
    pub current: Attributes,
    /// Maximum attainable attribute values.  From `u.amax`.
    pub max: Attributes,
    /// Racial/role bonuses to attributes.  From `u.abon`.
    pub bonus: Attributes,
    /// Exercise-based attribute changes.  From `u.aexe`.
    pub exercise: Attributes,
    /// Temporary attribute losses (e.g. from poison).  From `u.atemp`.
    pub temp_loss: Attributes,
    /// Timers for temporary attribute loss recovery.  From `u.atime`.
    pub temp_timer: Attributes,
}

/// The six D&D-style attributes.
/// Corresponds to `struct attribs` in `attrib.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attributes {
    /// Strength (STR).  From `attribs.a[A_STR]`.  Encoded as 3..25 (18/xx uses 18+extra).
    pub str_: i8,
    /// Dexterity (DEX).  From `attribs.a[A_DEX]`.
    pub dex: i8,
    /// Constitution (CON).  From `attribs.a[A_CON]`.
    pub con: i8,
    /// Intelligence (INT).  From `attribs.a[A_INT]`.
    pub int: i8,
    /// Wisdom (WIS).  From `attribs.a[A_WIS]`.
    pub wis: i8,
    /// Charisma (CHA).  From `attribs.a[A_CHA]`.
    pub cha: i8,
}

/// Number of property types in the property system (FIRE_RES=1 through LIFESAVED=68).
pub const NUM_PROPERTIES: usize = 68;

/// All player properties (intrinsics + extrinsics).
/// Corresponds to `u.uprops[LAST_PROP+1]` in C, indexed by property number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerProperties {
    /// Per-property state array.  Should contain exactly `NUM_PROPERTIES + 1`
    /// elements (index 0 unused, indices 1..=68 for each property).
    pub props: Vec<PropertyState>,
}

/// State of a single property for the player.
/// Corresponds to one `struct prop` element from `u.uprops[]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PropertyState {
    /// Extrinsic sources bitmask (which equipment slots provide this).  From `prop.extrinsic`.
    pub extrinsic: u32,
    /// Intrinsic state (timeout counter in low 24 bits + source flags in high bits).
    /// From `prop.intrinsic`.
    pub intrinsic: u32,
    /// Blocking sources bitmask (which equipment slots block this).  From `prop.blocked`.
    pub blocked: u32,
}

/// Player's combat statistics.
/// Corresponds to luck, hit/damage bonuses, AC, spell protection in `struct you`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerCombat {
    /// Current luck value (-10..10).  From `u.uluck`.
    pub luck: i8,
    /// Persistent luck bonus from luckstone/etc (-3..3).  From `u.moreluck`.
    pub luck_bonus: i8,
    /// Equipment-based to-hit bonus.  From `u.uhitinc`.
    pub hit_bonus: i8,
    /// Equipment-based damage bonus.  From `u.udaminc`.
    pub damage_bonus: i8,
    /// Armor class (lower is better).  From `u.uac`.
    pub ac: i8,
    /// Spell-granted protection (AC bonus).  From `u.uspellprot`.
    pub spell_protection: u8,
    /// Timer for spell protection duration.  From `u.usptime`.
    pub spell_prot_time: u8,
    /// Interval for spell protection ticking.  From `u.uspmtime`.
    pub spell_prot_interval: u8,
}

/// Player's hunger/satiation state.
/// Corresponds to `u.uhs` and `u.uhunger` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerHunger {
    /// Numeric nutrition counter (higher = more full).  From `u.uhunger`.
    pub nutrition: i32,
    /// Current hunger state category.  From `u.uhs`.
    pub hunger_state: HungerState,
}

/// Hunger state category.
/// Corresponds to the hunger levels in `eat.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum HungerState {
    /// Satiated -- overfull, may vomit
    Satiated = 0,
    /// Not hungry -- normal state
    NotHungry = 1,
    /// Hungry -- should eat soon
    Hungry = 2,
    /// Weak -- impaired from hunger
    Weak = 3,
    /// Fainting -- collapsing from hunger
    Fainting = 4,
    /// Fainted -- unconscious from hunger
    Fainted = 5,
    /// Starved -- fatal; game over
    Starved = 6,
}

/// Player's religious standing.
/// Corresponds to `u.ugangr`, `u.ugifts`, `u.ublessed`, `u.ublesscnt` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerReligion {
    /// God's anger level (0 = not angry).  From `u.ugangr`.
    pub god_anger: i32,
    /// Number of gifts received from god.  From `u.ugifts`.
    pub god_gifts: i32,
    /// Amount of divine protection granted.  From `u.ublessed`.
    pub blessed_amount: i32,
    /// Turns until next prayer will be answered.  From `u.ublesscnt`.
    pub bless_cooldown: i32,
}

/// Conduct tracking (voluntary challenges).
/// Corresponds to `struct u_conduct` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerConduct {
    /// Non-vegetarian food eaten count.  From `u.uconduct.unvegetarian`.
    pub unvegetarian: i64,
    /// Non-vegan food eaten count.  From `u.uconduct.unvegan`.
    pub unvegan: i64,
    /// Total food items eaten.  From `u.uconduct.food`.
    pub food: i64,
    /// Spells/prayers used count (atheist conduct).  From `u.uconduct.gnostic`.
    pub gnostic: i64,
    /// Weapon hits dealt.  From `u.uconduct.weaphit`.
    pub weaphit: i64,
    /// Direct kills.  From `u.uconduct.killer`.
    pub killer: i64,
    /// Scrolls/books read.  From `u.uconduct.literate`.
    pub literate: i64,
    /// Polymorph piles count.  From `u.uconduct.polypiles`.
    pub polypiles: i64,
    /// Self-polymorph count.  From `u.uconduct.polyselfs`.
    pub polyselfs: i64,
    /// Wishes made.  From `u.uconduct.wishes`.
    pub wishes: i64,
    /// Artifacts wished for.  From `u.uconduct.wisharti`.
    pub wisharti: i64,
    /// Sokoban cheats.  From `u.uconduct.sokocheat`.
    pub sokocheat: i64,
    /// Pets acquired/used.  From `u.uconduct.pets`.
    pub pets: i64,
}

/// Achievements the player has unlocked.
/// Corresponds to `u.uachieve` in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerAchievements {
    /// Set of achievements earned so far, in chronological order.
    pub achieved: Vec<Achievement>,
}

/// Individual achievement milestone.
/// Corresponds to `enum achivements` in `you.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Achievement {
    /// ACH_BELL (1) -- acquired Bell of Opening
    Bell = 1,
    /// ACH_HELL (2) -- entered Gehennom
    Hell = 2,
    /// ACH_CNDL (3) -- acquired Candelabrum of Invocation
    Candelabrum = 3,
    /// ACH_BOOK (4) -- acquired Book of the Dead
    Book = 4,
    /// ACH_INVK (5) -- performed invocation to gain access to Sanctum
    Invocation = 5,
    /// ACH_AMUL (6) -- acquired The Amulet
    Amulet = 6,
    /// ACH_ENDG (7) -- entered end game
    EndGame = 7,
    /// ACH_ASTR (8) -- entered Astral Plane
    AstralPlane = 8,
    /// ACH_UWIN (9) -- ascended
    Ascended = 9,
    /// ACH_MINE_PRIZE (10) -- acquired Mines' End luckstone
    MinesPrize = 10,
    /// ACH_SOKO_PRIZE (11) -- acquired Sokoban bag or amulet
    SokobanPrize = 11,
    /// ACH_MEDU (12) -- killed Medusa
    KilledMedusa = 12,
    /// ACH_BLND (13) -- hero was always blind
    AlwaysBlind = 13,
    /// ACH_NUDE (14) -- hero never wore armor
    NeverArmor = 14,
    /// ACH_MINE (15) -- entered Gnomish Mines
    EnteredMines = 15,
    /// ACH_TOWN (16) -- reached Minetown
    ReachedMinetown = 16,
    /// ACH_SHOP (17) -- entered a shop
    EnteredShop = 17,
    /// ACH_TMPL (18) -- entered a temple
    EnteredTemple = 18,
    /// ACH_ORCL (19) -- consulted the Oracle
    ConsultedOracle = 19,
    /// ACH_NOVL (20) -- read at least one passage from a Discworld novel
    ReadNovel = 20,
    /// ACH_SOKO (21) -- entered Sokoban
    EnteredSokoban = 21,
    /// ACH_BGRM (22) -- entered Bigroom
    EnteredBigroom = 22,
    /// ACH_RNK1 (23) -- reached rank title 1
    Rank1 = 23,
    /// ACH_RNK2 (24) -- reached rank title 2
    Rank2 = 24,
    /// ACH_RNK3 (25) -- reached rank title 3
    Rank3 = 25,
    /// ACH_RNK4 (26) -- reached rank title 4
    Rank4 = 26,
    /// ACH_RNK5 (27) -- reached rank title 5
    Rank5 = 27,
    /// ACH_RNK6 (28) -- reached rank title 6
    Rank6 = 28,
    /// ACH_RNK7 (29) -- reached rank title 7
    Rank7 = 29,
    /// ACH_RNK8 (30) -- reached rank title 8
    Rank8 = 30,
    /// ACH_TUNE (31) -- discovered the castle drawbridge's open/close tune
    DiscoveredTune = 31,
}

/// Event flags tracked for the player.
/// Corresponds to `struct u_event` in C.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PlayerEvents {
    /// Whether the minor oracle was consulted.  From `u.uevent.minor_oracle`.
    pub minor_oracle: bool,
    /// Whether the major oracle was consulted.  From `u.uevent.major_oracle`.
    pub major_oracle: bool,
    /// Whether the quest leader has spoken.  From `u.uevent.qcalled`.
    pub quest_called: bool,
    /// Whether hero was expelled from quest.  From `u.uevent.qexpelled`.
    pub quest_expelled: bool,
    /// Whether the quest has been completed.  From `u.uevent.qcompleted`.
    pub quest_completed: bool,
    /// Drawbridge tune knowledge level (0/1/2/3).  From `u.uevent.uheard_tune`.
    pub heard_tune: u8,
    /// Whether the drawbridge was opened.  From `u.uevent.uopened_dbridge`.
    pub opened_dbridge: bool,
    /// Whether the invocation ritual has been performed.  From `u.uevent.invoked`.
    pub invoked: bool,
    /// Whether the player has entered Gehennom.  From `u.uevent.gehennom_entered`.
    pub gehennom_entered: bool,
    /// Level of hand-of-Elbereth status (0/1/2).  From `u.uevent.uhand_of_elbereth`.
    pub hand_of_elbereth: u8,
    /// Whether the Wizard of Yendor has been killed.  From `u.uevent.udemigod`.
    pub killed_wizard: bool,
    /// Turn when the Wizard of Yendor was last killed.
    #[serde(default)]
    pub wizard_last_killed_turn: u32,
    /// Number of times the Wizard of Yendor has been killed.
    #[serde(default)]
    pub wizard_times_killed: u32,
    /// Countdown in turns until the next off-screen Wizard intervention.
    /// Mirrors NetHack's `u.udg_cnt` cadence once the Wizard starts meddling.
    #[serde(default)]
    pub wizard_intervention_cooldown: u32,
    /// Whether the vibrating square was found.  From `u.uevent.ufound_vibsquare`.
    pub found_vibrating_square: bool,
    /// Whether the player ascended.
    pub ascended: bool,
}

/// Whether the player holds specific quest-critical items.
/// Corresponds to `struct u_have` in C.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PlayerQuestItems {
    /// Whether the player has the Amulet of Yendor.
    pub has_amulet: bool,
    /// Whether the player has the Bell of Opening.
    pub has_bell: bool,
    /// Whether the player has the Book of the Dead.
    pub has_book: bool,
    /// Whether the player has the Candelabrum of Invocation (menorah).
    pub has_menorah: bool,
    /// Whether the player has their quest artifact.
    pub has_quest_artifact: bool,
}

/// Player's weapon skill levels and practice progress.
/// Corresponds to `u.weapon_skills[]` and related fields in C.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSkills {
    /// Number of available skill slots for advancement.  From `u.weapon_slots`.
    pub weapon_slots: i32,
    /// Number of skills already advanced.  From `u.skills_advanced`.
    pub skills_advanced: i32,
    /// Per-skill current and max level.  Indexed by `WeaponSkill` discriminant.
    pub skills: Vec<SkillState>,
    /// Whether two-weapon combat is active.  From `u.twoweap`.
    pub two_weapon: bool,
}

/// State of a single weapon/spell skill.
/// Corresponds to `struct skills` in `skills.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillState {
    /// Which skill this entry describes.
    pub skill: WeaponSkill,
    /// Current skill level (restricted, unskilled, basic, skilled, expert, master, grand master).
    pub level: u8,
    /// Maximum achievable skill level.
    pub max_level: u8,
    /// Current practice points toward next advancement.
    pub advance: u16,
}

// ===========================================================================
// Map components
// ===========================================================================

/// A single cell in the dungeon map (ECS component version).
/// Represents `struct rm` (level.locations[x][y]) from `rm.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapCellComponent {
    /// Terrain type at this location.  From `rm.typ`.
    pub terrain: TerrainType,
    /// Display glyph index for rendering.  From `rm.glyph`.
    pub glyph: i32,
    /// Whether this cell has been seen by the hero.  From `rm.seenv`.
    pub seen: bool,
    /// Whether this cell is currently lit.  From `rm.lit`.
    pub lit: bool,
    /// Whether this cell was lit when last visited.  From `rm.waslit`.
    pub waslit: bool,
    /// Trap at this location, if any.
    pub trap: Option<TrapType>,
}

/// An entity that emits light on the map.
/// Derived from the light source system in `light.c`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightEmitter {
    /// Type of light source.
    pub source_type: LightSourceType,
    /// Light radius in map squares.
    pub radius: u8,
    /// Map position X.
    pub x: i16,
    /// Map position Y.
    pub y: i16,
}

/// Classification of a light emitter.
/// Derived from the `LS_xxx` constants in `light.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LightSourceType {
    /// Light from an object (lamp, candle, etc.).
    Object,
    /// Light from a monster (fire elemental, etc.).
    Monster,
    /// Light from a map feature (fountain, etc.).
    Terrain,
}

// ===========================================================================
// Monster instance component
// ===========================================================================

/// Runtime state for a spawned monster on the map.
/// Corresponds to fields from C `struct monst`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonsterInstance {
    /// Monster species definition ID.  From `monst.mnum` / index into `mons[]`.
    pub species: MonsterId,
    /// Unique runtime identifier.  From `monst.m_id`.
    pub id: u32,
    /// Current hit points.  From `monst.mhp`.
    pub hp: i32,
    /// Maximum hit points.  From `monst.mhpmax`.
    pub hp_max: i32,
    /// Current X position.  From `monst.mx`.
    pub x: i16,
    /// Current Y position.  From `monst.my`.
    pub y: i16,
    /// Current experience level.  From `monst.m_lev`.
    pub level: i8,
    /// Current alignment.  From `monst.malign`.
    pub alignment: i8,
    /// Movement points accumulated this turn.  From `monst.movement`.
    pub movement: i32,
    /// Whether the monster is tame (pet).  From `monst.mtame`.
    pub tameness: u8,
    /// Whether the monster is peaceful.  From `monst.mpeaceful`.
    pub peaceful: bool,
    /// Whether the monster is fleeing.  From `monst.mflee`.
    pub fleeing: bool,
    /// Flee timer (turns remaining).  From `monst.mfleetim`.
    pub flee_timer: u8,
    /// Whether the monster is asleep.  From `monst.msleeping`.
    pub sleeping: bool,
    /// Whether the monster is stunned.  From `monst.mstun`.
    pub stunned: bool,
    /// Whether the monster is confused.  From `monst.mconf`.
    pub confused: bool,
    /// Whether the monster is blind.  From `monst.mblinded`.
    pub blinded: u8,
    /// Whether the monster is frozen/paralyzed.  From `monst.mfrozen`.
    pub frozen: u8,
    /// Whether the monster is cancellation-affected.  From `monst.mcan`.
    pub cancelled: bool,
    /// Whether the monster is invisible.  From `monst.minvis`.
    pub invisible: bool,
    /// Whether the monster is trapped.  From `monst.mtrapped`.
    pub trapped: bool,
    /// Individual monster name (if any, e.g. named pet).  From `monst.mnamelth` + name data.
    pub name: Option<String>,
    /// Equipment slots in use (what the monster is wearing/wielding).
    pub worn_mask: EquipSlot,
}
