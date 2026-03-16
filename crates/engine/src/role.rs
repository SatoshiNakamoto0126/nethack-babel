//! Role and race selection system.
//!
//! Defines the 13 player roles and 5 races from NetHack, along with
//! starting attributes, valid combinations, starting inventory, and
//! level-based titles.

use nethack_babel_data::{Alignment, ObjectTypeId, RaceId, RoleId};

use crate::world::Attributes;

// ---------------------------------------------------------------------------
// Role enum
// ---------------------------------------------------------------------------

/// The 13 player roles from NetHack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Archeologist,
    Barbarian,
    Caveperson,
    Healer,
    Knight,
    Monk,
    Priest,
    Ranger,
    Rogue,
    Samurai,
    Tourist,
    Valkyrie,
    Wizard,
}

impl Role {
    /// All roles in canonical order.
    pub const ALL: [Role; 13] = [
        Role::Archeologist,
        Role::Barbarian,
        Role::Caveperson,
        Role::Healer,
        Role::Knight,
        Role::Monk,
        Role::Priest,
        Role::Ranger,
        Role::Rogue,
        Role::Samurai,
        Role::Tourist,
        Role::Valkyrie,
        Role::Wizard,
    ];

    /// Convert a `RoleId` to a `Role`.
    pub fn from_id(id: RoleId) -> Option<Role> {
        Self::ALL.get(id.0 as usize).copied()
    }

    /// Convert a `Role` to a `RoleId`.
    pub fn to_id(self) -> RoleId {
        RoleId(self as u8)
    }

    /// The display name of this role.
    pub fn name(self) -> &'static str {
        match self {
            Role::Archeologist => "Archeologist",
            Role::Barbarian => "Barbarian",
            Role::Caveperson => "Caveperson",
            Role::Healer => "Healer",
            Role::Knight => "Knight",
            Role::Monk => "Monk",
            Role::Priest => "Priest",
            Role::Ranger => "Ranger",
            Role::Rogue => "Rogue",
            Role::Samurai => "Samurai",
            Role::Tourist => "Tourist",
            Role::Valkyrie => "Valkyrie",
            Role::Wizard => "Wizard",
        }
    }
}

// ---------------------------------------------------------------------------
// Race enum
// ---------------------------------------------------------------------------

/// The 5 player races from NetHack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Race {
    Human,
    Elf,
    Dwarf,
    Gnome,
    Orc,
}

impl Race {
    /// All races in canonical order.
    pub const ALL: [Race; 5] = [Race::Human, Race::Elf, Race::Dwarf, Race::Gnome, Race::Orc];

    /// Convert a `RaceId` to a `Race`.
    pub fn from_id(id: RaceId) -> Option<Race> {
        Self::ALL.get(id.0 as usize).copied()
    }

    /// Convert a `Race` to a `RaceId`.
    pub fn to_id(self) -> RaceId {
        RaceId(self as u8)
    }

    /// The display name of this race.
    pub fn name(self) -> &'static str {
        match self {
            Race::Human => "Human",
            Race::Elf => "Elf",
            Race::Dwarf => "Dwarf",
            Race::Gnome => "Gnome",
            Race::Orc => "Orc",
        }
    }
}

// ---------------------------------------------------------------------------
// RoleData
// ---------------------------------------------------------------------------

/// Static data for a player role.
pub struct RoleData {
    /// Base starting attributes (before race modifiers).
    pub base_attrs: Attributes,
    /// Races allowed for this role.
    pub allowed_races: &'static [Race],
    /// Alignments allowed for this role.
    pub allowed_alignments: &'static [Alignment],
    /// Starting maximum HP (role component of `newhp()` initial).
    pub starting_hp: i32,
    /// Starting maximum power/mana (role component of `newpw()` initial).
    pub starting_pw: i32,
    /// HP advancement per level table index (xlev threshold).
    pub xlev: u8,
    /// Skill restrictions: maps weapon skill to maximum skill level.
    pub skill_restrictions: &'static [(WeaponSkillCategory, SkillCap)],
}

/// Simplified weapon skill categories for skill restriction tables.
/// These correspond to the `WeaponSkill` enum values from the data crate,
/// but only the ones commonly restricted by roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeaponSkillCategory {
    Dagger,
    LongSword,
    TwoHandedSword,
    Saber,
    BroadSword,
    ShortSword,
    Quarterstaff,
    Spear,
    Polearms,
    Bow,
    Crossbow,
    Mace,
    Axe,
    Club,
    Flail,
    Hammer,
    MorningStar,
    Whip,
    UnicornHorn,
    Trident,
    Lance,
    PickAxe,
    Knife,
    Dart,
    Shuriken,
    Boomerang,
    Sling,
    AttackSpell,
    HealingSpell,
    DivineSpell,
    EnchantSpell,
    ClericSpell,
    EscapeSpell,
    MatterSpell,
    BareHanded,
    TwoWeapon,
    Riding,
}

/// Maximum skill level a role can attain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillCap {
    Restricted,
    Basic,
    Skilled,
    Expert,
}

// ---------------------------------------------------------------------------
// RaceData
// ---------------------------------------------------------------------------

/// Static data for a player race.
pub struct RaceData {
    /// Attribute modifiers (added to role base).
    pub attr_bonus: AttrModifiers,
    /// Attribute caps per race.
    pub attr_caps: AttrCaps,
    /// Racial abilities granted at level 1.
    pub abilities: &'static [RacialAbility],
    /// Starting HP modifier (race component of `newhp()` initial).
    pub hp_bonus: i32,
    /// Starting PW modifier (race component of `newpw()` initial).
    pub pw_bonus: i32,
}

/// Attribute modifiers from race selection.
#[derive(Debug, Clone, Copy)]
pub struct AttrModifiers {
    pub strength: i8,
    pub dexterity: i8,
    pub constitution: i8,
    pub intelligence: i8,
    pub wisdom: i8,
    pub charisma: i8,
}

/// Maximum attribute values per race.
#[derive(Debug, Clone, Copy)]
pub struct AttrCaps {
    pub strength: u8,
    pub dexterity: u8,
    pub constitution: u8,
    pub intelligence: u8,
    pub wisdom: u8,
    pub charisma: u8,
}

/// Racial innate abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RacialAbility {
    SleepResistance,
    Infravision,
    PoisonResistance,
    Tunneling,
}

// ---------------------------------------------------------------------------
// Role data tables
// ---------------------------------------------------------------------------

/// Retrieve the static data for a role.
///
/// Base attributes are from NetHack's `roles[]` table in `role.c`.
pub fn role_data(role: Role) -> &'static RoleData {
    match role {
        Role::Archeologist => &ARCHEOLOGIST_DATA,
        Role::Barbarian => &BARBARIAN_DATA,
        Role::Caveperson => &CAVEPERSON_DATA,
        Role::Healer => &HEALER_DATA,
        Role::Knight => &KNIGHT_DATA,
        Role::Monk => &MONK_DATA,
        Role::Priest => &PRIEST_DATA,
        Role::Ranger => &RANGER_DATA,
        Role::Rogue => &ROGUE_DATA,
        Role::Samurai => &SAMURAI_DATA,
        Role::Tourist => &TOURIST_DATA,
        Role::Valkyrie => &VALKYRIE_DATA,
        Role::Wizard => &WIZARD_DATA,
    }
}

// Starting attributes from role.c (STR, INT, WIS, DEX, CON, CHA)
// Note: NetHack role.c has { STR, INT, WIS, DEX, CON, CHA } order.

static ARCHEOLOGIST_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 7,
        strength_extra: 0,
        dexterity: 10,
        constitution: 7,
        intelligence: 10,
        wisdom: 10,
        charisma: 7,
    },
    allowed_races: &[Race::Human, Race::Dwarf, Race::Gnome],
    allowed_alignments: &[Alignment::Lawful, Alignment::Neutral],
    starting_hp: 11,
    starting_pw: 1,
    xlev: 14,
    skill_restrictions: &[],
};

static BARBARIAN_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 16,
        strength_extra: 30,
        dexterity: 15,
        constitution: 16,
        intelligence: 7,
        wisdom: 7,
        charisma: 6,
    },
    allowed_races: &[Race::Human, Race::Orc],
    allowed_alignments: &[Alignment::Neutral, Alignment::Chaotic],
    starting_hp: 14,
    starting_pw: 1,
    xlev: 10,
    skill_restrictions: &[],
};

static CAVEPERSON_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 10,
        strength_extra: 0,
        dexterity: 7,
        constitution: 10,
        intelligence: 7,
        wisdom: 7,
        charisma: 7,
    },
    allowed_races: &[Race::Human, Race::Dwarf, Race::Gnome],
    allowed_alignments: &[Alignment::Lawful, Alignment::Neutral],
    starting_hp: 14,
    starting_pw: 1,
    xlev: 10,
    skill_restrictions: &[],
};

static HEALER_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 7,
        strength_extra: 0,
        dexterity: 7,
        constitution: 11,
        intelligence: 11,
        wisdom: 14,
        charisma: 11,
    },
    allowed_races: &[Race::Human, Race::Gnome],
    allowed_alignments: &[Alignment::Neutral],
    starting_hp: 11,
    starting_pw: 1, // infix=1, inrnd=4 => 1 + rnd(4)
    xlev: 20,
    skill_restrictions: &[],
};

static KNIGHT_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 13,
        strength_extra: 0,
        dexterity: 7,
        constitution: 14,
        intelligence: 7,
        wisdom: 14,
        charisma: 17,
    },
    allowed_races: &[Race::Human],
    allowed_alignments: &[Alignment::Lawful],
    starting_hp: 14,
    starting_pw: 1, // infix=1, inrnd=4
    xlev: 10,
    skill_restrictions: &[],
};

static MONK_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 10,
        strength_extra: 0,
        dexterity: 8,
        constitution: 7,
        intelligence: 7,
        wisdom: 14,
        charisma: 8,
    },
    allowed_races: &[Race::Human],
    allowed_alignments: &[Alignment::Lawful, Alignment::Neutral, Alignment::Chaotic],
    starting_hp: 12,
    starting_pw: 2, // infix=2, inrnd=2
    xlev: 10,
    skill_restrictions: &[],
};

static PRIEST_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 7,
        strength_extra: 0,
        dexterity: 7,
        constitution: 7,
        intelligence: 7,
        wisdom: 10,
        charisma: 7,
    },
    allowed_races: &[Race::Human, Race::Elf, Race::Gnome],
    allowed_alignments: &[Alignment::Lawful, Alignment::Neutral, Alignment::Chaotic],
    starting_hp: 12,
    starting_pw: 4, // infix=4, inrnd=3
    xlev: 10,
    skill_restrictions: &[],
};

static RANGER_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 13,
        strength_extra: 0,
        dexterity: 13,
        constitution: 13,
        intelligence: 13,
        wisdom: 13,
        charisma: 7,
    },
    allowed_races: &[Race::Human, Race::Elf, Race::Gnome, Race::Orc],
    allowed_alignments: &[Alignment::Neutral, Alignment::Chaotic],
    starting_hp: 13,
    starting_pw: 1,
    xlev: 12,
    skill_restrictions: &[],
};

static ROGUE_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 7,
        strength_extra: 0,
        dexterity: 10,
        constitution: 7,
        intelligence: 7,
        wisdom: 7,
        charisma: 10,
    },
    allowed_races: &[Race::Human, Race::Orc],
    allowed_alignments: &[Alignment::Chaotic],
    starting_hp: 10,
    starting_pw: 1,
    xlev: 11,
    skill_restrictions: &[],
};

static SAMURAI_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 10,
        strength_extra: 0,
        dexterity: 10,
        constitution: 17,
        intelligence: 6,
        wisdom: 7,
        charisma: 7,
    },
    allowed_races: &[Race::Human],
    allowed_alignments: &[Alignment::Lawful],
    starting_hp: 13,
    starting_pw: 1,
    xlev: 11,
    skill_restrictions: &[],
};

static TOURIST_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 7,
        strength_extra: 0,
        dexterity: 10,
        constitution: 7,
        intelligence: 10,
        wisdom: 6,
        charisma: 10,
    },
    allowed_races: &[Race::Human, Race::Gnome],
    allowed_alignments: &[Alignment::Neutral],
    starting_hp: 8,
    starting_pw: 1,
    xlev: 14,
    skill_restrictions: &[],
};

static VALKYRIE_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 10,
        strength_extra: 0,
        dexterity: 7,
        constitution: 10,
        intelligence: 7,
        wisdom: 7,
        charisma: 7,
    },
    allowed_races: &[Race::Human, Race::Dwarf],
    allowed_alignments: &[Alignment::Lawful, Alignment::Neutral],
    starting_hp: 14,
    starting_pw: 1,
    xlev: 10,
    skill_restrictions: &[],
};

static WIZARD_DATA: RoleData = RoleData {
    base_attrs: Attributes {
        strength: 7,
        strength_extra: 0,
        dexterity: 7,
        constitution: 7,
        intelligence: 10,
        wisdom: 7,
        charisma: 7,
    },
    allowed_races: &[Race::Human, Race::Elf, Race::Gnome, Race::Orc],
    allowed_alignments: &[Alignment::Neutral, Alignment::Chaotic],
    starting_hp: 10,
    starting_pw: 4, // infix=4, inrnd=3
    xlev: 12,
    skill_restrictions: &[],
};

// ---------------------------------------------------------------------------
// Race data tables
// ---------------------------------------------------------------------------

/// Retrieve the static data for a race.
///
/// Attribute modifiers and caps are from NetHack's `races[]` table in `race.c`.
pub fn race_data(race: Race) -> &'static RaceData {
    match race {
        Race::Human => &HUMAN_DATA,
        Race::Elf => &ELF_DATA,
        Race::Dwarf => &DWARF_DATA,
        Race::Gnome => &GNOME_DATA,
        Race::Orc => &ORC_DATA,
    }
}

static HUMAN_DATA: RaceData = RaceData {
    attr_bonus: AttrModifiers {
        strength: 0,
        dexterity: 0,
        constitution: 0,
        intelligence: 0,
        wisdom: 0,
        charisma: 0,
    },
    attr_caps: AttrCaps {
        strength: 18,
        dexterity: 18,
        constitution: 18,
        intelligence: 18,
        wisdom: 18,
        charisma: 18,
    },
    abilities: &[],
    hp_bonus: 2,
    pw_bonus: 1,
};

static ELF_DATA: RaceData = RaceData {
    attr_bonus: AttrModifiers {
        strength: 0,
        dexterity: 1,
        constitution: -1,
        intelligence: 1,
        wisdom: 1,
        charisma: 1,
    },
    attr_caps: AttrCaps {
        strength: 18,
        dexterity: 20,
        constitution: 16,
        intelligence: 20,
        wisdom: 20,
        charisma: 18,
    },
    abilities: &[RacialAbility::Infravision, RacialAbility::SleepResistance],
    hp_bonus: 1,
    pw_bonus: 2,
};

static DWARF_DATA: RaceData = RaceData {
    attr_bonus: AttrModifiers {
        strength: 1,
        dexterity: 0,
        constitution: 1,
        intelligence: -1,
        wisdom: 0,
        charisma: -1,
    },
    attr_caps: AttrCaps {
        strength: 20,
        dexterity: 18,
        constitution: 20,
        intelligence: 16,
        wisdom: 18,
        charisma: 16,
    },
    abilities: &[RacialAbility::Infravision],
    hp_bonus: 4,
    pw_bonus: 0,
};

static GNOME_DATA: RaceData = RaceData {
    attr_bonus: AttrModifiers {
        strength: -1,
        dexterity: 1,
        constitution: 0,
        intelligence: 1,
        wisdom: 0,
        charisma: -1,
    },
    attr_caps: AttrCaps {
        strength: 18,
        dexterity: 20,
        constitution: 18,
        intelligence: 20,
        wisdom: 18,
        charisma: 18,
    },
    abilities: &[RacialAbility::Infravision, RacialAbility::Tunneling],
    hp_bonus: 1,
    pw_bonus: 2,
};

static ORC_DATA: RaceData = RaceData {
    attr_bonus: AttrModifiers {
        strength: 1,
        dexterity: 0,
        constitution: 1,
        intelligence: -1,
        wisdom: -1,
        charisma: -2,
    },
    attr_caps: AttrCaps {
        strength: 20,
        dexterity: 18,
        constitution: 20,
        intelligence: 16,
        wisdom: 16,
        charisma: 16,
    },
    abilities: &[RacialAbility::Infravision, RacialAbility::PoisonResistance],
    hp_bonus: 1,
    pw_bonus: 1,
};

// ---------------------------------------------------------------------------
// Starting attributes
// ---------------------------------------------------------------------------

/// Compute starting attributes by combining role base stats with race
/// modifiers.  Values are clamped to [3, race_cap].
pub fn starting_attributes(role: Role, race: Race) -> Attributes {
    let rd = role_data(role);
    let rr = race_data(race);
    let b = &rd.base_attrs;
    let m = &rr.attr_bonus;
    let c = &rr.attr_caps;

    fn clamp_attr(base: u8, modifier: i8, cap: u8) -> u8 {
        let val = (base as i16 + modifier as i16).clamp(3, cap as i16);
        val as u8
    }

    Attributes {
        strength: clamp_attr(b.strength, m.strength, c.strength),
        strength_extra: b.strength_extra,
        dexterity: clamp_attr(b.dexterity, m.dexterity, c.dexterity),
        constitution: clamp_attr(b.constitution, m.constitution, c.constitution),
        intelligence: clamp_attr(b.intelligence, m.intelligence, c.intelligence),
        wisdom: clamp_attr(b.wisdom, m.wisdom, c.wisdom),
        charisma: clamp_attr(b.charisma, m.charisma, c.charisma),
    }
}

// ---------------------------------------------------------------------------
// Starting inventory
// ---------------------------------------------------------------------------

/// A single item in a role's starting inventory.
#[derive(Debug, Clone)]
pub struct StartingItem {
    /// The object type id (index into the `objects[]` table).
    pub otyp: ObjectTypeId,
    /// Human-readable name for debugging/tests.
    pub name: &'static str,
    /// How many of this item to start with.
    pub quantity: i32,
}

/// Return the starting inventory for the given role.
///
/// Item type IDs correspond to the canonical object table order in
/// NetHack's `objects.c` / the Babel TOML data.  At least 5 roles are
/// fully specified; others get a minimal loadout.
pub fn starting_inventory(role: Role) -> Vec<StartingItem> {
    match role {
        Role::Valkyrie => vec![
            StartingItem {
                otyp: ObjectTypeId(28), // long sword
                name: "long sword",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(84), // small shield
                name: "small shield",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(369), // food ration
                name: "food ration",
                quantity: 1,
            },
        ],
        Role::Wizard => vec![
            StartingItem {
                otyp: ObjectTypeId(36), // quarterstaff
                name: "quarterstaff",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(73), // cloak of magic resistance
                name: "cloak of magic resistance",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(346), // spellbook of force bolt
                name: "spellbook of force bolt",
                quantity: 1,
            },
        ],
        Role::Rogue => vec![
            StartingItem {
                otyp: ObjectTypeId(22), // short sword
                name: "short sword",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(0), // dagger
                name: "dagger",
                quantity: 10,
            },
            StartingItem {
                otyp: ObjectTypeId(56), // leather armor
                name: "leather armor",
                quantity: 1,
            },
        ],
        Role::Barbarian => vec![
            StartingItem {
                otyp: ObjectTypeId(31), // two-handed sword
                name: "two-handed sword",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(10), // axe
                name: "axe",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(64), // ring mail
                name: "ring mail",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(369), // food ration
                name: "food ration",
                quantity: 1,
            },
        ],
        Role::Samurai => vec![
            StartingItem {
                otyp: ObjectTypeId(28), // katana (long sword)
                name: "katana",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(22), // wakizashi (short sword)
                name: "wakizashi",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(20), // yumi (bow)
                name: "yumi",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(16), // ya (arrow)
                name: "ya",
                quantity: 25,
            },
            StartingItem {
                otyp: ObjectTypeId(65), // splint mail
                name: "splint mail",
                quantity: 1,
            },
        ],
        Role::Knight => vec![
            StartingItem {
                otyp: ObjectTypeId(28), // long sword
                name: "long sword",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(40), // lance
                name: "lance",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(64), // ring mail
                name: "ring mail",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(79), // helmet
                name: "helmet",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(84), // small shield
                name: "small shield",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(88), // leather gloves
                name: "leather gloves",
                quantity: 1,
            },
        ],
        // Other roles get a minimal default loadout.
        _ => vec![
            StartingItem {
                otyp: ObjectTypeId(0), // dagger
                name: "dagger",
                quantity: 1,
            },
            StartingItem {
                otyp: ObjectTypeId(369), // food ration
                name: "food ration",
                quantity: 1,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// Level titles
// ---------------------------------------------------------------------------

/// Return the title for a role at a given experience level.
///
/// Uses the `xlev_to_rank()` mapping from NetHack's `botl.c`:
///   level 1-2  => rank 0
///   level 3-5  => rank 1
///   level 6-9  => rank 2
///   level 10-13 => rank 3
///   level 14-17 => rank 4
///   level 18-21 => rank 5
///   level 22-25 => rank 6
///   level 26-29 => rank 7
///   level 30    => rank 8
pub fn role_title(role: Role, level: u8) -> &'static str {
    let rank = xlev_to_rank(level);
    let titles = role_titles(role);
    titles[rank]
}

/// Map an experience level to a rank index (0..=8).
fn xlev_to_rank(xlev: u8) -> usize {
    if xlev <= 2 {
        0
    } else if xlev <= 30 {
        ((xlev as usize) + 2) / 4
    } else {
        8
    }
}

/// Return the 9 male/default titles for a role.
fn role_titles(role: Role) -> [&'static str; 9] {
    match role {
        Role::Archeologist => [
            "Digger",
            "Field Worker",
            "Investigator",
            "Exhumer",
            "Excavator",
            "Spelunker",
            "Speleologist",
            "Collector",
            "Curator",
        ],
        Role::Barbarian => [
            "Plunderer",
            "Pillager",
            "Bandit",
            "Brigand",
            "Raider",
            "Reaver",
            "Slayer",
            "Chieftain",
            "Conqueror",
        ],
        Role::Caveperson => [
            "Troglodyte",
            "Aborigine",
            "Wanderer",
            "Vagrant",
            "Wayfarer",
            "Roamer",
            "Nomad",
            "Rover",
            "Pioneer",
        ],
        Role::Healer => [
            "Rhizotomist",
            "Empiric",
            "Embalmer",
            "Dresser",
            "Medicus ossium",
            "Herbalist",
            "Magister",
            "Physician",
            "Chirurgeon",
        ],
        Role::Knight => [
            "Gallant",
            "Esquire",
            "Bachelor",
            "Sergeant",
            "Knight",
            "Banneret",
            "Chevalier",
            "Seignieur",
            "Paladin",
        ],
        Role::Monk => [
            "Candidate",
            "Novice",
            "Initiate",
            "Student of Stones",
            "Student of Waters",
            "Student of Metals",
            "Student of Winds",
            "Student of Fire",
            "Master",
        ],
        Role::Priest => [
            "Aspirant",
            "Acolyte",
            "Adept",
            "Priest",
            "Curate",
            "Canon",
            "Lama",
            "Patriarch",
            "High Priest",
        ],
        Role::Ranger => [
            "Tenderfoot",
            "Lookout",
            "Trailblazer",
            "Reconnoiterer",
            "Scout",
            "Arbalester",
            "Archer",
            "Sharpshooter",
            "Marksman",
        ],
        Role::Rogue => [
            "Footpad", "Cutpurse", "Rogue", "Pilferer", "Robber", "Burglar", "Filcher", "Magsman",
            "Thief",
        ],
        Role::Samurai => [
            "Hatamoto", "Ronin", "Ninja", "Joshu", "Ryoshu", "Kokushu", "Daimyo", "Kuge", "Shogun",
        ],
        Role::Tourist => [
            "Rambler",
            "Sightseer",
            "Excursionist",
            "Peregrinator",
            "Traveler",
            "Journeyer",
            "Voyager",
            "Explorer",
            "Adventurer",
        ],
        Role::Valkyrie => [
            "Stripling",
            "Skirmisher",
            "Fighter",
            "Man-at-arms",
            "Warrior",
            "Swashbuckler",
            "Hero",
            "Champion",
            "Lord",
        ],
        Role::Wizard => [
            "Evoker",
            "Conjurer",
            "Thaumaturge",
            "Magician",
            "Enchanter",
            "Sorcerer",
            "Necromancer",
            "Wizard",
            "Mage",
        ],
    }
}

/// Return the 9 female titles for a role.
///
/// From C's `role.c`: only some roles have female-specific rank variants.
/// Where no female form exists (`0` in C), the male form is used.
fn role_titles_female(role: Role) -> [&'static str; 9] {
    match role {
        // Barbarian: Plunderess, Chieftainess, Conqueress differ
        Role::Barbarian => [
            "Plunderess",
            "Pillager",
            "Bandit",
            "Brigand",
            "Raider",
            "Reaver",
            "Slayer",
            "Chieftainess",
            "Conqueress",
        ],
        // Healer: Medica ossium, Magistra differ
        Role::Healer => [
            "Rhizotomist",
            "Empiric",
            "Embalmer",
            "Dresser",
            "Medica ossium",
            "Herbalist",
            "Magistra",
            "Physician",
            "Chirurgeon",
        ],
        // Knight: Chevaliere, Dame differ
        Role::Knight => [
            "Gallant",
            "Esquire",
            "Bachelor",
            "Sergeant",
            "Knight",
            "Banneret",
            "Chevaliere",
            "Dame",
            "Paladin",
        ],
        // Priest: Priestess, Canoness, Matriarch, High Priestess differ
        Role::Priest => [
            "Aspirant",
            "Acolyte",
            "Adept",
            "Priestess",
            "Curate",
            "Canoness",
            "Lama",
            "Matriarch",
            "High Priestess",
        ],
        // Ranger: Reconnoiteress, Markswoman differ
        Role::Ranger => [
            "Tenderfoot",
            "Lookout",
            "Trailblazer",
            "Reconnoiteress",
            "Scout",
            "Arbalester",
            "Archer",
            "Sharpshooter",
            "Markswoman",
        ],
        // Rogue: Magswoman differs
        Role::Rogue => [
            "Footpad",
            "Cutpurse",
            "Rogue",
            "Pilferer",
            "Robber",
            "Burglar",
            "Filcher",
            "Magswoman",
            "Thief",
        ],
        // Samurai: Kunoichi differs
        Role::Samurai => [
            "Hatamoto", "Ronin", "Kunoichi", "Joshu", "Ryoshu", "Kokushu", "Daimyo", "Kuge",
            "Shogun",
        ],
        // Tourist: Peregrinatrix differs
        Role::Tourist => [
            "Rambler",
            "Sightseer",
            "Excursionist",
            "Peregrinatrix",
            "Traveler",
            "Journeyer",
            "Voyager",
            "Explorer",
            "Adventurer",
        ],
        // Valkyrie: Woman-at-arms, Heroine, Lady differ
        Role::Valkyrie => [
            "Stripling",
            "Skirmisher",
            "Fighter",
            "Woman-at-arms",
            "Warrior",
            "Swashbuckler",
            "Heroine",
            "Champion",
            "Lady",
        ],
        // Wizard: Enchantress, Sorceress differ
        Role::Wizard => [
            "Evoker",
            "Conjurer",
            "Thaumaturge",
            "Magician",
            "Enchantress",
            "Sorceress",
            "Necromancer",
            "Wizard",
            "Mage",
        ],
        // All other roles have no female-specific titles
        _ => role_titles(role),
    }
}

/// Get the rank title for a role at a given experience level, with gender.
///
/// Equivalent to C's `rank_of()` in `botl.c`.
pub fn rank_of(role: Role, level: u8, is_female: bool) -> &'static str {
    let rank = xlev_to_rank(level);
    let titles = if is_female {
        role_titles_female(role)
    } else {
        role_titles(role)
    };
    titles[rank]
}

/// Get all 9 rank entries for a role as `(min_level, title)` pairs.
pub fn all_ranks(role: Role, is_female: bool) -> Vec<(u8, &'static str)> {
    let titles = if is_female {
        role_titles_female(role)
    } else {
        role_titles(role)
    };
    // The min levels for each rank index come from rank_to_xlev:
    // 0=>1, 1=>3, 2=>6, 3=>10, 4=>14, 5=>18, 6=>22, 7=>26, 8=>30
    const MIN_LEVELS: [u8; 9] = [1, 3, 6, 10, 14, 18, 22, 26, 30];
    MIN_LEVELS
        .iter()
        .zip(titles.iter())
        .map(|(&lvl, &title)| (lvl, title))
        .collect()
}

/// Get the experience level at which the player reaches the next rank.
///
/// Returns `None` if already at maximum rank (level 30).
pub fn next_rank_level(_role: Role, current_level: u8) -> Option<u8> {
    const MIN_LEVELS: [u8; 9] = [1, 3, 6, 10, 14, 18, 22, 26, 30];
    for &lvl in &MIN_LEVELS {
        if lvl > current_level {
            return Some(lvl);
        }
    }
    None
}

/// Player alignment title.
pub fn alignment_title(alignment: Alignment) -> &'static str {
    match alignment {
        Alignment::Lawful => "Lawful",
        Alignment::Neutral => "Neutral",
        Alignment::Chaotic => "Chaotic",
    }
}

/// Full status title string: "Name the Rank".
pub fn full_title(player_name: &str, role: Role, xlevel: u8, is_female: bool) -> String {
    let rank = rank_of(role, xlevel, is_female);
    format!("{} the {}", player_name, rank)
}

// ---------------------------------------------------------------------------
// Valid combination check
// ---------------------------------------------------------------------------

/// Check whether the given role + race + alignment is a valid character
/// combination.
///
/// A combination is valid if:
/// 1. The race is in the role's allowed races list
/// 2. The alignment is in the role's allowed alignments list
/// 3. The alignment is valid for the race (race-specific alignment
///    restrictions from `races[]`)
pub fn valid_combination(role: Role, race: Race, alignment: Alignment) -> bool {
    let rd = role_data(role);

    // Check race is allowed for role.
    if !rd.allowed_races.contains(&race) {
        return false;
    }

    // Check alignment is allowed for role.
    if !rd.allowed_alignments.contains(&alignment) {
        return false;
    }

    // Check race-specific alignment restrictions.
    let race_alignments = race_allowed_alignments(race);
    race_alignments.contains(&alignment)
}

/// Return the alignments allowed for a given race.
///
/// From NetHack's `races[]` table in `role.c`:
///   Human: all three
///   Elf: Chaotic
///   Dwarf: Lawful
///   Gnome: Neutral
///   Orc: Chaotic
fn race_allowed_alignments(race: Race) -> &'static [Alignment] {
    match race {
        Race::Human => &[Alignment::Lawful, Alignment::Neutral, Alignment::Chaotic],
        Race::Elf => &[Alignment::Chaotic],
        Race::Dwarf => &[Alignment::Lawful],
        Race::Gnome => &[Alignment::Neutral],
        Race::Orc => &[Alignment::Chaotic],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- test_all_roles_have_attributes --

    #[test]
    fn test_all_roles_have_attributes() {
        for &role in &Role::ALL {
            let rd = role_data(role);
            let a = &rd.base_attrs;
            // All attributes must be in a sane range [3..25].
            assert!(
                a.strength >= 3 && a.strength <= 25,
                "{:?} STR out of range: {}",
                role,
                a.strength
            );
            assert!(
                a.dexterity >= 3 && a.dexterity <= 25,
                "{:?} DEX out of range: {}",
                role,
                a.dexterity
            );
            assert!(
                a.constitution >= 3 && a.constitution <= 25,
                "{:?} CON out of range: {}",
                role,
                a.constitution
            );
            assert!(
                a.intelligence >= 3 && a.intelligence <= 25,
                "{:?} INT out of range: {}",
                role,
                a.intelligence
            );
            assert!(
                a.wisdom >= 3 && a.wisdom <= 25,
                "{:?} WIS out of range: {}",
                role,
                a.wisdom
            );
            assert!(
                a.charisma >= 3 && a.charisma <= 25,
                "{:?} CHA out of range: {}",
                role,
                a.charisma
            );
            // Must have at least one allowed race and alignment.
            assert!(
                !rd.allowed_races.is_empty(),
                "{:?} has no allowed races",
                role
            );
            assert!(
                !rd.allowed_alignments.is_empty(),
                "{:?} has no allowed alignments",
                role
            );
            // Starting HP must be positive.
            assert!(
                rd.starting_hp > 0,
                "{:?} starting HP not positive: {}",
                role,
                rd.starting_hp
            );
        }
    }

    // -- test_starting_inventory_valkyrie --

    #[test]
    fn test_starting_inventory_valkyrie() {
        let inv = starting_inventory(Role::Valkyrie);
        assert!(inv.len() >= 3, "Valkyrie should have at least 3 items");

        // long sword
        assert!(
            inv.iter()
                .any(|i| i.name == "long sword" && i.quantity == 1),
            "Valkyrie should start with a long sword"
        );
        // small shield
        assert!(
            inv.iter()
                .any(|i| i.name == "small shield" && i.quantity == 1),
            "Valkyrie should start with a small shield"
        );
        // food ration
        assert!(
            inv.iter()
                .any(|i| i.name == "food ration" && i.quantity == 1),
            "Valkyrie should start with a food ration"
        );
    }

    // -- test_starting_inventory_wizard --

    #[test]
    fn test_starting_inventory_wizard() {
        let inv = starting_inventory(Role::Wizard);
        assert!(inv.len() >= 3, "Wizard should have at least 3 items");

        assert!(
            inv.iter()
                .any(|i| i.name == "quarterstaff" && i.quantity == 1),
            "Wizard should start with a quarterstaff"
        );
        assert!(
            inv.iter()
                .any(|i| i.name == "cloak of magic resistance" && i.quantity == 1),
            "Wizard should start with cloak of magic resistance"
        );
    }

    // -- test_starting_inventory_rogue --

    #[test]
    fn test_starting_inventory_rogue() {
        let inv = starting_inventory(Role::Rogue);
        assert!(inv.len() >= 3, "Rogue should have at least 3 items");

        assert!(
            inv.iter()
                .any(|i| i.name == "short sword" && i.quantity == 1),
            "Rogue should start with a short sword"
        );
        assert!(
            inv.iter().any(|i| i.name == "dagger" && i.quantity == 10),
            "Rogue should start with 10 daggers"
        );
        assert!(
            inv.iter()
                .any(|i| i.name == "leather armor" && i.quantity == 1),
            "Rogue should start with leather armor"
        );
    }

    // -- test_starting_inventory_barbarian --

    #[test]
    fn test_starting_inventory_barbarian() {
        let inv = starting_inventory(Role::Barbarian);
        assert!(inv.len() >= 3, "Barbarian should have at least 3 items");

        assert!(
            inv.iter()
                .any(|i| i.name == "two-handed sword" && i.quantity == 1),
            "Barbarian should start with a two-handed sword"
        );
        assert!(
            inv.iter().any(|i| i.name == "axe" && i.quantity == 1),
            "Barbarian should start with an axe"
        );
        assert!(
            inv.iter().any(|i| i.name == "ring mail" && i.quantity == 1),
            "Barbarian should start with ring mail"
        );
    }

    // -- test_starting_inventory_samurai --

    #[test]
    fn test_starting_inventory_samurai() {
        let inv = starting_inventory(Role::Samurai);
        assert!(inv.len() >= 4, "Samurai should have at least 4 items");

        assert!(
            inv.iter().any(|i| i.name == "katana" && i.quantity == 1),
            "Samurai should start with a katana"
        );
        assert!(
            inv.iter().any(|i| i.name == "wakizashi" && i.quantity == 1),
            "Samurai should start with a wakizashi"
        );
    }

    // -- test_starting_inventory_knight --

    #[test]
    fn test_starting_inventory_knight() {
        let inv = starting_inventory(Role::Knight);
        assert!(inv.len() >= 4, "Knight should have at least 4 items");

        assert!(
            inv.iter()
                .any(|i| i.name == "long sword" && i.quantity == 1),
            "Knight should start with a long sword"
        );
        assert!(
            inv.iter().any(|i| i.name == "lance" && i.quantity == 1),
            "Knight should start with a lance"
        );
    }

    // -- test_race_attribute_modifiers --

    #[test]
    fn test_race_attribute_modifiers() {
        // Elf: DEX +1, CON -1, INT +1, WIS +1, CHA +1
        let elf = race_data(Race::Elf);
        assert_eq!(elf.attr_bonus.dexterity, 1, "Elf should have DEX +1");
        assert_eq!(elf.attr_bonus.constitution, -1, "Elf should have CON -1");
        assert_eq!(elf.attr_bonus.intelligence, 1, "Elf should have INT +1");

        // Dwarf: STR +1, CON +1, INT -1, CHA -1
        let dwarf = race_data(Race::Dwarf);
        assert_eq!(dwarf.attr_bonus.strength, 1, "Dwarf should have STR +1");
        assert_eq!(dwarf.attr_bonus.constitution, 1, "Dwarf should have CON +1");
        assert_eq!(
            dwarf.attr_bonus.intelligence, -1,
            "Dwarf should have INT -1"
        );

        // Orc: STR +1, CON +1, INT -1, WIS -1, CHA -2
        let orc = race_data(Race::Orc);
        assert_eq!(orc.attr_bonus.strength, 1, "Orc should have STR +1");
        assert_eq!(orc.attr_bonus.charisma, -2, "Orc should have CHA -2");

        // Gnome: STR -1, DEX +1, INT +1, CHA -1
        let gnome = race_data(Race::Gnome);
        assert_eq!(gnome.attr_bonus.strength, -1, "Gnome should have STR -1");
        assert_eq!(gnome.attr_bonus.dexterity, 1, "Gnome should have DEX +1");

        // Human: no modifiers
        let human = race_data(Race::Human);
        assert_eq!(human.attr_bonus.strength, 0, "Human should have STR +0");
        assert_eq!(human.attr_bonus.dexterity, 0, "Human should have DEX +0");
    }

    // -- test_starting_attributes_combines --

    #[test]
    fn test_starting_attributes_combines() {
        // Wizard (base STR 7) + Elf (STR +0) = STR 7
        let attrs = starting_attributes(Role::Wizard, Race::Elf);
        assert_eq!(attrs.strength, 7);
        // Wizard (base DEX 7) + Elf (DEX +1) = DEX 8
        assert_eq!(attrs.dexterity, 8);
        // Wizard (base INT 10) + Elf (INT +1) = INT 11
        assert_eq!(attrs.intelligence, 11);

        // Barbarian (base STR 16) + Orc (STR +1) = STR 17
        let attrs2 = starting_attributes(Role::Barbarian, Race::Orc);
        assert_eq!(attrs2.strength, 17);
        // Barbarian STR_EXTRA preserved
        assert_eq!(attrs2.strength_extra, 30);
    }

    // -- test_starting_attributes_clamped --

    #[test]
    fn test_starting_attributes_clamped() {
        // Knight (CHA 17) + Human (CHA +0, cap 18) = 17
        let attrs = starting_attributes(Role::Knight, Race::Human);
        assert_eq!(attrs.charisma, 17);

        // Orc CHA cap is 16, so Wizard (CHA 7) + Orc (CHA -2) = 5, cap 16 ok
        let attrs2 = starting_attributes(Role::Wizard, Race::Orc);
        assert_eq!(attrs2.charisma, 5);
    }

    // -- test_valid_combinations --

    #[test]
    fn test_valid_combinations() {
        // Valkyrie can be Human/Dwarf, not Elf
        assert!(valid_combination(
            Role::Valkyrie,
            Race::Human,
            Alignment::Neutral
        ));
        assert!(valid_combination(
            Role::Valkyrie,
            Race::Dwarf,
            Alignment::Lawful
        ));
        assert!(
            !valid_combination(Role::Valkyrie, Race::Elf, Alignment::Lawful),
            "Valkyrie cannot be Elf"
        );
        assert!(
            !valid_combination(Role::Valkyrie, Race::Gnome, Alignment::Neutral),
            "Valkyrie cannot be Gnome"
        );
        assert!(
            !valid_combination(Role::Valkyrie, Race::Orc, Alignment::Chaotic),
            "Valkyrie cannot be Orc"
        );
    }

    // -- test_valid_combination_alignment_matters --

    #[test]
    fn test_valid_combination_alignment_matters() {
        // Knight must be Human Lawful
        assert!(valid_combination(
            Role::Knight,
            Race::Human,
            Alignment::Lawful
        ));
        assert!(
            !valid_combination(Role::Knight, Race::Human, Alignment::Chaotic),
            "Knight cannot be Chaotic"
        );
        assert!(
            !valid_combination(Role::Knight, Race::Human, Alignment::Neutral),
            "Knight cannot be Neutral"
        );
    }

    // -- test_valid_combination_race_alignment_restriction --

    #[test]
    fn test_valid_combination_race_alignment_restriction() {
        // Ranger allows Elf and Chaotic alignment, Elf is Chaotic only.
        assert!(valid_combination(
            Role::Ranger,
            Race::Elf,
            Alignment::Chaotic
        ));
        // Ranger allows Neutral, but Elf only allows Chaotic.
        assert!(
            !valid_combination(Role::Ranger, Race::Elf, Alignment::Neutral),
            "Elf can only be Chaotic"
        );

        // Priest allows all alignments, Dwarf is Lawful only.
        assert!(valid_combination(
            Role::Priest,
            Race::Elf,
            Alignment::Chaotic
        ));
        // Gnome is Neutral only.
        assert!(valid_combination(
            Role::Priest,
            Race::Gnome,
            Alignment::Neutral
        ));
        assert!(!valid_combination(
            Role::Priest,
            Race::Gnome,
            Alignment::Lawful
        ));
    }

    // -- test_role_titles_level_1 --

    #[test]
    fn test_role_titles_level_1() {
        assert_eq!(role_title(Role::Valkyrie, 1), "Stripling");
        assert_eq!(role_title(Role::Wizard, 1), "Evoker");
        assert_eq!(role_title(Role::Rogue, 1), "Footpad");
        assert_eq!(role_title(Role::Barbarian, 1), "Plunderer");
        assert_eq!(role_title(Role::Archeologist, 1), "Digger");
    }

    // -- test_role_titles_level_30 --

    #[test]
    fn test_role_titles_level_30() {
        assert_eq!(role_title(Role::Valkyrie, 30), "Lord");
        assert_eq!(role_title(Role::Wizard, 30), "Mage");
        assert_eq!(role_title(Role::Rogue, 30), "Thief");
        assert_eq!(role_title(Role::Barbarian, 30), "Conqueror");
        assert_eq!(role_title(Role::Archeologist, 30), "Curator");
        assert_eq!(role_title(Role::Samurai, 30), "Shogun");
        assert_eq!(role_title(Role::Knight, 30), "Paladin");
    }

    // -- test_role_titles_mid_levels --

    #[test]
    fn test_role_titles_mid_levels() {
        // Level 10 => rank 3
        assert_eq!(role_title(Role::Valkyrie, 10), "Man-at-arms");
        // Level 14 => rank 4
        assert_eq!(role_title(Role::Valkyrie, 14), "Warrior");
        // Level 22 => rank 6
        assert_eq!(role_title(Role::Valkyrie, 22), "Hero");
    }

    // -- test_xlev_to_rank_boundaries --

    #[test]
    fn test_xlev_to_rank_boundaries() {
        assert_eq!(xlev_to_rank(1), 0);
        assert_eq!(xlev_to_rank(2), 0);
        assert_eq!(xlev_to_rank(3), 1);
        assert_eq!(xlev_to_rank(5), 1);
        assert_eq!(xlev_to_rank(6), 2);
        assert_eq!(xlev_to_rank(9), 2);
        assert_eq!(xlev_to_rank(10), 3);
        assert_eq!(xlev_to_rank(13), 3);
        assert_eq!(xlev_to_rank(14), 4);
        assert_eq!(xlev_to_rank(17), 4);
        assert_eq!(xlev_to_rank(18), 5);
        assert_eq!(xlev_to_rank(21), 5);
        assert_eq!(xlev_to_rank(22), 6);
        assert_eq!(xlev_to_rank(25), 6);
        assert_eq!(xlev_to_rank(26), 7);
        assert_eq!(xlev_to_rank(29), 7);
        assert_eq!(xlev_to_rank(30), 8);
    }

    // -- test_all_roles_have_titles --

    #[test]
    fn test_all_roles_have_titles() {
        for &role in &Role::ALL {
            // Every role must have 9 non-empty titles.
            let titles = role_titles(role);
            for (i, title) in titles.iter().enumerate() {
                assert!(!title.is_empty(), "{:?} title rank {} is empty", role, i);
            }
            // Level 1 and 30 should produce valid titles.
            let t1 = role_title(role, 1);
            let t30 = role_title(role, 30);
            assert!(!t1.is_empty());
            assert!(!t30.is_empty());
            assert_ne!(t1, t30, "{:?} level 1 and 30 should differ", role);
        }
    }

    // -- test_racial_abilities --

    #[test]
    fn test_racial_abilities() {
        let elf = race_data(Race::Elf);
        assert!(elf.abilities.contains(&RacialAbility::SleepResistance));
        assert!(elf.abilities.contains(&RacialAbility::Infravision));

        let dwarf = race_data(Race::Dwarf);
        assert!(dwarf.abilities.contains(&RacialAbility::Infravision));
        assert!(!dwarf.abilities.contains(&RacialAbility::SleepResistance));

        let orc = race_data(Race::Orc);
        assert!(orc.abilities.contains(&RacialAbility::PoisonResistance));
        assert!(orc.abilities.contains(&RacialAbility::Infravision));

        let gnome = race_data(Race::Gnome);
        assert!(gnome.abilities.contains(&RacialAbility::Tunneling));
        assert!(gnome.abilities.contains(&RacialAbility::Infravision));

        let human = race_data(Race::Human);
        assert!(human.abilities.is_empty());
    }

    // -- test_role_from_id_roundtrip --

    #[test]
    fn test_role_from_id_roundtrip() {
        for &role in &Role::ALL {
            let id = role.to_id();
            let back = Role::from_id(id).expect("should roundtrip");
            assert_eq!(back, role);
        }
        assert!(Role::from_id(RoleId(255)).is_none());
    }

    // -- test_race_from_id_roundtrip --

    #[test]
    fn test_race_from_id_roundtrip() {
        for &race in &Race::ALL {
            let id = race.to_id();
            let back = Race::from_id(id).expect("should roundtrip");
            assert_eq!(back, race);
        }
        assert!(Race::from_id(RaceId(255)).is_none());
    }

    // -- test_race_hp_pw_bonuses --

    #[test]
    fn test_race_hp_pw_bonuses() {
        assert_eq!(race_data(Race::Human).hp_bonus, 2);
        assert_eq!(race_data(Race::Elf).hp_bonus, 1);
        assert_eq!(race_data(Race::Dwarf).hp_bonus, 4);
        assert_eq!(race_data(Race::Gnome).hp_bonus, 1);
        assert_eq!(race_data(Race::Orc).hp_bonus, 1);

        assert_eq!(race_data(Race::Human).pw_bonus, 1);
        assert_eq!(race_data(Race::Elf).pw_bonus, 2);
        assert_eq!(race_data(Race::Dwarf).pw_bonus, 0);
        assert_eq!(race_data(Race::Gnome).pw_bonus, 2);
        assert_eq!(race_data(Race::Orc).pw_bonus, 1);
    }

    // -- test_all_roles_have_at_least_one_valid_combo --

    #[test]
    fn test_all_roles_have_at_least_one_valid_combo() {
        for &role in &Role::ALL {
            let mut found = false;
            for &race in &Race::ALL {
                for &align in &[Alignment::Lawful, Alignment::Neutral, Alignment::Chaotic] {
                    if valid_combination(role, race, align) {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            assert!(
                found,
                "{:?} has no valid role/race/alignment combination",
                role
            );
        }
    }

    // -- test_starting_inventory_all_roles_nonempty --

    #[test]
    fn test_starting_inventory_all_roles_nonempty() {
        for &role in &Role::ALL {
            let inv = starting_inventory(role);
            assert!(!inv.is_empty(), "{:?} starting inventory is empty", role);
            for item in &inv {
                assert!(item.quantity > 0, "{:?} has item with qty 0", role);
                assert!(!item.name.is_empty(), "{:?} has unnamed item", role);
            }
        }
    }

    // -- test_rank_of_level_1_all_roles --

    #[test]
    fn test_rank_of_level_1_all_roles() {
        assert_eq!(rank_of(Role::Archeologist, 1, false), "Digger");
        assert_eq!(rank_of(Role::Barbarian, 1, false), "Plunderer");
        assert_eq!(rank_of(Role::Caveperson, 1, false), "Troglodyte");
        assert_eq!(rank_of(Role::Healer, 1, false), "Rhizotomist");
        assert_eq!(rank_of(Role::Knight, 1, false), "Gallant");
        assert_eq!(rank_of(Role::Monk, 1, false), "Candidate");
        assert_eq!(rank_of(Role::Priest, 1, false), "Aspirant");
        assert_eq!(rank_of(Role::Ranger, 1, false), "Tenderfoot");
        assert_eq!(rank_of(Role::Rogue, 1, false), "Footpad");
        assert_eq!(rank_of(Role::Samurai, 1, false), "Hatamoto");
        assert_eq!(rank_of(Role::Tourist, 1, false), "Rambler");
        assert_eq!(rank_of(Role::Valkyrie, 1, false), "Stripling");
        assert_eq!(rank_of(Role::Wizard, 1, false), "Evoker");
    }

    // -- test_rank_of_level_30_all_roles --

    #[test]
    fn test_rank_of_level_30_all_roles() {
        assert_eq!(rank_of(Role::Archeologist, 30, false), "Curator");
        assert_eq!(rank_of(Role::Barbarian, 30, false), "Conqueror");
        assert_eq!(rank_of(Role::Caveperson, 30, false), "Pioneer");
        assert_eq!(rank_of(Role::Healer, 30, false), "Chirurgeon");
        assert_eq!(rank_of(Role::Knight, 30, false), "Paladin");
        assert_eq!(rank_of(Role::Monk, 30, false), "Master");
        assert_eq!(rank_of(Role::Priest, 30, false), "High Priest");
        assert_eq!(rank_of(Role::Ranger, 30, false), "Marksman");
        assert_eq!(rank_of(Role::Rogue, 30, false), "Thief");
        assert_eq!(rank_of(Role::Samurai, 30, false), "Shogun");
        assert_eq!(rank_of(Role::Tourist, 30, false), "Adventurer");
        assert_eq!(rank_of(Role::Valkyrie, 30, false), "Lord");
        assert_eq!(rank_of(Role::Wizard, 30, false), "Mage");
    }

    // -- test_rank_of_mid_level_thresholds --

    #[test]
    fn test_rank_of_mid_level_thresholds() {
        // Level 6 => rank 2 (Wanderer for Caveperson)
        assert_eq!(rank_of(Role::Caveperson, 6, false), "Wanderer");
        // Level 9 => still rank 2
        assert_eq!(rank_of(Role::Caveperson, 9, false), "Wanderer");
        // Level 10 => rank 3
        assert_eq!(rank_of(Role::Caveperson, 10, false), "Vagrant");
        // Level 18 => rank 5
        assert_eq!(rank_of(Role::Knight, 18, false), "Banneret");
        // Level 26 => rank 7
        assert_eq!(rank_of(Role::Monk, 26, false), "Student of Fire");
    }

    // -- test_rank_of_female_priest --

    #[test]
    fn test_rank_of_female_priest() {
        assert_eq!(rank_of(Role::Priest, 1, true), "Aspirant");
        assert_eq!(rank_of(Role::Priest, 10, true), "Priestess");
        assert_eq!(rank_of(Role::Priest, 18, true), "Canoness");
        assert_eq!(rank_of(Role::Priest, 26, true), "Matriarch");
        assert_eq!(rank_of(Role::Priest, 30, true), "High Priestess");
    }

    // -- test_rank_of_female_barbarian --

    #[test]
    fn test_rank_of_female_barbarian() {
        assert_eq!(rank_of(Role::Barbarian, 1, true), "Plunderess");
        assert_eq!(rank_of(Role::Barbarian, 26, true), "Chieftainess");
        assert_eq!(rank_of(Role::Barbarian, 30, true), "Conqueress");
    }

    // -- test_rank_of_female_valkyrie --

    #[test]
    fn test_rank_of_female_valkyrie() {
        assert_eq!(rank_of(Role::Valkyrie, 10, true), "Woman-at-arms");
        assert_eq!(rank_of(Role::Valkyrie, 22, true), "Heroine");
        assert_eq!(rank_of(Role::Valkyrie, 30, true), "Lady");
    }

    // -- test_rank_of_female_wizard --

    #[test]
    fn test_rank_of_female_wizard() {
        assert_eq!(rank_of(Role::Wizard, 14, true), "Enchantress");
        assert_eq!(rank_of(Role::Wizard, 18, true), "Sorceress");
        // Non-gendered ranks stay the same
        assert_eq!(rank_of(Role::Wizard, 1, true), "Evoker");
        assert_eq!(rank_of(Role::Wizard, 30, true), "Mage");
    }

    // -- test_rank_of_female_knight --

    #[test]
    fn test_rank_of_female_knight() {
        assert_eq!(rank_of(Role::Knight, 22, true), "Chevaliere");
        assert_eq!(rank_of(Role::Knight, 26, true), "Dame");
    }

    // -- test_rank_of_female_rogue --

    #[test]
    fn test_rank_of_female_rogue() {
        assert_eq!(rank_of(Role::Rogue, 26, true), "Magswoman");
        // Non-gendered stay the same
        assert_eq!(rank_of(Role::Rogue, 1, true), "Footpad");
    }

    // -- test_rank_of_female_samurai --

    #[test]
    fn test_rank_of_female_samurai() {
        assert_eq!(rank_of(Role::Samurai, 6, true), "Kunoichi");
        assert_eq!(rank_of(Role::Samurai, 1, true), "Hatamoto");
    }

    // -- test_next_rank_level --

    #[test]
    fn test_next_rank_level() {
        // From level 1, next rank at level 3
        assert_eq!(next_rank_level(Role::Wizard, 1), Some(3));
        // From level 3, next rank at level 6
        assert_eq!(next_rank_level(Role::Wizard, 3), Some(6));
        // From level 5, next rank at level 6
        assert_eq!(next_rank_level(Role::Wizard, 5), Some(6));
        // From level 26, next rank at level 30
        assert_eq!(next_rank_level(Role::Wizard, 26), Some(30));
    }

    // -- test_next_rank_level_at_max --

    #[test]
    fn test_next_rank_level_at_max() {
        assert_eq!(next_rank_level(Role::Wizard, 30), None);
        assert_eq!(next_rank_level(Role::Valkyrie, 30), None);
    }

    // -- test_full_title --

    #[test]
    fn test_full_title() {
        assert_eq!(
            full_title("Gandalf", Role::Wizard, 30, false),
            "Gandalf the Mage"
        );
        assert_eq!(
            full_title("Brienne", Role::Knight, 26, true),
            "Brienne the Dame"
        );
        assert_eq!(
            full_title("Conan", Role::Barbarian, 1, false),
            "Conan the Plunderer"
        );
    }

    // -- test_alignment_title --

    #[test]
    fn test_alignment_title() {
        assert_eq!(alignment_title(Alignment::Lawful), "Lawful");
        assert_eq!(alignment_title(Alignment::Neutral), "Neutral");
        assert_eq!(alignment_title(Alignment::Chaotic), "Chaotic");
    }

    // -- test_all_ranks_returns_9_entries --

    #[test]
    fn test_all_ranks_returns_9_entries() {
        for &role in &Role::ALL {
            let ranks = all_ranks(role, false);
            assert_eq!(ranks.len(), 9, "{:?} should have 9 rank entries", role);
            // First entry should be level 1
            assert_eq!(ranks[0].0, 1);
            // Last entry should be level 30
            assert_eq!(ranks[8].0, 30);
            // All titles non-empty
            for (lvl, title) in &ranks {
                assert!(!title.is_empty(), "{:?} rank at level {} empty", role, lvl);
            }
        }
    }

    // -- test_all_ranks_female_variants --

    #[test]
    fn test_all_ranks_female_variants() {
        let ranks = all_ranks(Role::Priest, true);
        assert_eq!(ranks.len(), 9);
        assert_eq!(ranks[3].1, "Priestess");
        assert_eq!(ranks[8].1, "High Priestess");

        let ranks_m = all_ranks(Role::Priest, false);
        assert_eq!(ranks_m[3].1, "Priest");
        assert_eq!(ranks_m[8].1, "High Priest");
    }
}
