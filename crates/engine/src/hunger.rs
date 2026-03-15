//! Hunger and eating system for NetHack Babel.
//!
//! Implements nutrition tracking, food eating, corpse effects, tin eating,
//! choking, starvation, and conduct tracking.
//!
//! Reference: `specs/hunger.md` (extracted from `src/eat.c`, `include/hack.h`).

use hecs::Entity;
use rand::Rng;

use nethack_babel_data::{
    Material, MonsterDef, MonsterFlags, ObjectDef, ResistanceSet,
};

use crate::event::{DeathCause, EngineEvent, HpSource, HungerLevel, StatusEffect};
use crate::world::{GameWorld, HitPoints, Nutrition};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Starting nutrition for a new game.
pub const INITIAL_NUTRITION: i32 = 900;

/// Nutrition threshold above which the hero is satiated.
pub const SATIATED_THRESHOLD: i32 = 1000;

/// Nutrition threshold above which the hero is not hungry.
pub const NOT_HUNGRY_THRESHOLD: i32 = 150;

/// Nutrition threshold above which the hero is merely hungry (not weak).
pub const HUNGRY_THRESHOLD: i32 = 50;

/// Choking triggers when nutrition reaches this value while eating satiated.
pub const CHOKING_THRESHOLD: i32 = 2000;

/// "Nearly full" warning threshold.
pub const NEARLY_FULL_THRESHOLD: i32 = 1500;

/// Survival chance denominator for choking (1 in 20 = 5%).
pub const CHOKING_SURVIVAL_DENOM: u32 = 20;

// ---------------------------------------------------------------------------
// Food definition snapshot — extracted from ObjectDef for pure functions
// ---------------------------------------------------------------------------

/// Lightweight snapshot of a food item's static properties, extracted from
/// `ObjectDef` so that eating logic can be tested without the full data
/// tables.
#[derive(Debug, Clone)]
pub struct FoodDef {
    /// Display name of the food.
    pub name: String,
    /// Base nutrition value from `oc_nutrition`.
    pub nutrition: i32,
    /// Eating time from `oc_delay`.
    pub oc_delay: i32,
    /// Material of the food item (Veggy, Flesh, etc.).
    pub material: Material,
    /// Whether this food is a corpse.
    pub is_corpse: bool,
    /// Whether this food is a tin.
    pub is_tin: bool,
    /// Whether this food is a glob.
    pub is_glob: bool,
    /// Weight of the food item (relevant for corpses/globs eating time).
    pub weight: u32,
}

impl FoodDef {
    /// Create a FoodDef from an ObjectDef.
    pub fn from_object_def(obj: &ObjectDef) -> Self {
        Self {
            name: obj.name.clone(),
            nutrition: obj.nutrition as i32,
            oc_delay: obj.use_delay as i32,
            material: obj.material,
            is_corpse: false,
            is_tin: false,
            is_glob: false,
            weight: obj.weight as u32,
        }
    }
}

/// Lightweight snapshot of a monster species relevant to corpse eating.
#[derive(Debug, Clone)]
pub struct CorpseDef {
    /// Monster display name.
    pub name: String,
    /// Monster base level.
    pub base_level: i8,
    /// Corpse weight.
    pub corpse_weight: u16,
    /// Corpse nutrition.
    pub corpse_nutrition: u16,
    /// Resistances conveyed by eating.
    pub conveys: ResistanceSet,
    /// Monster flags (M1/M2/M3).
    pub flags: MonsterFlags,
    /// Whether the monster is poisonous (M1_POIS).
    pub poisonous: bool,
    /// Whether the monster is acidic (M1_ACID).
    pub acidic: bool,
    /// Whether the monster's flesh petrifies (cockatrice-like).
    pub flesh_petrifies: bool,
    /// Whether the monster is a giant (M2_GIANT).
    pub is_giant: bool,
    /// Whether the monster is domestic (M2_DOMESTIC — dog/cat).
    pub is_domestic: bool,
    /// Whether this is a same-race corpse for the eater (cannibalism).
    pub is_same_race: bool,
    /// Whether the eater's race allows cannibalism (Orc/Cave Dweller).
    pub cannibal_allowed: bool,
    /// Whether this monster conveys telepathy.
    pub conveys_telepathy: bool,
    /// Whether this monster conveys teleportitis.
    pub conveys_teleport: bool,
    /// Whether this is a non-rotting corpse (lizard, lichen, acid blob, Rider).
    pub nonrotting: bool,
    /// Whether eating this corpse is vegan-safe.
    pub is_vegan: bool,
    /// Whether eating this corpse is vegetarian-safe.
    pub is_vegetarian: bool,
}

impl CorpseDef {
    /// Build a CorpseDef from a MonsterDef with player context.
    pub fn from_monster_def(
        mon: &MonsterDef,
        is_same_race: bool,
        cannibal_allowed: bool,
    ) -> Self {
        let flags = mon.flags;
        Self {
            name: mon.names.male.clone(),
            base_level: mon.base_level,
            corpse_weight: mon.corpse_weight,
            corpse_nutrition: mon.corpse_nutrition,
            conveys: mon.conveys,
            flags,
            poisonous: flags.contains(MonsterFlags::POIS),
            acidic: flags.contains(MonsterFlags::ACID),
            // Cockatrice-family: symbol 'c' with stoning touch.
            // In practice this is determined by specific PM_* ids;
            // we approximate via the STONE resistance conveyance.
            flesh_petrifies: mon.conveys.contains(ResistanceSet::STONE)
                && mon.symbol == 'c',
            is_giant: flags.contains(MonsterFlags::GIANT),
            is_domestic: flags.contains(MonsterFlags::DOMESTIC),
            is_same_race,
            cannibal_allowed,
            conveys_telepathy: false, // Set by caller based on PM check
            conveys_teleport: mon.conveys.contains(ResistanceSet::empty())
                && flags.contains(MonsterFlags::TPORT),
            nonrotting: false, // Set by caller based on PM check
            is_vegan: !flags.intersects(
                MonsterFlags::CARNIVORE | MonsterFlags::HERBIVORE
                    | MonsterFlags::OMNIVORE,
            ),
            is_vegetarian: !flags.contains(MonsterFlags::CARNIVORE)
                || flags.contains(MonsterFlags::HERBIVORE),
        }
    }
}

// ---------------------------------------------------------------------------
// Tin definition
// ---------------------------------------------------------------------------

/// Tin variety, corresponding to `tintxts[]` index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TinVariety {
    Rotten = 0,
    Homemade = 1,
    Soup = 2,
    FrenchFried = 3,
    Pickled = 4,
    Boiled = 5,
    Smoked = 6,
    Dried = 7,
    DeepFried = 8,
    Szechuan = 9,
    Broiled = 10,
    StirFried = 11,
    Sauteed = 12,
    Candied = 13,
    Pureed = 14,
    Spinach = 15,
}

/// Nutrition for each tin variety (index 0..14).
const TIN_NUTRITION: [i32; 15] = [
    -50, 50, 20, 40, 40, 50, 50, 55, 60, 70, 80, 80, 95, 100, 500,
];

/// Whether a tin variety is greasy (french fried, deep fried, stir fried).
pub fn tin_is_greasy(variety: TinVariety) -> bool {
    matches!(
        variety,
        TinVariety::FrenchFried | TinVariety::DeepFried | TinVariety::StirFried
    )
}

// ---------------------------------------------------------------------------
// Hunger level computation
// ---------------------------------------------------------------------------

/// Map a raw nutrition counter to a HungerLevel.
///
/// Thresholds: >1000 Satiated, >150 NotHungry, >50 Hungry, >0 Weak,
/// <=0 Fainting.
pub fn nutrition_to_hunger_level(nutrition: i32) -> HungerLevel {
    if nutrition > SATIATED_THRESHOLD {
        HungerLevel::Satiated
    } else if nutrition > NOT_HUNGRY_THRESHOLD {
        HungerLevel::NotHungry
    } else if nutrition > HUNGRY_THRESHOLD {
        HungerLevel::Hungry
    } else if nutrition > 0 {
        HungerLevel::Weak
    } else {
        HungerLevel::Fainting
    }
}

/// Compute the starvation death threshold for a given constitution.
///
/// Death occurs when `nutrition < -(100 + 10 * con)`.
/// Note: strict less-than, so exactly at the threshold the hero survives.
pub fn starvation_threshold(con: u8) -> i32 {
    -(100 + 10 * con as i32)
}

/// Check whether the hero should die of starvation at the given nutrition
/// and constitution values.
pub fn should_starve(nutrition: i32, con: u8) -> bool {
    nutrition < starvation_threshold(con)
}

// ---------------------------------------------------------------------------
// Eating time calculation
// ---------------------------------------------------------------------------

/// Calculate eating time for normal food (non-corpse, non-glob).
pub fn eating_time_normal(oc_delay: i32) -> i32 {
    oc_delay.max(1)
}

/// Calculate eating time for a corpse.
///
/// `reqtime = 3 + (cwt >> 6)` where cwt is the corpse weight.
pub fn eating_time_corpse(corpse_weight: u32) -> i32 {
    3 + (corpse_weight >> 6) as i32
}

/// Calculate eating time for a glob.
///
/// `reqtime = 3 + (owt >> 6)` where owt is the glob's current weight.
pub fn eating_time_glob(glob_weight: u32) -> i32 {
    3 + (glob_weight >> 6) as i32
}

// ---------------------------------------------------------------------------
// Choking
// ---------------------------------------------------------------------------

/// Result of a choking check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChokeOutcome {
    /// No choking (nutrition below threshold or not eating while satiated).
    NoChoke,
    /// Survived by vomiting.
    Vomited {
        /// Nutrition lost from vomiting.
        nutrition_lost: i32,
    },
    /// Choked to death.
    Choked,
}

/// Check for choking when eating while satiated.
///
/// `canchoke` must be true (set when eating starts while satiated).
/// `nutrition` is the current nutrition after adding food.
/// `breathless` and `has_hunger_property` provide guaranteed survival.
/// `strangled` blocks the random survival chance.
///
/// Survival chance for normal hero: 5% (1 in 20). NOT 95%.
pub fn check_choking(
    nutrition: i32,
    canchoke: bool,
    breathless: bool,
    has_hunger_property: bool,
    strangled: bool,
    rng: &mut impl Rng,
) -> ChokeOutcome {
    if !canchoke || nutrition < CHOKING_THRESHOLD {
        return ChokeOutcome::NoChoke;
    }

    // Breathless or Hunger property guarantees survival.
    if breathless || has_hunger_property {
        let lost = if has_hunger_property {
            // With Hunger property: reduce to 60.
            (nutrition - 60).max(0)
        } else {
            // Without: lose 1000 nutrition.
            1000
        };
        return ChokeOutcome::Vomited {
            nutrition_lost: lost,
        };
    }

    // Random survival: 1 in 20 chance (5%), but strangled blocks this.
    if !strangled && rng.random_range(0..CHOKING_SURVIVAL_DENOM) == 0 {
        return ChokeOutcome::Vomited {
            nutrition_lost: 1000,
        };
    }

    ChokeOutcome::Choked
}

// ---------------------------------------------------------------------------
// Conduct tracking
// ---------------------------------------------------------------------------

/// Food conduct classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoodConductClass {
    /// Vegan-safe food.
    Vegan,
    /// Vegetarian but not vegan (eggs, dairy, etc.).
    Vegetarian,
    /// Meat / non-vegetarian.
    Meat,
}

/// Classify a food item for conduct purposes.
///
/// Based on material: Veggy is vegan, Flesh is meat.
/// Certain specific items have overrides (eggs = vegetarian, etc.)
pub fn classify_food_conduct(material: Material, is_corpse: bool, corpse_def: Option<&CorpseDef>) -> FoodConductClass {
    if is_corpse {
        if let Some(cd) = corpse_def {
            if cd.is_vegan {
                return FoodConductClass::Vegan;
            } else if cd.is_vegetarian {
                return FoodConductClass::Vegetarian;
            } else {
                return FoodConductClass::Meat;
            }
        }
        // Default corpse is meat.
        return FoodConductClass::Meat;
    }

    match material {
        Material::Veggy => FoodConductClass::Vegan,
        Material::Flesh => FoodConductClass::Meat,
        Material::Wax => FoodConductClass::Vegetarian,
        Material::Leather | Material::Bone | Material::DragonHide => {
            FoodConductClass::Meat
        }
        _ => FoodConductClass::Vegan,
    }
}

/// Record returned from conduct tracking so the caller can emit events.
#[derive(Debug, Clone, Default)]
pub struct ConductViolations {
    /// Whether the foodless conduct was broken.
    pub broke_foodless: bool,
    /// Whether the vegan conduct was broken.
    pub broke_vegan: bool,
    /// Whether the vegetarian conduct was broken.
    pub broke_vegetarian: bool,
}

/// Update conduct counters based on food classification.
///
/// Returns which conducts were violated.
pub fn update_conducts(
    conduct_class: FoodConductClass,
    food_count: &mut i64,
    unvegan_count: &mut i64,
    unvegetarian_count: &mut i64,
) -> ConductViolations {
    let mut violations = ConductViolations::default();

    // Any eating breaks foodless conduct.
    *food_count += 1;
    violations.broke_foodless = true;

    match conduct_class {
        FoodConductClass::Vegan => {
            // Vegan food: no further conduct violations.
        }
        FoodConductClass::Vegetarian => {
            // Vegetarian (not vegan): breaks vegan conduct.
            *unvegan_count += 1;
            violations.broke_vegan = true;
        }
        FoodConductClass::Meat => {
            // Meat: breaks both vegan and vegetarian.
            *unvegan_count += 1;
            *unvegetarian_count += 1;
            violations.broke_vegan = true;
            violations.broke_vegetarian = true;
        }
    }

    violations
}

// ---------------------------------------------------------------------------
// Corpse eating effects
// ---------------------------------------------------------------------------

/// Result of eating a poisonous corpse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoisonOutcome {
    /// Not poisonous, no effect.
    NotPoisonous,
    /// Poisonous but resisted (has poison resistance).
    Resisted,
    /// Poisoned: took damage and strength loss.
    Poisoned {
        /// HP damage dealt.
        hp_damage: i32,
        /// Strength points lost.
        str_loss: i32,
    },
}

/// Check poisonous corpse effects.
///
/// 80% chance of poison effect activating (4 in 5).
/// With poison resistance: no damage, message only.
/// Without: lose rnd(4) STR and take rnd(15) damage.
pub fn check_poison(
    poisonous: bool,
    has_poison_resistance: bool,
    rng: &mut impl Rng,
) -> PoisonOutcome {
    if !poisonous {
        return PoisonOutcome::NotPoisonous;
    }

    // 80% chance poison activates: rn2(5) != 0 in C.
    if rng.random_range(0..5) == 0 {
        return PoisonOutcome::NotPoisonous;
    }

    if has_poison_resistance {
        return PoisonOutcome::Resisted;
    }

    let str_loss = rng.random_range(1..=4);
    let hp_damage = rng.random_range(1..=15);
    PoisonOutcome::Poisoned {
        hp_damage,
        str_loss,
    }
}

/// Result of eating an acidic corpse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcidOutcome {
    /// Not acidic.
    NotAcidic,
    /// Acidic but resisted.
    Resisted,
    /// Took acid damage.
    Damaged { hp_damage: i32 },
}

/// Check acidic corpse effects.
///
/// With acid resistance: no damage.
/// Without: take rnd(15) damage.
pub fn check_acid(
    acidic: bool,
    has_acid_resistance: bool,
    rng: &mut impl Rng,
) -> AcidOutcome {
    if !acidic {
        return AcidOutcome::NotAcidic;
    }

    if has_acid_resistance {
        return AcidOutcome::Resisted;
    }

    let hp_damage = rng.random_range(1..=15);
    AcidOutcome::Damaged { hp_damage }
}

/// Result of a stoning check from eating a petrifying corpse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoningOutcome {
    /// Not a petrifying corpse.
    NotPetrifying,
    /// Has stone resistance, no effect.
    Resisted,
    /// Petrified!
    Petrified,
}

/// Check for petrification from eating cockatrice/chickatrice corpse.
pub fn check_stoning(
    flesh_petrifies: bool,
    has_stone_resistance: bool,
) -> StoningOutcome {
    if !flesh_petrifies {
        return StoningOutcome::NotPetrifying;
    }

    if has_stone_resistance {
        return StoningOutcome::Resisted;
    }

    StoningOutcome::Petrified
}

/// Result of the cannibalism check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CannibalismPenalty {
    /// Permanent aggravate monster.
    pub aggravate: bool,
    /// Luck penalty (negative value, e.g. -2 to -5).
    pub luck_penalty: i8,
}

/// Check for cannibalism effects.
///
/// Same-race eating: permanent aggravate + luck penalty of -rn1(4,2) = -2..-5.
/// Exempt if cannibal_allowed (Orc/Cave Dweller).
pub fn check_cannibalism(
    is_same_race: bool,
    cannibal_allowed: bool,
    rng: &mut impl Rng,
) -> Option<CannibalismPenalty> {
    if !is_same_race || cannibal_allowed {
        return None;
    }

    // rn1(4,2) = rn2(4) + 2 = 2..5
    let luck_loss = rng.random_range(0..4) + 2;
    Some(CannibalismPenalty {
        aggravate: true,
        luck_penalty: -(luck_loss as i8),
    })
}

/// Intrinsic that can be gained from eating a corpse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpseIntrinsic {
    FireResistance,
    ColdResistance,
    SleepResistance,
    ShockResistance,
    PoisonResistance,
    DisintegrationResistance,
    /// Temporary acid resistance (d(3,6) turns).
    AcidResistance { duration: u32 },
    /// Temporary stone resistance (d(3,6) turns).
    StoneResistance { duration: u32 },
    Telepathy,
    Teleportitis,
    TeleportControl,
    /// Strength gain from a giant corpse.
    Strength,
    /// Invisibility (from stalker corpse).
    Invisibility,
    /// See invisible (from stalker/yellow light corpse).
    SeeInvisible,
}

/// Negative effect from eating a corpse (not a permanent intrinsic gain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpseNegativeEffect {
    /// Eating cockatrice = death by stoning.
    Stoning,
    /// Eating werecreature = lycanthropy.
    Lycanthropy,
    /// Eating poisonous corpse without resistance.
    Poisoned { hp_damage: i32, str_loss: i32 },
    /// Eating yellow mold = hallucination.
    Hallucination { duration: i32 },
    /// Eating bat = stun.
    Stun { duration: i32 },
    /// Eating yellow light = blindness.
    Blind { duration: i32 },
    /// Eating domestic animal = aggravation + alignment penalty.
    Aggravation,
}

/// Combined result of eating a corpse: possible intrinsic gain and/or negative effect.
#[derive(Debug, Clone)]
pub struct CorpseEatEffects {
    /// Intrinsic gained (if any).
    pub intrinsic: Option<CorpseIntrinsic>,
    /// Negative effect (if any).
    pub negative: Option<CorpseNegativeEffect>,
    /// Post-eating special effect (wraith level gain, nurse heal, etc.).
    pub post_effect: CorpsePostEffect,
    /// Speed toggle (quantum mechanic).
    pub speed_toggle: bool,
    /// Strength gain amount (from giant corpses).
    pub strength_gain: i32,
}

/// Look up what intrinsic a specific monster corpse may confer by name.
///
/// Returns `(intrinsic, chance_percentage)`.  This is a convenience
/// function for name-based lookup; the main intrinsic system uses
/// `CorpseDef.conveys` flags via `check_intrinsic_gain`.
pub fn corpse_intrinsic_by_name(monster_name: &str) -> Option<(CorpseIntrinsic, u32)> {
    match monster_name {
        // Fire resistance
        "red dragon" | "baby red dragon" | "red naga" | "fire ant" | "fire elemental"
        | "fire vortex" | "salamander" | "hell hound pup" | "hell hound"
            => Some((CorpseIntrinsic::FireResistance, 33)),
        // Cold resistance
        "white dragon" | "baby white dragon" | "frost giant" | "blue jelly"
        | "ice troll" | "ice vortex" | "winter wolf cub" | "winter wolf"
            => Some((CorpseIntrinsic::ColdResistance, 33)),
        // Sleep resistance
        "orange dragon" | "baby orange dragon" | "elf" | "wood elf" | "Green-elf"
            => Some((CorpseIntrinsic::SleepResistance, 33)),
        // Shock resistance
        "blue dragon" | "baby blue dragon" | "electric eel" | "storm giant"
        | "lightning vortex"
            => Some((CorpseIntrinsic::ShockResistance, 33)),
        // Poison resistance
        "yellow dragon" | "baby yellow dragon" | "killer bee" | "scorpion"
        | "pit viper" | "cobra" | "water moccasin" | "garter snake"
            => Some((CorpseIntrinsic::PoisonResistance, 33)),
        // Telepathy
        "floating eye" => Some((CorpseIntrinsic::Telepathy, 100)),
        // Teleportitis
        "leprechaun" | "nymph" => Some((CorpseIntrinsic::Teleportitis, 33)),
        // Teleport control
        "tengu" => Some((CorpseIntrinsic::TeleportControl, 33)),
        // See invisible
        "stalker" | "yellow light" => Some((CorpseIntrinsic::SeeInvisible, 33)),
        // Strength gain (giants)
        "giant" | "stone giant" | "hill giant" | "fire giant" | "frost giant"
        | "storm giant" | "titan" | "minotaur"
            => Some((CorpseIntrinsic::Strength, 50)),
        _ => None,
    }
}

/// Process eating a corpse: determine intrinsic gain and negative effects.
///
/// This combines `check_intrinsic_gain` with negative effect determination
/// based on monster name/properties.
pub fn eat_corpse_effects(
    corpse: &CorpseDef,
    has_poison_res: bool,
    has_stone_res: bool,
    rng: &mut impl Rng,
) -> CorpseEatEffects {
    let mut effects = CorpseEatEffects {
        intrinsic: None,
        negative: None,
        post_effect: CorpsePostEffect::None,
        speed_toggle: false,
        strength_gain: 0,
    };

    // Stoning check (cockatrice family).
    if corpse.flesh_petrifies && !has_stone_res {
        effects.negative = Some(CorpseNegativeEffect::Stoning);
        return effects; // Fatal -- no other effects matter.
    }

    // Poisonous corpse.
    if corpse.poisonous {
        let poison = check_poison(true, has_poison_res, rng);
        if let PoisonOutcome::Poisoned { hp_damage, str_loss } = poison {
            effects.negative = Some(CorpseNegativeEffect::Poisoned {
                hp_damage,
                str_loss,
            });
        }
    }

    // Domestic animal penalty (dog, cat).
    if corpse.is_domestic {
        effects.negative = Some(CorpseNegativeEffect::Aggravation);
    }

    // Intrinsic gain.
    effects.intrinsic = check_intrinsic_gain(corpse, rng);

    // Giant strength gain.
    if corpse.is_giant {
        if let Some(CorpseIntrinsic::Strength) = effects.intrinsic {
            effects.strength_gain = 1;
        }
    }

    // Name-based post effects.
    effects.post_effect = corpse_post_effect_by_name(&corpse.name);

    // Quantum mechanic speed toggle.
    if corpse.name == "quantum mechanic" {
        effects.speed_toggle = true;
    }

    effects
}

/// Determine the post-eating special effect for a corpse by monster name.
fn corpse_post_effect_by_name(name: &str) -> CorpsePostEffect {
    match name {
        "wraith" => CorpsePostEffect::GainLevel,
        "nurse" => CorpsePostEffect::FullHeal,
        "stalker" => CorpsePostEffect::Invisibility,
        "yellow light" => CorpsePostEffect::Stun { duration: 30 },
        "giant bat" | "bat" => CorpsePostEffect::Stun { duration: 30 },
        "chameleon" | "doppelganger" | "genetic engineer"
            => CorpsePostEffect::PolymorphSelf,
        "displacer beast" => CorpsePostEffect::Displacement { duration: 30 },
        "disenchanter" => CorpsePostEffect::StripIntrinsic,
        "mind flayer" | "master mind flayer" => CorpsePostEffect::GainIntelligence,
        "werewolf" | "werejackal" | "wererat" => CorpsePostEffect::Lycanthropy,
        "lizard" => CorpsePostEffect::LizardCure,
        "quantum mechanic" => CorpsePostEffect::ToggleSpeed,
        _ => CorpsePostEffect::None,
    }
}

/// Generate a random tin variety for a new tin.
pub fn random_tin_variety(rng: &mut impl Rng) -> TinVariety {
    match rng.random_range(0..15u32) {
        0 => TinVariety::Rotten,
        1 => TinVariety::Homemade,
        2 => TinVariety::Soup,
        3 => TinVariety::FrenchFried,
        4 => TinVariety::Pickled,
        5 => TinVariety::Boiled,
        6 => TinVariety::Smoked,
        7 => TinVariety::Dried,
        8 => TinVariety::DeepFried,
        9 => TinVariety::Szechuan,
        10 => TinVariety::Broiled,
        11 => TinVariety::StirFried,
        12 => TinVariety::Sauteed,
        13 => TinVariety::Candied,
        14 => TinVariety::Pureed,
        _ => TinVariety::Homemade,
    }
}

/// Description text for a tin variety.
pub fn tin_description(variety: TinVariety, monster_name: &str) -> String {
    match variety {
        TinVariety::Rotten => "It smells terrible!".to_string(),
        TinVariety::Homemade => format!("It smells like {} soup.", monster_name),
        TinVariety::Soup => "It is hot soup.".to_string(),
        TinVariety::FrenchFried => format!("It is French fried {} meat.", monster_name),
        TinVariety::Pickled => format!("It is pickled {} meat.", monster_name),
        TinVariety::Boiled => format!("It is boiled {} meat.", monster_name),
        TinVariety::Smoked => format!("It is smoked {} meat.", monster_name),
        TinVariety::Dried => format!("It is dried {} meat.", monster_name),
        TinVariety::DeepFried => format!("It is deep fried {} meat.", monster_name),
        TinVariety::Szechuan => format!("It is Szechuan {} meat.", monster_name),
        TinVariety::Broiled => format!("It is broiled {} meat.", monster_name),
        TinVariety::StirFried => format!("It is stir fried {} meat.", monster_name),
        TinVariety::Sauteed => format!("It is sauteed {} meat.", monster_name),
        TinVariety::Candied => format!("It is candied {} meat.", monster_name),
        TinVariety::Pureed => format!("It is pureed {} meat.", monster_name),
        TinVariety::Spinach => "This makes you feel like Strongo!".to_string(),
    }
}

/// Check for intrinsic gain from eating a corpse.
///
/// Uses the `should_givit()` probability from NetHack:
/// - Telepathy: chance = 1 (always succeeds if level > 0)
/// - Poison resistance (killer bee/scorpion): 1/4 chance of chance=1
/// - Teleport: chance = 10
/// - Teleport control: chance = 12
/// - Default: chance = 15
///
/// Succeed if `monster_level > rn2(chance)`.
///
/// For temporary resistances (STONE, ACID), even if the main check fails,
/// there is a fallback with lower threshold.
pub fn check_intrinsic_gain(
    corpse: &CorpseDef,
    rng: &mut impl Rng,
) -> Option<CorpseIntrinsic> {
    let conveys = corpse.conveys;
    let level = corpse.base_level as i32;

    // Build list of possible intrinsics.
    let mut candidates: Vec<CorpseIntrinsic> = Vec::new();

    if conveys.contains(ResistanceSet::FIRE) {
        candidates.push(CorpseIntrinsic::FireResistance);
    }
    if conveys.contains(ResistanceSet::COLD) {
        candidates.push(CorpseIntrinsic::ColdResistance);
    }
    if conveys.contains(ResistanceSet::SLEEP) {
        candidates.push(CorpseIntrinsic::SleepResistance);
    }
    if conveys.contains(ResistanceSet::SHOCK) {
        candidates.push(CorpseIntrinsic::ShockResistance);
    }
    if conveys.contains(ResistanceSet::POISON) {
        candidates.push(CorpseIntrinsic::PoisonResistance);
    }
    if conveys.contains(ResistanceSet::DISINTEGRATE) {
        candidates.push(CorpseIntrinsic::DisintegrationResistance);
    }
    if conveys.contains(ResistanceSet::ACID) {
        candidates.push(CorpseIntrinsic::AcidResistance { duration: 0 });
    }
    if conveys.contains(ResistanceSet::STONE) {
        candidates.push(CorpseIntrinsic::StoneResistance { duration: 0 });
    }
    if corpse.conveys_telepathy {
        candidates.push(CorpseIntrinsic::Telepathy);
    }
    if corpse.conveys_teleport {
        candidates.push(CorpseIntrinsic::Teleportitis);
    }
    if corpse.is_giant {
        candidates.push(CorpseIntrinsic::Strength);
    }

    if candidates.is_empty() {
        return None;
    }

    // Select one uniformly at random (reservoir sampling).
    let idx = rng.random_range(0..candidates.len());
    let selected = candidates[idx];

    // If strength is the only candidate, 50% chance of nothing.
    if candidates.len() == 1 && selected == CorpseIntrinsic::Strength
        && rng.random_range(0..2) == 0 {
            return None;
        }

    // Check should_givit probability.
    match selected {
        CorpseIntrinsic::Telepathy => {
            // chance = 1, always succeeds if level > 0
            if level > rng.random_range(0..1) {
                Some(CorpseIntrinsic::Telepathy)
            } else {
                None
            }
        }
        CorpseIntrinsic::PoisonResistance => {
            // Special: killer bee/scorpion get 1/4 chance of chance=1.
            // We approximate: the caller can set this; for now use chance=15.
            let chance = 15;
            if level > rng.random_range(0..chance) {
                Some(CorpseIntrinsic::PoisonResistance)
            } else {
                None
            }
        }
        CorpseIntrinsic::Teleportitis => {
            let chance = 10;
            if level > rng.random_range(0..chance) {
                Some(CorpseIntrinsic::Teleportitis)
            } else {
                None
            }
        }
        CorpseIntrinsic::TeleportControl => {
            let chance = 12;
            if level > rng.random_range(0..chance) {
                Some(CorpseIntrinsic::TeleportControl)
            } else {
                None
            }
        }
        CorpseIntrinsic::AcidResistance { .. } => {
            let chance = 15;
            if level > rng.random_range(0..chance) {
                // Permanent (full conveyance — but spec says acid is always temp).
                let duration = roll_d(3, 6, rng);
                Some(CorpseIntrinsic::AcidResistance { duration })
            } else {
                // Fallback: temp_givit with chance=3.
                if level > rng.random_range(0..3_i32) {
                    let duration = roll_d(3, 6, rng);
                    Some(CorpseIntrinsic::AcidResistance { duration })
                } else {
                    None
                }
            }
        }
        CorpseIntrinsic::StoneResistance { .. } => {
            let chance = 15;
            if level > rng.random_range(0..chance) {
                let duration = roll_d(3, 6, rng);
                Some(CorpseIntrinsic::StoneResistance { duration })
            } else {
                // Fallback: temp_givit with chance=6.
                if level > rng.random_range(0..6_i32) {
                    let duration = roll_d(3, 6, rng);
                    Some(CorpseIntrinsic::StoneResistance { duration })
                } else {
                    None
                }
            }
        }
        CorpseIntrinsic::Strength => {
            // Giants always succeed the givit check (they are high level).
            Some(CorpseIntrinsic::Strength)
        }
        _ => {
            // Default chance = 15.
            let chance = 15;
            if level > rng.random_range(0..chance) {
                Some(selected)
            } else {
                None
            }
        }
    }
}

/// Roll d(n, s) — n dice of s sides, total.
fn roll_d(n: u32, s: u32, rng: &mut impl Rng) -> u32 {
    (0..n).map(|_| rng.random_range(1..=s)).sum()
}

// ---------------------------------------------------------------------------
// Eating: main entry point
// ---------------------------------------------------------------------------

/// Result of eating food.
#[derive(Debug, Clone)]
pub struct EatResult {
    /// Events generated during eating.
    pub events: Vec<EngineEvent>,
    /// Number of turns the eating takes.
    pub eating_time: i32,
    /// Whether the food item should be consumed (removed from world).
    pub consumed: bool,
    /// Whether the eater died.
    pub died: bool,
    /// Conduct violations.
    pub conduct: ConductViolations,
}

/// Eat a food item (non-corpse, non-tin).
///
/// Adds nutrition to the eater's hunger counter, checks for choking,
/// tracks conduct, and consumes the food.
pub fn eat_food(
    world: &mut GameWorld,
    eater: Entity,
    food_entity: Entity,
    food_def: &FoodDef,
    rng: &mut impl Rng,
) -> EatResult {
    let mut events = Vec::new();
    let mut died = false;

    let old_nutrition = world
        .get_component::<Nutrition>(eater)
        .map(|n| n.0)
        .unwrap_or(INITIAL_NUTRITION);

    let old_level = nutrition_to_hunger_level(old_nutrition);
    let was_satiated = old_level == HungerLevel::Satiated;

    // Calculate eating time.
    let eating_time = if food_def.is_glob {
        eating_time_glob(food_def.weight)
    } else {
        eating_time_normal(food_def.oc_delay)
    };

    // Add nutrition.
    let new_nutrition = old_nutrition + food_def.nutrition;

    // Check for choking (simplified: no breathless/hunger/strangled tracking
    // in current ECS, so default to normal hero).
    let choke = check_choking(
        new_nutrition,
        was_satiated,
        false, // breathless
        false, // has_hunger_property
        false, // strangled
        rng,
    );

    let final_nutrition = match &choke {
        ChokeOutcome::Vomited { nutrition_lost } => {
            events.push(EngineEvent::msg("potion-sickness"));
            new_nutrition - nutrition_lost
        }
        ChokeOutcome::Choked => {
            events.push(EngineEvent::msg("eat-choke"));
            events.push(EngineEvent::EntityDied {
                entity: eater,
                killer: None,
                cause: DeathCause::KilledBy {
                    killer_name: "choking on food".to_string(),
                },
            });
            died = true;
            new_nutrition
        }
        ChokeOutcome::NoChoke => new_nutrition,
    };

    // Update nutrition component.
    if let Some(mut n) = world.get_component_mut::<Nutrition>(eater) {
        n.0 = final_nutrition;
    }

    // Emit hunger change if level changed.
    let new_level = nutrition_to_hunger_level(final_nutrition);
    if old_level != new_level && !died {
        events.push(EngineEvent::HungerChange {
            entity: eater,
            old: old_level,
            new_level,
        });
    }

    // Track conduct.
    let conduct_class = classify_food_conduct(food_def.material, false, None);
    let conduct = ConductViolations {
        broke_foodless: true,
        broke_vegan: conduct_class != FoodConductClass::Vegan,
        broke_vegetarian: conduct_class == FoodConductClass::Meat,
    };

    // Consume the food item.
    let consumed = true; // Food is consumed even on death.
    let _ = world.despawn(food_entity);

    EatResult {
        events,
        eating_time,
        consumed,
        died,
        conduct,
    }
}

/// Eat a corpse, applying all corpse-specific effects.
///
/// Returns events from the eating. The caller should also call `eat_food`
/// for the nutrition component, or this function handles nutrition directly.
pub fn eat_corpse(
    world: &mut GameWorld,
    eater: Entity,
    corpse_entity: Entity,
    corpse: &CorpseDef,
    rng: &mut impl Rng,
) -> EatResult {
    let mut events = Vec::new();
    let mut died = false;

    let old_nutrition = world
        .get_component::<Nutrition>(eater)
        .map(|n| n.0)
        .unwrap_or(INITIAL_NUTRITION);

    let old_level = nutrition_to_hunger_level(old_nutrition);
    let was_satiated = old_level == HungerLevel::Satiated;

    // Eating time for corpses.
    let eating_time = eating_time_corpse(corpse.corpse_weight as u32);

    // --- Pre-eating effects (cprefx) ---

    // Cannibalism check.
    if let Some(penalty) = check_cannibalism(
        corpse.is_same_race,
        corpse.cannibal_allowed,
        rng,
    ) {
        events.push(EngineEvent::msg("eat-cannibal"));
        if penalty.aggravate {
            events.push(EngineEvent::StatusApplied {
                entity: eater,
                status: StatusEffect::Aggravate,
                duration: None, // permanent
                source: None,
            });
        }
        // Luck penalty is tracked but we don't have a luck component
        // in the minimal ECS yet. Record it as an event/message.
        events.push(EngineEvent::msg("eat-dread"));
    }

    // Petrification check.
    let stoning = check_stoning(corpse.flesh_petrifies, false);
    if stoning == StoningOutcome::Petrified {
        events.push(EngineEvent::msg("eat-petrify"));
        events.push(EngineEvent::EntityDied {
            entity: eater,
            killer: None,
            cause: DeathCause::Petrification,
        });
        died = true;
    }

    // Domestic animal penalty.
    if corpse.is_domestic && !corpse.cannibal_allowed && !died {
        events.push(EngineEvent::msg("eat-corpse-effect"));
        events.push(EngineEvent::StatusApplied {
            entity: eater,
            status: StatusEffect::Aggravate,
            duration: None,
            source: None,
        });
    }

    // --- Nutrition ---
    if !died {
        let nutrition_gain = corpse.corpse_nutrition as i32;
        let new_nutrition = old_nutrition + nutrition_gain;

        // Choking check.
        let choke = check_choking(
            new_nutrition,
            was_satiated,
            false,
            false,
            false,
            rng,
        );

        let final_nutrition = match &choke {
            ChokeOutcome::Vomited { nutrition_lost } => {
                events.push(EngineEvent::msg("potion-sickness"));
                new_nutrition - nutrition_lost
            }
            ChokeOutcome::Choked => {
                events.push(EngineEvent::EntityDied {
                    entity: eater,
                    killer: None,
                    cause: DeathCause::KilledBy {
                        killer_name: "choking on food".to_string(),
                    },
                });
                died = true;
                new_nutrition
            }
            ChokeOutcome::NoChoke => new_nutrition,
        };

        if let Some(mut n) = world.get_component_mut::<Nutrition>(eater) {
            n.0 = final_nutrition;
        }

        let new_level = nutrition_to_hunger_level(final_nutrition);
        if old_level != new_level && !died {
            events.push(EngineEvent::HungerChange {
                entity: eater,
                old: old_level,
                new_level,
            });
        }
    }

    // --- Post-eating effects ---
    if !died {
        // Poisonous corpse.
        let poison = check_poison(corpse.poisonous, false, rng);
        match poison {
            PoisonOutcome::Poisoned {
                hp_damage,
                str_loss,
            } => {
                events.push(EngineEvent::msg("eat-poisoned"));
                // Apply HP damage.
                if let Some(mut hp) = world.get_component_mut::<HitPoints>(eater) {
                    hp.current -= hp_damage;
                    events.push(EngineEvent::HpChange {
                        entity: eater,
                        amount: -hp_damage,
                        new_hp: hp.current,
                        source: HpSource::Poison,
                    });
                    if hp.current <= 0 {
                        events.push(EngineEvent::EntityDied {
                            entity: eater,
                            killer: None,
                            cause: DeathCause::Poisoning,
                        });
                        died = true;
                    }
                }
                // Strength loss tracked as message (simplified).
                if str_loss > 0 {
                    events.push(EngineEvent::msg("eat-weakened"));
                }
            }
            PoisonOutcome::Resisted => {
                events.push(EngineEvent::msg("eat-poison-resist"));
            }
            PoisonOutcome::NotPoisonous => {}
        }

        // Acidic corpse.
        if !died {
            let acid = check_acid(corpse.acidic, false, rng);
            match acid {
                AcidOutcome::Damaged { hp_damage } => {
                    events.push(EngineEvent::msg("eat-acidic"));
                    if let Some(mut hp) = world.get_component_mut::<HitPoints>(eater) {
                        hp.current -= hp_damage;
                        events.push(EngineEvent::HpChange {
                            entity: eater,
                            amount: -hp_damage,
                            new_hp: hp.current,
                            source: HpSource::Combat,
                        });
                    }
                }
                AcidOutcome::Resisted => {
                    events.push(EngineEvent::msg("eat-acidic"));
                }
                AcidOutcome::NotAcidic => {}
            }
        }

        // Intrinsic gain.
        if !died
            && let Some(intrinsic) = check_intrinsic_gain(corpse, rng) {
                let (status, msg, duration) = match intrinsic {
                    CorpseIntrinsic::FireResistance => (
                        StatusEffect::FireResistance,
                        "You feel a momentary chill.",
                        None,
                    ),
                    CorpseIntrinsic::ColdResistance => (
                        StatusEffect::ColdResistance,
                        "You feel full of hot air.",
                        None,
                    ),
                    CorpseIntrinsic::SleepResistance => (
                        StatusEffect::SleepResistance,
                        "You feel wide awake.",
                        None,
                    ),
                    CorpseIntrinsic::ShockResistance => (
                        StatusEffect::ShockResistance,
                        "Your health currently feels amplified!",
                        None,
                    ),
                    CorpseIntrinsic::PoisonResistance => (
                        StatusEffect::PoisonResistance,
                        "You feel healthy.",
                        None,
                    ),
                    CorpseIntrinsic::DisintegrationResistance => (
                        StatusEffect::DisintegrationResistance,
                        "You feel very firm.",
                        None,
                    ),
                    CorpseIntrinsic::AcidResistance { duration: d } => (
                        StatusEffect::FireResistance, // placeholder
                        "You feel a burning sensation fade.",
                        Some(d),
                    ),
                    CorpseIntrinsic::StoneResistance { duration: d } => (
                        StatusEffect::Protected, // placeholder
                        "You feel limber.",
                        Some(d),
                    ),
                    CorpseIntrinsic::Telepathy => (
                        StatusEffect::Telepathy,
                        "You feel a strange mental acuity.",
                        None,
                    ),
                    CorpseIntrinsic::Teleportitis => (
                        StatusEffect::FastSpeed, // placeholder
                        "You feel very jumpy.",
                        None,
                    ),
                    CorpseIntrinsic::TeleportControl => (
                        StatusEffect::FastSpeed, // placeholder
                        "You feel in control of yourself.",
                        None,
                    ),
                    CorpseIntrinsic::Strength => (
                        StatusEffect::Protected, // placeholder
                        "You feel strong!",
                        None,
                    ),
                    CorpseIntrinsic::Invisibility => (
                        StatusEffect::Invisible,
                        "You feel rather airy.",
                        None,
                    ),
                    CorpseIntrinsic::SeeInvisible => (
                        StatusEffect::SeeInvisible,
                        "You can see through yourself.",
                        None,
                    ),
                };

                events.push(EngineEvent::msg(msg));
                events.push(EngineEvent::StatusApplied {
                    entity: eater,
                    status,
                    duration,
                    source: None,
                });
            }
    }

    // Conduct.
    let conduct_class = if corpse.is_vegan {
        FoodConductClass::Vegan
    } else if corpse.is_vegetarian {
        FoodConductClass::Vegetarian
    } else {
        FoodConductClass::Meat
    };

    let conduct = ConductViolations {
        broke_foodless: true,
        broke_vegan: conduct_class != FoodConductClass::Vegan,
        broke_vegetarian: conduct_class == FoodConductClass::Meat,
    };

    // Consume the corpse.
    let _ = world.despawn(corpse_entity);

    EatResult {
        events,
        eating_time,
        consumed: true,
        died,
        conduct,
    }
}

// ---------------------------------------------------------------------------
// Tin eating
// ---------------------------------------------------------------------------

/// Result of opening/eating a tin.
#[derive(Debug, Clone)]
pub struct TinEatResult {
    /// Events generated.
    pub events: Vec<EngineEvent>,
    /// Turns spent opening the tin.
    pub opening_time: i32,
    /// Nutrition gained.
    pub nutrition: i32,
    /// Whether the tin was consumed.
    pub consumed: bool,
}

/// Calculate tin opening time.
///
/// - With tin opener: rn2(2) = 0 or 1 (blessed opener: 0 = instant).
/// - With dagger/knife: 3 turns.
/// - With pick-axe/axe: 6 turns.
/// - Bare hands: rn1(1 + 500/(DEX+STR), 10) turns.
/// - Blessed tin: rn2(2).
/// - M2_STRONG (very strong): 2 turns.
#[allow(clippy::too_many_arguments)]
pub fn tin_opening_time(
    has_tin_opener: bool,
    opener_blessed: bool,
    has_dagger: bool,
    has_pick: bool,
    is_strong: bool,
    dex: u8,
    str_: u8,
    tin_blessed: bool,
    rng: &mut impl Rng,
) -> i32 {
    // Blessed tin or blessed tin opener: instant or 1 turn.
    if tin_blessed || (has_tin_opener && opener_blessed) {
        return rng.random_range(0..2);
    }

    if has_tin_opener {
        return rng.random_range(0..2);
    }

    if is_strong {
        return 2;
    }

    if has_dagger {
        return 3;
    }

    if has_pick {
        return 6;
    }

    // Bare hands: rn1(1 + 500/(DEX+STR), 10)
    let stat_sum = (dex as i32 + str_ as i32).max(1);
    let range = 1 + 500 / stat_sum;
    let time = rng.random_range(0..range) + 10;
    time.min(50) // cap at 50
}

/// Calculate tin nutrition based on variety and monster nutrition.
pub fn tin_nutrition(
    variety: TinVariety,
    monster_nutrition: u16,
    blessed: bool,
    cursed: bool,
    rng: &mut impl Rng,
) -> i32 {
    match variety {
        TinVariety::Spinach => {
            if blessed {
                600
            } else if cursed {
                200 + rng.random_range(1..=400)
            } else {
                400 + rng.random_range(1..=200)
            }
        }
        TinVariety::Rotten => -50,
        TinVariety::Homemade => {
            // Capped at monster's cnutrit.
            (50_i32).min(monster_nutrition as i32)
        }
        _ => {
            let idx = variety as usize;
            if idx < TIN_NUTRITION.len() {
                TIN_NUTRITION[idx]
            } else {
                0
            }
        }
    }
}

/// Eat a tin, handling opening time, nutrition, and effects.
#[allow(clippy::too_many_arguments)]
pub fn eat_tin(
    world: &mut GameWorld,
    eater: Entity,
    tin_entity: Entity,
    variety: TinVariety,
    monster_nutrition: u16,
    blessed: bool,
    cursed: bool,
    opening_time: i32,
    rng: &mut impl Rng,
) -> TinEatResult {
    let mut events = Vec::new();

    let nutrition = tin_nutrition(variety, monster_nutrition, blessed, cursed, rng);

    // Rotten tin causes vomiting, no nutrition gained.
    if variety == TinVariety::Rotten {
        events.push(EngineEvent::msg("eat-rotten"));
    } else if variety == TinVariety::Spinach {
        events.push(EngineEvent::msg("eat-gain-strength"));
        // Strength gain handled by caller.
    }

    // Apply nutrition (only if positive).
    if nutrition > 0
        && let Some(mut n) = world.get_component_mut::<Nutrition>(eater) {
            n.0 += nutrition;
        }

    // Greasy tins.
    if tin_is_greasy(variety) {
        events.push(EngineEvent::msg("eat-greasy"));
    }

    // Consume the tin.
    let _ = world.despawn(tin_entity);

    TinEatResult {
        events,
        opening_time,
        nutrition,
        consumed: true,
    }
}

// ---------------------------------------------------------------------------
// Per-turn hunger depletion (gethungry)
// ---------------------------------------------------------------------------

/// Accessory hunger context — all the state needed to compute per-turn
/// hunger depletion beyond the base 1-point cost.
#[derive(Debug, Clone, Default)]
pub struct AccessoryHungerCtx {
    /// Hero is asleep/fainted ("Unaware").
    pub unaware: bool,
    /// Hero has Slow_digestion property (from a ring).
    pub slow_digestion: bool,
    /// Hero has Slow_digestion from armor (not ring).
    pub slow_digestion_from_armor: bool,
    /// Hero can eat (carnivorous OR herbivorous OR metallivorous).
    pub can_eat: bool,
    /// Hero has regeneration from non-artifact, non-polyform source.
    pub has_regeneration: bool,
    /// Encumbrance is Stressed or worse (> SLT_ENCUMBER).
    pub stressed_or_worse: bool,
    /// Hero has the Hunger property active.
    pub has_hunger_property: bool,
    /// Hero has Conflict from a non-artifact source.
    pub has_conflict: bool,
    /// Hero is wearing a left ring (non-meat, hunger-causing).
    pub left_ring_causes_hunger: bool,
    /// Hero is wearing a right ring (non-meat, hunger-causing).
    pub right_ring_causes_hunger: bool,
    /// Hero is wearing an amulet (not fake Amulet of Yendor).
    pub amulet_causes_hunger: bool,
    /// Hero is carrying the real Amulet of Yendor.
    pub carrying_amulet: bool,
    /// Hero is invulnerable (praying).
    pub invulnerable: bool,
}

/// Compute per-turn hunger depletion.
///
/// Returns the number of nutrition points to subtract this turn.
/// Implements the full `gethungry()` logic from the spec (Section 3).
///
/// `accessorytime` is `rn2(20)` — the caller provides the random value
/// so that tests can be deterministic.
pub fn compute_hunger_depletion(ctx: &AccessoryHungerCtx, accessorytime: u32) -> i32 {
    if ctx.invulnerable {
        return 0;
    }

    let mut depletion = 0;

    // Base depletion: 1 point per turn (if can eat and no Slow_digestion).
    // When Unaware (asleep/fainted), only 10% chance.
    let base_applies = if ctx.unaware {
        // Would need rng — but we make the caller pass this info.
        // For simplicity, we'll handle this at the call site.
        // Here we assume the caller already checked and set can_eat=false
        // for the 90% of turns when Unaware blocks base depletion.
        ctx.can_eat && !ctx.slow_digestion
    } else {
        ctx.can_eat && !ctx.slow_digestion
    };

    if base_applies {
        depletion += 1;
    }

    // Accessory hunger (Section 3 of spec).
    let odd = accessorytime % 2 == 1;

    if odd {
        // Odd accessorytime: regeneration and encumbrance.
        if ctx.has_regeneration {
            depletion += 1;
        }
        if ctx.stressed_or_worse {
            depletion += 1;
        }
    } else {
        // Even accessorytime values.
        // General even checks (0, 2, 6, 10, 14, 18):
        if matches!(accessorytime, 0 | 2 | 6 | 10 | 14 | 18) {
            if ctx.has_hunger_property {
                depletion += 1;
            }
            if ctx.has_conflict {
                depletion += 1;
            }
        }

        // Specific even values:
        match accessorytime {
            0 if ctx.slow_digestion_from_armor => {
                depletion += 1;
            }
            4 if ctx.left_ring_causes_hunger => {
                depletion += 1;
            }
            8 if ctx.amulet_causes_hunger => {
                depletion += 1;
            }
            12 if ctx.right_ring_causes_hunger => {
                depletion += 1;
            }
            16 if ctx.carrying_amulet => {
                depletion += 1;
            }
            _ => {}
        }
    }

    depletion
}

// ---------------------------------------------------------------------------
// Nutrition per bite (nmod calculation)
// ---------------------------------------------------------------------------

/// Nutrition-per-bite calculation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NutritionPerBite {
    /// If negative: give |nmod| nutrition per turn.
    /// If positive: give 1 nutrition every `nmod` turns (when
    /// `used_time % nmod != 0`).
    /// If zero: no nutrition this bite.
    pub nmod: i32,
}

/// Calculate the nutrition-per-bite value (`nmod`) for multi-turn eating.
///
/// `reqtime`: total eating turns.
/// `oeaten`: remaining nutrition in the food.
///
/// From spec Section 5:
/// - If reqtime == 0 or oeaten == 0: nmod = 0
/// - If oeaten >= reqtime: nmod = -(oeaten / reqtime)  (negative = give |nmod| per turn)
/// - Else: nmod = reqtime % oeaten  (positive = give 1 nutrition every nmod turns)
pub fn calc_nmod(reqtime: i32, oeaten: i32) -> NutritionPerBite {
    if reqtime == 0 || oeaten == 0 {
        return NutritionPerBite { nmod: 0 };
    }

    if oeaten >= reqtime {
        NutritionPerBite {
            nmod: -(oeaten / reqtime),
        }
    } else {
        NutritionPerBite {
            nmod: reqtime % oeaten,
        }
    }
}

/// Adjust eating time for partially-eaten food.
///
/// `reqtime = rounddiv(reqtime * oeaten, basenutrit)`
pub fn adjust_eating_time_partial(reqtime: i32, oeaten: i32, base_nutrit: i32) -> i32 {
    if base_nutrit == 0 {
        return reqtime;
    }
    rounddiv(reqtime * oeaten, base_nutrit)
}

/// Integer division with rounding (same as NetHack's rounddiv).
fn rounddiv(x: i32, y: i32) -> i32 {
    if y == 0 {
        return x;
    }
    let t = x / y;
    if 2 * (x % y) >= y {
        t + 1
    } else {
        t
    }
}

// ---------------------------------------------------------------------------
// Racial nutrition modifiers (adj_victual_nutrition)
// ---------------------------------------------------------------------------

/// Hero race for racial nutrition modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroRace {
    Human,
    Elf,
    Dwarf,
    Gnome,
    Orc,
}

/// Apply racial nutrition modifier to a per-bite amount.
///
/// From spec Section 4:
/// - Lembas wafer + Elf: +25% (nutrition * 5/4)
/// - Lembas wafer + Orc: -25% (nutrition * 3/4)
/// - Cram ration + Dwarf: +17% (nutrition * 7/6)
///
/// Returns the adjusted per-bite nutrition.
pub fn adj_victual_nutrition(base_per_bite: i32, food_name: &str, race: HeroRace) -> i32 {
    match (food_name, race) {
        ("lembas wafer", HeroRace::Elf) => base_per_bite * 5 / 4,
        ("lembas wafer", HeroRace::Orc) => base_per_bite * 3 / 4,
        ("cram ration", HeroRace::Dwarf) => base_per_bite * 7 / 6,
        _ => base_per_bite,
    }
}

// ---------------------------------------------------------------------------
// Tainted / rotten corpse calculation
// ---------------------------------------------------------------------------

/// Result of checking whether a corpse is tainted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaintedOutcome {
    /// Corpse is fresh (not rotten).
    Fresh,
    /// Corpse is tainted (rotted > 5): food poisoning.
    Tainted {
        /// Turns until death from food poisoning (10-19).
        turns_to_die: i32,
    },
    /// Corpse is mildly rotten (rotted > 3): take some damage.
    MildlyIll {
        /// HP damage dealt (rnd(8)).
        hp_damage: i32,
    },
}

/// Check whether a corpse is tainted based on age.
///
/// From spec Section 7:
/// ```text
/// rotted = (moves - corpse_age) / (10 + rn2(20))
/// if cursed: rotted += 2
/// if blessed: rotted -= 2
/// ```
///
/// Non-rotting corpses (lizard, lichen, Riders, acid blob) skip this check.
///
/// `rot_divisor_rand` is the `rn2(20)` value (0..19), provided by caller.
pub fn check_tainted_corpse(
    current_turn: u32,
    corpse_age: u32,
    cursed: bool,
    blessed: bool,
    nonrotting: bool,
    rot_divisor_rand: u32,
    rng: &mut impl Rng,
) -> TaintedOutcome {
    if nonrotting {
        return TaintedOutcome::Fresh;
    }

    let age_diff = current_turn.saturating_sub(corpse_age) as i32;
    let divisor = (10 + rot_divisor_rand as i32).max(1);
    let mut rotted = age_diff / divisor;

    if cursed {
        rotted += 2;
    }
    if blessed {
        rotted -= 2;
    }

    if rotted > 5 {
        // Tainted: food poisoning rn1(10,10) = 10..19.
        let turns_to_die = rng.random_range(0..10) + 10;
        return TaintedOutcome::Tainted { turns_to_die };
    }

    if rotted > 3 && rng.random_range(0..5) != 0 {
        // Mildly ill: rnd(8) damage.
        let hp_damage = rng.random_range(1..=8);
        return TaintedOutcome::MildlyIll { hp_damage };
    }

    TaintedOutcome::Fresh
}

// ---------------------------------------------------------------------------
// Rotten non-corpse food
// ---------------------------------------------------------------------------

/// Check whether a non-corpse food item has rotted.
///
/// From spec Section 14:
/// ```text
/// if cursed: always rotten
/// else if not nonrotting_food(otyp):
///     if (moves - age) > (blessed ? 50 : 30):
///         if orotten flag OR !rn2(7): rotten
/// ```
///
/// Non-rotting foods: lembas wafer, cram ration (but if cursed, they DO rot).
pub fn check_food_rotten(
    current_turn: u32,
    food_age: u32,
    cursed: bool,
    blessed: bool,
    is_nonrotting_food: bool,
    has_orotten_flag: bool,
    rng: &mut impl Rng,
) -> bool {
    if cursed {
        return true;
    }

    if is_nonrotting_food {
        return false;
    }

    let age_diff = current_turn.saturating_sub(food_age) as i32;
    let threshold = if blessed { 50 } else { 30 };

    if age_diff > threshold
        && (has_orotten_flag || rng.random_range(0..7) == 0)
    {
        return true;
    }

    false
}

/// Rotten food effect outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RottenFoodEffect {
    /// Confusion: d(2,4) turns.
    Confusion { duration: i32 },
    /// Blindness: d(2,10) turns.
    Blindness { duration: i32 },
    /// Unconscious: rnd(10) turns + deafness.
    Unconscious { duration: i32 },
    /// Nothing happened (lucky!).
    Nothing,
}

/// Determine the effect of eating rotten food.
///
/// From spec Section 7:
/// The probability chain is:
/// - 1/4 confusion
/// - 3/4 * 1/4 = 3/16 blindness
/// - 3/4 * 3/4 * 1/3 = 3/16 unconscious
/// - remainder (~6/16) nothing
pub fn rotten_food_effect(rng: &mut impl Rng) -> RottenFoodEffect {
    if rng.random_range(0..4) == 0 {
        let duration = rng.random_range(1..=4) + rng.random_range(1..=4);
        return RottenFoodEffect::Confusion { duration };
    }

    if rng.random_range(0..4) == 0 {
        let duration = rng.random_range(1..=10) + rng.random_range(1..=10);
        return RottenFoodEffect::Blindness { duration };
    }

    if rng.random_range(0..3) == 0 {
        let duration = rng.random_range(1..=10);
        return RottenFoodEffect::Unconscious { duration };
    }

    RottenFoodEffect::Nothing
}

// ---------------------------------------------------------------------------
// Fainting mechanics
// ---------------------------------------------------------------------------

/// Result of a fainting check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FaintingOutcome {
    /// No fainting occurs.
    NoFaint,
    /// Hero faints.
    Faint {
        /// Duration of the faint in turns.
        duration: i32,
    },
}

/// Check whether the hero should faint from hunger.
///
/// From spec Section 2:
/// ```text
/// uhunger_div_by_10 = sgn(uhunger) * ((abs(uhunger) + 5) / 10)
///
/// if u.uhs <= WEAK OR rn2(20 - uhunger_div_by_10) >= 19:
///     faint_duration = 10 - uhunger_div_by_10
///     hero faints
/// ```
///
/// `was_weak_or_worse`: true if previous hunger state was WEAK or worse
/// (i.e., this is a first-time transition to FAINTING from Weak).
/// `already_fainted`: true if the hero is already in FAINTED state.
pub fn check_fainting(
    nutrition: i32,
    was_weak_or_worse: bool,
    already_fainted: bool,
    rng: &mut impl Rng,
) -> FaintingOutcome {
    if nutrition > 0 || already_fainted {
        return FaintingOutcome::NoFaint;
    }

    let abs_hunger = nutrition.unsigned_abs() as i32;
    let div_by_10 = if nutrition >= 0 {
        (abs_hunger + 5) / 10
    } else {
        -((abs_hunger + 5) / 10)
    };

    // Check probability: if was WEAK or worse, always faint.
    // Otherwise: rn2(20 - div_by_10) >= 19.
    let should_faint = if was_weak_or_worse {
        true
    } else {
        let range = (20 - div_by_10).max(1);
        rng.random_range(0..range) >= 19
    };

    if should_faint {
        let duration = (10 - div_by_10).max(1);
        FaintingOutcome::Faint { duration }
    } else {
        FaintingOutcome::NoFaint
    }
}

// ---------------------------------------------------------------------------
// Strength penalty at Weak
// ---------------------------------------------------------------------------

/// Check if a strength penalty should be applied/removed based on hunger
/// state transition.
///
/// Returns the strength modifier change:
/// - Returns -1 when transitioning INTO Weak or worse (from NotHungry/Hungry).
/// - Returns +1 when transitioning OUT OF Weak or worse (recovering).
/// - Returns 0 for no change.
pub fn strength_penalty_change(old_level: HungerLevel, new_level: HungerLevel) -> i8 {
    let old_weak = matches!(
        old_level,
        HungerLevel::Weak | HungerLevel::Fainting | HungerLevel::Fainted
    );
    let new_weak = matches!(
        new_level,
        HungerLevel::Weak | HungerLevel::Fainting | HungerLevel::Fainted
    );

    if new_weak && !old_weak {
        -1 // Entering Weak: apply strength penalty
    } else if !new_weak && old_weak {
        1 // Recovering: remove strength penalty
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Corpse post-eating effects (cpostfx)
// ---------------------------------------------------------------------------

/// Special effects from consuming a corpse (post-eating).
///
/// From spec Section 7 table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpsePostEffect {
    /// Wraith: gain one experience level.
    GainLevel,
    /// Nurse: full HP heal, cure blindness.
    FullHeal,
    /// Stalker: temporary or permanent invisibility + see invisible.
    Invisibility,
    /// Yellow light / giant bat / bat: stun +30 turns.
    Stun { duration: i32 },
    /// Mimics: forced mimicry.
    ForcedMimicry { duration: i32 },
    /// Quantum mechanic: toggle intrinsic speed.
    ToggleSpeed,
    /// Lizard: reduce stun to 2, reduce confusion to 2.
    LizardCure,
    /// Chameleon / Doppelganger / Genetic engineer: polymorph self.
    PolymorphSelf,
    /// Displacer beast: temporary displacement.
    Displacement { duration: i32 },
    /// Disenchanter: strip a random intrinsic.
    StripIntrinsic,
    /// Mind flayer: 50% chance +1 Int.
    GainIntelligence,
    /// Lycanthropy from werewolf/werejackal/wererat corpse.
    Lycanthropy,
    /// No special effect.
    None,
}

// ---------------------------------------------------------------------------
// Special food effects (fpostfx)
// ---------------------------------------------------------------------------

/// Special effects from consuming specific food items.
///
/// From spec Section 15.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialFoodEffect {
    /// Sprig of wolfsbane: cure lycanthropy.
    CureLycanthropy,
    /// Carrot: cure blindness.
    CureBlindness,
    /// Fortune cookie: random rumor + break literate conduct.
    FortuneCookie,
    /// Lump of royal jelly: gain strength, rnd(20) HP heal (or -rnd(20) if cursed).
    RoyalJelly {
        hp_change: i32,
        max_hp_gain: bool,
        cure_wounded_legs: bool,
    },
    /// Eucalyptus leaf: cure sickness and vomiting (unless cursed).
    CureSickness,
    /// Cursed apple: fall asleep.
    FallAsleep { duration: i32 },
    /// Cockatrice/chickatrice egg: stoning.
    StoningEgg,
    /// Pyrolisk egg: explosion (fire damage).
    PyroliskEgg { fire_damage: i32 },
    /// Stale egg: vomiting.
    StaleEgg { vomit_duration: i32 },
    /// Spinach: gain strength.
    GainStrength,
    /// No special effect.
    None,
}

/// Determine the special food effect for a given food name.
///
/// This covers the `fpostfx` logic from the spec.
pub fn special_food_effect(
    food_name: &str,
    cursed: bool,
    _blessed: bool,
    has_sleep_resistance: bool,
    rng: &mut impl Rng,
) -> SpecialFoodEffect {
    match food_name {
        "sprig of wolfsbane" => SpecialFoodEffect::CureLycanthropy,
        "carrot" => SpecialFoodEffect::CureBlindness,
        "fortune cookie" => SpecialFoodEffect::FortuneCookie,
        "lump of royal jelly" => {
            let hp_change = if cursed {
                -rng.random_range(1..=20)
            } else {
                rng.random_range(1..=20)
            };
            // 1/17 chance of +1 max HP.
            let max_hp_gain = rng.random_range(0..17) == 0;
            SpecialFoodEffect::RoyalJelly {
                hp_change,
                max_hp_gain,
                cure_wounded_legs: true,
            }
        }
        "eucalyptus leaf" => {
            if cursed {
                SpecialFoodEffect::None
            } else {
                SpecialFoodEffect::CureSickness
            }
        }
        "apple" if cursed => {
            if has_sleep_resistance {
                SpecialFoodEffect::None
            } else {
                // rn1(11, 20) = rn2(11) + 20 = 20..30
                let duration = rng.random_range(0..11) + 20;
                SpecialFoodEffect::FallAsleep { duration }
            }
        }
        _ => SpecialFoodEffect::None,
    }
}

// ---------------------------------------------------------------------------
// "Nearly full" warning
// ---------------------------------------------------------------------------

/// Check if the "nearly full" warning should be displayed.
///
/// Returns true when nutrition >= 1500 and no Hunger property.
pub fn should_warn_nearly_full(nutrition: i32, has_hunger_property: bool) -> bool {
    nutrition >= NEARLY_FULL_THRESHOLD && !has_hunger_property
}

// ---------------------------------------------------------------------------
// Cannibalism detection (race-based)
// ---------------------------------------------------------------------------

/// Check if eating a given monster corpse counts as cannibalism for the
/// player's race.  The `player_race` is a lowercase race name ("human",
/// "elf", etc.) and `corpse_monster` is the monster's display name.
///
/// Reference: NetHack `src/eat.c` — `maybe_cannibal()`.
pub fn is_cannibalism(player_race: &str, corpse_monster: &str) -> bool {
    let m = corpse_monster.to_lowercase();
    match player_race {
        "human" => {
            m.contains("human")
                || m == "wizard"
                || m == "guard"
                || m == "shopkeeper"
                || m == "oracle"
                || m == "aligned cleric"
                || m == "nurse"
        }
        "elf" => m.contains("elf"),
        "dwarf" => m.contains("dwarf"),
        "gnome" => m.contains("gnome"),
        "orc" => m.contains("orc"),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Cannibalism effects (simplified)
// ---------------------------------------------------------------------------

/// Discrete cannibalism effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CannibalismEffect {
    /// Alignment penalty (negative value).
    AlignmentPenalty(i32),
    /// Display a message.
    Message(String),
    /// Permanently aggravate monsters (first offense only).
    AggravateMonsters,
}

/// Compute cannibalism effects.  First-time cannibalism also causes
/// permanent aggravation.
pub fn cannibalism_effects(is_first_time: bool) -> Vec<CannibalismEffect> {
    let mut effects = vec![
        CannibalismEffect::AlignmentPenalty(-15),
        CannibalismEffect::Message("You cannibal! You feel guilty!".into()),
    ];
    if is_first_time {
        effects.push(CannibalismEffect::AggravateMonsters);
    }
    effects
}

// ---------------------------------------------------------------------------
// Simple conduct violation check (string-based)
// ---------------------------------------------------------------------------

/// Conduct violation category for a food item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConductViolation {
    /// Breaks vegetarian (and therefore vegan) conduct.
    Vegetarian,
    /// Breaks vegan but not vegetarian conduct.
    Vegan,
    /// No conduct violated.
    None,
}

/// Check whether eating a food type violates vegetarian/vegan conduct.
///
/// This is a simplified name-based check; the full system uses
/// `classify_food_conduct` with `Material` data.
pub fn violates_conduct(food_type: &str) -> ConductViolation {
    match food_type {
        "corpse" | "tin" | "meat ring" | "meat stick" | "huge chunk of meat" => {
            ConductViolation::Vegetarian
        }
        "egg" | "cream pie" | "candy bar" | "lump of royal jelly" => {
            ConductViolation::Vegan
        }
        _ => ConductViolation::None,
    }
}

// ---------------------------------------------------------------------------
// Amulet eating effects
// ---------------------------------------------------------------------------

/// Effect of eating an amulet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmuletEatEffect {
    /// Choking from amulet of strangulation.
    Choke,
    /// Unchanging property (prevents polymorph).
    Unchanging,
    /// Life saving effect (one-time death prevention).
    LifeSaving,
    /// Eating cheap plastic imitation — nothing useful.
    Plastic,
    /// Eating the real Amulet of Yendor — very bad idea.
    RealAmulet,
    /// Generic nutrition gain (for other amulets).
    Nutrition(i32),
}

/// Determine the effect of eating a specific amulet.
pub fn eat_amulet_effect(amulet_name: &str) -> AmuletEatEffect {
    match amulet_name {
        "amulet of strangulation" => AmuletEatEffect::Choke,
        "amulet of unchanging" => AmuletEatEffect::Unchanging,
        "amulet of life saving" => AmuletEatEffect::LifeSaving,
        "cheap plastic imitation of the Amulet of Yendor" => AmuletEatEffect::Plastic,
        "Amulet of Yendor" => AmuletEatEffect::RealAmulet,
        _ => AmuletEatEffect::Nutrition(20),
    }
}

// ---------------------------------------------------------------------------
// Ring eating effects
// ---------------------------------------------------------------------------

/// Effect of eating a ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RingEatEffect {
    /// Gain a permanent intrinsic from the ring.
    GainIntrinsic(&'static str),
    /// Generic nutrition gain (for other rings).
    Nutrition(i32),
}

/// Determine the effect of eating a specific ring.
///
/// Eating a ring can grant permanent intrinsics corresponding to the
/// ring's power.  Reference: `src/eat.c` `eatring()`.
pub fn eat_ring_effect(ring_name: &str) -> RingEatEffect {
    match ring_name {
        "ring of fire resistance" => RingEatEffect::GainIntrinsic("fire_resistance"),
        "ring of cold resistance" => RingEatEffect::GainIntrinsic("cold_resistance"),
        "ring of poison resistance" => RingEatEffect::GainIntrinsic("poison_resistance"),
        "ring of shock resistance" => RingEatEffect::GainIntrinsic("shock_resistance"),
        "ring of invisibility" => RingEatEffect::GainIntrinsic("invisibility"),
        "ring of see invisible" => RingEatEffect::GainIntrinsic("see_invisible"),
        "ring of free action" => RingEatEffect::GainIntrinsic("free_action"),
        "ring of teleportation" => RingEatEffect::GainIntrinsic("teleportitis"),
        "ring of teleport control" => RingEatEffect::GainIntrinsic("teleport_control"),
        "ring of levitation" => RingEatEffect::GainIntrinsic("levitation"),
        "ring of regeneration" => RingEatEffect::GainIntrinsic("regeneration"),
        "ring of stealth" => RingEatEffect::GainIntrinsic("stealth"),
        "ring of sustain ability" => RingEatEffect::GainIntrinsic("sustain_ability"),
        "ring of polymorph" => RingEatEffect::GainIntrinsic("polymorphitis"),
        "ring of polymorph control" => RingEatEffect::GainIntrinsic("polymorph_control"),
        "ring of slow digestion" => RingEatEffect::GainIntrinsic("slow_digestion"),
        "ring of hunger" => RingEatEffect::GainIntrinsic("hunger"),
        "ring of conflict" => RingEatEffect::GainIntrinsic("conflict"),
        "ring of warning" => RingEatEffect::GainIntrinsic("warning"),
        "ring of searching" => RingEatEffect::GainIntrinsic("searching"),
        _ => RingEatEffect::Nutrition(15),
    }
}

// ---------------------------------------------------------------------------
// Lizard corpse / stoning cure
// ---------------------------------------------------------------------------

/// Lizard corpse special effects — cures stoning and confusion.
pub fn lizard_corpse_effect() -> Vec<EngineEvent> {
    vec![
        EngineEvent::msg("lizard-cures-stoning"),
        EngineEvent::msg("lizard-cures-confusion"),
    ]
}

/// Check if an item can cure stoning when eaten.
pub fn can_cure_stoning(item_name: &str) -> bool {
    matches!(item_name, "lizard corpse" | "acid" | "potion of acid")
}

// ═══════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Position;
    use crate::world::{GameWorld, Nutrition};
    use rand::SeedableRng;
    use rand_pcg::Pcg64;

    /// Deterministic RNG for reproducible tests.
    fn test_rng() -> Pcg64 {
        Pcg64::seed_from_u64(42)
    }

    fn make_test_world() -> GameWorld {
        GameWorld::new(Position::new(5, 5))
    }

    /// Create a basic food def for testing.
    fn basic_food(name: &str, nutrition: i32, delay: i32, material: Material) -> FoodDef {
        FoodDef {
            name: name.to_string(),
            nutrition,
            oc_delay: delay,
            material,
            is_corpse: false,
            is_tin: false,
            is_glob: false,
            weight: 20,
        }
    }

    /// Create a basic corpse def for testing.
    fn basic_corpse(
        name: &str,
        level: i8,
        weight: u16,
        nutrition: u16,
        conveys: ResistanceSet,
    ) -> CorpseDef {
        CorpseDef {
            name: name.to_string(),
            base_level: level,
            corpse_weight: weight,
            corpse_nutrition: nutrition,
            conveys,
            flags: MonsterFlags::empty(),
            poisonous: false,
            acidic: false,
            flesh_petrifies: false,
            is_giant: false,
            is_domestic: false,
            is_same_race: false,
            cannibal_allowed: false,
            conveys_telepathy: false,
            conveys_teleport: false,
            nonrotting: false,
            is_vegan: false,
            is_vegetarian: false,
        }
    }

    // ── Test 1: Eating food adds nutrition ───────────────────────────

    #[test]
    fn eating_food_adds_nutrition() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let player = world.player();
        let initial = world
            .get_component::<Nutrition>(player)
            .unwrap()
            .0;
        assert_eq!(initial, 900);

        let food = basic_food("food ration", 800, 5, Material::Veggy);
        let food_entity = world.spawn((Nutrition(0),));

        let result = eat_food(&mut world, player, food_entity, &food, &mut rng);

        let after = world
            .get_component::<Nutrition>(player)
            .unwrap()
            .0;
        assert_eq!(after, 900 + 800, "nutrition should increase by food value");
        assert!(result.consumed, "food should be consumed");
        assert!(!result.died, "should not die from normal eating");
    }

    // ── Test 2: Choking when satiated (5% survival, NOT 95%) ────────

    #[test]
    fn choking_when_satiated_mostly_fatal() {
        // When eating while satiated and nutrition >= 2000, the survival
        // chance is 5% (1/20), meaning ~95% of the time the hero chokes.
        let mut rng = test_rng();
        let mut survived = 0;
        let mut died = 0;
        let trials = 10000;

        for _ in 0..trials {
            let outcome = check_choking(
                CHOKING_THRESHOLD,
                true,  // canchoke
                false, // breathless
                false, // hunger property
                false, // strangled
                &mut rng,
            );
            match outcome {
                ChokeOutcome::Vomited { .. } => survived += 1,
                ChokeOutcome::Choked => died += 1,
                ChokeOutcome::NoChoke => {
                    panic!("should not get NoChoke when above threshold")
                }
            }
        }

        let survival_rate = survived as f64 / trials as f64;
        // 5% = 0.05, allow [0.03, 0.08] margin.
        assert!(
            (0.03..=0.08).contains(&survival_rate),
            "survival rate {:.4} should be ~5%, NOT 95%. Survived: {}, Died: {}",
            survival_rate,
            survived,
            died
        );
    }

    // ── Test 3: Poisonous corpse with resistance ────────────────────

    #[test]
    fn poisonous_corpse_with_resistance() {
        let mut rng = test_rng();

        // With resistance: always Resisted (when poison activates).
        let mut resisted = 0;
        let mut not_triggered = 0;
        for _ in 0..1000 {
            let outcome = check_poison(true, true, &mut rng);
            match outcome {
                PoisonOutcome::Resisted => resisted += 1,
                PoisonOutcome::NotPoisonous => not_triggered += 1,
                PoisonOutcome::Poisoned { .. } => {
                    panic!("should never be poisoned with resistance")
                }
            }
        }
        // 80% activate -> Resisted, 20% not triggered.
        assert!(resisted > 0, "should get some Resisted outcomes");
        assert!(not_triggered > 0, "20% should not trigger");
    }

    // ── Test 4: Poisonous corpse without resistance ─────────────────

    #[test]
    fn poisonous_corpse_without_resistance() {
        let mut rng = test_rng();

        let mut poisoned = 0;
        for _ in 0..1000 {
            let outcome = check_poison(true, false, &mut rng);
            match outcome {
                PoisonOutcome::Poisoned { hp_damage, str_loss } => {
                    assert!(hp_damage >= 1 && hp_damage <= 15);
                    assert!(str_loss >= 1 && str_loss <= 4);
                    poisoned += 1;
                }
                PoisonOutcome::NotPoisonous => { /* 20% no trigger */ }
                PoisonOutcome::Resisted => {
                    panic!("should not get Resisted without resistance")
                }
            }
        }
        // 80% should be poisoned.
        let poison_rate = poisoned as f64 / 1000.0;
        assert!(
            (0.70..=0.90).contains(&poison_rate),
            "poison rate {:.3} should be ~80%",
            poison_rate
        );
    }

    // ── Test 5: Corpse intrinsic gain (floating eye -> telepathy) ───

    #[test]
    fn floating_eye_gives_telepathy() {
        let mut rng = test_rng();

        // Floating eye: level 3, conveys telepathy, chance=1.
        // 3 > rn2(1) is always true, so telepathy is always granted
        // (when telepathy is selected as the candidate).
        let mut corpse = basic_corpse(
            "floating eye",
            3,
            10,
            10,
            ResistanceSet::empty(),
        );
        corpse.conveys_telepathy = true;

        // With only telepathy as candidate, it should always be selected
        // and always pass the check (chance=1, level=3 > rn2(1)=0 always).
        let mut gained = 0;
        let trials = 100;
        for _ in 0..trials {
            if let Some(CorpseIntrinsic::Telepathy) =
                check_intrinsic_gain(&corpse, &mut rng)
            {
                gained += 1;
            }
        }
        assert_eq!(
            gained, trials,
            "floating eye should always grant telepathy"
        );
    }

    // ── Test 6: Cannibalism penalty ─────────────────────────────────

    #[test]
    fn cannibalism_penalty_applied() {
        let mut rng = test_rng();

        // Same race, not cannibal-allowed.
        let penalty = check_cannibalism(true, false, &mut rng);
        assert!(penalty.is_some(), "should get cannibalism penalty");

        let p = penalty.unwrap();
        assert!(p.aggravate, "should get permanent aggravate");
        assert!(
            p.luck_penalty >= -5 && p.luck_penalty <= -2,
            "luck penalty {} should be -2..-5",
            p.luck_penalty
        );
    }

    #[test]
    fn cannibalism_exempt_for_orcs() {
        let mut rng = test_rng();

        // Same race but cannibal allowed (Orc).
        let penalty = check_cannibalism(true, true, &mut rng);
        assert!(
            penalty.is_none(),
            "orcs should be exempt from cannibalism penalty"
        );
    }

    // ── Test 7: Starvation threshold exact formula ──────────────────

    #[test]
    fn starvation_threshold_exact_formula() {
        // Con 18: threshold = -(100 + 180) = -280.
        assert_eq!(starvation_threshold(18), -280);
        // At exactly -280: should NOT starve (strict <).
        assert!(!should_starve(-280, 18));
        // At -281: should starve.
        assert!(should_starve(-281, 18));

        // Con 3: threshold = -(100 + 30) = -130.
        assert_eq!(starvation_threshold(3), -130);
        assert!(!should_starve(-130, 3));
        assert!(should_starve(-131, 3));

        // Con 10: threshold = -(100 + 100) = -200.
        assert_eq!(starvation_threshold(10), -200);
        assert!(!should_starve(-200, 10));
        assert!(should_starve(-201, 10));
    }

    // ── Test 8: Vegan conduct violated by meat ──────────────────────

    #[test]
    fn vegan_conduct_violated_by_meat() {
        let class = classify_food_conduct(Material::Flesh, false, None);
        assert_eq!(class, FoodConductClass::Meat);

        let mut food_count = 0i64;
        let mut unvegan = 0i64;
        let mut unveg = 0i64;

        let violations = update_conducts(class, &mut food_count, &mut unvegan, &mut unveg);

        assert!(violations.broke_vegan, "meat should break vegan conduct");
        assert!(
            violations.broke_vegetarian,
            "meat should break vegetarian conduct"
        );
        assert_eq!(food_count, 1);
        assert_eq!(unvegan, 1);
        assert_eq!(unveg, 1);
    }

    // ── Test 9: Vegetarian conduct not violated by veggy food ───────

    #[test]
    fn vegetarian_conduct_not_violated_by_veggy() {
        let class = classify_food_conduct(Material::Veggy, false, None);
        assert_eq!(class, FoodConductClass::Vegan);

        let mut food_count = 0i64;
        let mut unvegan = 0i64;
        let mut unveg = 0i64;

        let violations = update_conducts(class, &mut food_count, &mut unvegan, &mut unveg);

        assert!(
            !violations.broke_vegan,
            "veggy food should not break vegan conduct"
        );
        assert!(
            !violations.broke_vegetarian,
            "veggy food should not break vegetarian conduct"
        );
        assert_eq!(food_count, 1, "foodless conduct always broken");
        assert_eq!(unvegan, 0);
        assert_eq!(unveg, 0);
    }

    // ── Test 10: Tin opening time with/without opener ───────────────

    #[test]
    fn tin_opening_time_with_opener() {
        let mut rng = test_rng();

        // With tin opener: should be 0 or 1.
        let mut times = Vec::new();
        for _ in 0..100 {
            let t = tin_opening_time(
                true,  // has_tin_opener
                false, // opener_blessed
                false, // has_dagger
                false, // has_pick
                false, // is_strong
                10,    // dex
                10,    // str
                false, // tin_blessed
                &mut rng,
            );
            assert!(
                t == 0 || t == 1,
                "tin opener should give 0 or 1, got {}",
                t
            );
            times.push(t);
        }
        // Should see both values.
        assert!(times.contains(&0), "should sometimes be 0");
        assert!(times.contains(&1), "should sometimes be 1");
    }

    #[test]
    fn tin_opening_time_bare_hands() {
        let mut rng = test_rng();

        // Bare hands with STR=10, DEX=10: rn1(1 + 500/20, 10) = rn1(26, 10)
        // = rn2(26) + 10 = 10..35
        for _ in 0..100 {
            let t = tin_opening_time(
                false, // no opener
                false,
                false, // no dagger
                false, // no pick
                false, // not strong
                10,
                10,
                false,
                &mut rng,
            );
            assert!(
                t >= 10 && t <= 50,
                "bare hands opening time {} should be 10..50",
                t
            );
        }
    }

    #[test]
    fn tin_opening_time_strong() {
        let mut rng = test_rng();

        // Strong hero: always 2 turns.
        let t = tin_opening_time(
            false,
            false,
            false,
            false,
            true, // is_strong
            10,
            10,
            false,
            &mut rng,
        );
        assert_eq!(t, 2, "strong hero should open tin in 2 turns");
    }

    // ── Test 11: Giant corpse gives strength ────────────────────────

    #[test]
    fn giant_corpse_gives_strength() {
        let mut rng = test_rng();

        // Giant: level 6, is_giant=true, no other conveys.
        let mut corpse = basic_corpse("hill giant", 6, 2000, 750, ResistanceSet::empty());
        corpse.is_giant = true;

        // Strength is the only candidate. 50% chance of nothing, 50% of Strength.
        let mut got_strength = 0;
        let trials = 1000;
        for _ in 0..trials {
            if let Some(CorpseIntrinsic::Strength) =
                check_intrinsic_gain(&corpse, &mut rng)
            {
                got_strength += 1;
            }
        }

        let rate = got_strength as f64 / trials as f64;
        assert!(
            (0.40..=0.60).contains(&rate),
            "giant strength gain rate {:.3} should be ~50%",
            rate
        );
    }

    // ── Test 12: Eating time for corpses ────────────────────────────

    #[test]
    fn corpse_eating_time_formula() {
        // cwt=400: 3 + 400/64 = 3 + 6 = 9
        assert_eq!(eating_time_corpse(400), 9);
        // cwt=0: 3 + 0 = 3 (minimum)
        assert_eq!(eating_time_corpse(0), 3);
        // cwt=64: 3 + 1 = 4
        assert_eq!(eating_time_corpse(64), 4);
        // cwt=2000: 3 + 31 = 34
        assert_eq!(eating_time_corpse(2000), 34);
    }

    // ── Test 13: Acid corpse with/without resistance ────────────────

    #[test]
    fn acid_corpse_with_resistance() {
        let mut rng = test_rng();
        let outcome = check_acid(true, true, &mut rng);
        assert_eq!(outcome, AcidOutcome::Resisted);
    }

    #[test]
    fn acid_corpse_without_resistance() {
        let mut rng = test_rng();
        let outcome = check_acid(true, false, &mut rng);
        match outcome {
            AcidOutcome::Damaged { hp_damage } => {
                assert!(hp_damage >= 1 && hp_damage <= 15);
            }
            _ => panic!("should take acid damage without resistance"),
        }
    }

    // ── Test 14: Stoning from cockatrice ────────────────────────────

    #[test]
    fn stoning_from_cockatrice() {
        assert_eq!(
            check_stoning(true, false),
            StoningOutcome::Petrified,
            "should be petrified without stone resistance"
        );
        assert_eq!(
            check_stoning(true, true),
            StoningOutcome::Resisted,
            "should resist with stone resistance"
        );
        assert_eq!(
            check_stoning(false, false),
            StoningOutcome::NotPetrifying,
            "non-petrifying corpse should have no effect"
        );
    }

    // ── Test 15: Hunger thresholds match spec ───────────────────────

    #[test]
    fn hunger_thresholds_match_spec() {
        assert_eq!(nutrition_to_hunger_level(1001), HungerLevel::Satiated);
        assert_eq!(nutrition_to_hunger_level(1000), HungerLevel::NotHungry);
        assert_eq!(nutrition_to_hunger_level(151), HungerLevel::NotHungry);
        assert_eq!(nutrition_to_hunger_level(150), HungerLevel::Hungry);
        assert_eq!(nutrition_to_hunger_level(51), HungerLevel::Hungry);
        assert_eq!(nutrition_to_hunger_level(50), HungerLevel::Weak);
        assert_eq!(nutrition_to_hunger_level(1), HungerLevel::Weak);
        assert_eq!(nutrition_to_hunger_level(0), HungerLevel::Fainting);
        assert_eq!(nutrition_to_hunger_level(-1), HungerLevel::Fainting);
    }

    // ── Test 16: Spinach tin nutrition values ────────────────────────

    #[test]
    fn spinach_tin_nutrition() {
        let mut rng = test_rng();

        // Blessed: always 600.
        assert_eq!(tin_nutrition(TinVariety::Spinach, 100, true, false, &mut rng), 600);

        // Uncursed: 401..600.
        for _ in 0..100 {
            let n = tin_nutrition(TinVariety::Spinach, 100, false, false, &mut rng);
            assert!(
                n >= 401 && n <= 600,
                "uncursed spinach nutrition {} should be 401..600",
                n
            );
        }

        // Cursed: 201..600.
        for _ in 0..100 {
            let n = tin_nutrition(TinVariety::Spinach, 100, false, true, &mut rng);
            assert!(
                n >= 201 && n <= 600,
                "cursed spinach nutrition {} should be 201..600",
                n
            );
        }
    }

    // ── Test 17: Choking with breathless guarantees survival ────────

    #[test]
    fn choking_breathless_always_survives() {
        let mut rng = test_rng();

        for _ in 0..100 {
            let outcome = check_choking(
                CHOKING_THRESHOLD + 100,
                true,  // canchoke
                true,  // breathless
                false,
                false,
                &mut rng,
            );
            match outcome {
                ChokeOutcome::Vomited { nutrition_lost } => {
                    assert_eq!(nutrition_lost, 1000);
                }
                _ => panic!("breathless should always survive choking"),
            }
        }
    }

    // ── Test 18: Eating food in world updates nutrition component ───

    #[test]
    fn eat_food_updates_world_nutrition() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        // Set nutrition low so we don't choke.
        let player = world.player();
        if let Some(mut n) = world.get_component_mut::<Nutrition>(player) {
            n.0 = 100;
        }

        let food = basic_food("apple", 50, 1, Material::Veggy);
        let food_entity = world.spawn((Nutrition(0),));

        let result = eat_food(&mut world, player, food_entity, &food, &mut rng);

        let after = world
            .get_component::<Nutrition>(player)
            .unwrap()
            .0;
        assert_eq!(after, 150);
        assert!(!result.died);
        assert!(!result.conduct.broke_vegan, "apple is vegan");
        assert!(!result.conduct.broke_vegetarian, "apple is vegetarian");
    }

    // ── Test 19: Eating corpse in world with full pipeline ──────────

    #[test]
    fn eat_corpse_full_pipeline() {
        let mut world = make_test_world();
        let mut rng = test_rng();

        let player = world.player();
        if let Some(mut n) = world.get_component_mut::<Nutrition>(player) {
            n.0 = 200;
        }

        let corpse_def = basic_corpse("newt", 1, 10, 20, ResistanceSet::empty());
        let corpse_entity = world.spawn((Nutrition(0),));

        let result = eat_corpse(
            &mut world,
            player,
            corpse_entity,
            &corpse_def,
            &mut rng,
        );

        let after = world.get_component::<Nutrition>(player).unwrap().0;
        assert_eq!(after, 220, "should gain 20 nutrition from newt corpse");
        assert!(!result.died);
        assert_eq!(result.eating_time, 3, "tiny corpse eating time = 3");
    }

    // ── Test 20: No choking when not satiated ───────────────────────

    #[test]
    fn no_choking_when_not_satiated() {
        let mut rng = test_rng();

        // canchoke = false (was not satiated at meal start).
        let outcome = check_choking(
            CHOKING_THRESHOLD + 500,
            false, // canchoke
            false,
            false,
            false,
            &mut rng,
        );
        assert_eq!(outcome, ChokeOutcome::NoChoke);
    }

    // ═══════════════════════════════════════════════════════════════════
    // New tests for hunger system alignment (Track I.3)
    // ═══════════════════════════════════════════════════════════════════

    // ── Per-turn depletion tests ─────────────────────────────────────

    #[test]
    fn test_hunger_depletion_base_only() {
        // With all accessories off, base depletion = 1.
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            ..Default::default()
        };
        // Regardless of accessorytime, base is always 1.
        for t in 0..20 {
            let d = compute_hunger_depletion(&ctx, t);
            assert_eq!(d, 1, "base depletion should be 1 for accessorytime={}", t);
        }
    }

    #[test]
    fn test_hunger_depletion_slow_digestion_suppresses_base() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            slow_digestion: true,
            ..Default::default()
        };
        // Base depletion should be 0 when Slow_digestion active.
        for t in 0..20 {
            let d = compute_hunger_depletion(&ctx, t);
            assert_eq!(d, 0, "Slow_digestion should suppress base depletion at t={}", t);
        }
    }

    #[test]
    fn test_hunger_depletion_invulnerable_zero() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            invulnerable: true,
            ..Default::default()
        };
        assert_eq!(compute_hunger_depletion(&ctx, 5), 0,
            "invulnerable (praying) should not deplete hunger");
    }

    #[test]
    fn test_hunger_depletion_regeneration_odd_only() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            has_regeneration: true,
            ..Default::default()
        };
        // Odd accessorytime: base(1) + regeneration(1) = 2.
        assert_eq!(compute_hunger_depletion(&ctx, 1), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 3), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 19), 2);
        // Even accessorytime: base(1) only = 1.
        assert_eq!(compute_hunger_depletion(&ctx, 0), 1);
        assert_eq!(compute_hunger_depletion(&ctx, 2), 1);
    }

    #[test]
    fn test_hunger_depletion_encumbrance_odd_only() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            stressed_or_worse: true,
            ..Default::default()
        };
        // Odd: base(1) + encumbrance(1) = 2.
        assert_eq!(compute_hunger_depletion(&ctx, 5), 2);
        // Even: base(1) only.
        assert_eq!(compute_hunger_depletion(&ctx, 6), 1);
    }

    #[test]
    fn test_hunger_depletion_hunger_property_even() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            has_hunger_property: true,
            ..Default::default()
        };
        // Even 0,2,6,10,14,18: base(1) + hunger(1) = 2.
        assert_eq!(compute_hunger_depletion(&ctx, 0), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 2), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 6), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 10), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 14), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 18), 2);
        // Even 4,8,12,16: base(1) only (not in the general even set).
        assert_eq!(compute_hunger_depletion(&ctx, 4), 1);
        assert_eq!(compute_hunger_depletion(&ctx, 8), 1);
        assert_eq!(compute_hunger_depletion(&ctx, 12), 1);
        assert_eq!(compute_hunger_depletion(&ctx, 16), 1);
        // Odd: base(1) only.
        assert_eq!(compute_hunger_depletion(&ctx, 1), 1);
    }

    #[test]
    fn test_hunger_depletion_left_ring_at_4() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            left_ring_causes_hunger: true,
            ..Default::default()
        };
        // accessorytime=4: base(1) + left_ring(1) = 2.
        assert_eq!(compute_hunger_depletion(&ctx, 4), 2);
        // Other times: base(1) only.
        assert_eq!(compute_hunger_depletion(&ctx, 0), 1);
        assert_eq!(compute_hunger_depletion(&ctx, 8), 1);
        assert_eq!(compute_hunger_depletion(&ctx, 12), 1);
    }

    #[test]
    fn test_hunger_depletion_right_ring_at_12() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            right_ring_causes_hunger: true,
            ..Default::default()
        };
        // accessorytime=12: base(1) + right_ring(1) = 2.
        assert_eq!(compute_hunger_depletion(&ctx, 12), 2);
        // Other: base only.
        assert_eq!(compute_hunger_depletion(&ctx, 4), 1);
    }

    #[test]
    fn test_hunger_depletion_amulet_at_8() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            amulet_causes_hunger: true,
            ..Default::default()
        };
        assert_eq!(compute_hunger_depletion(&ctx, 8), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 0), 1);
    }

    #[test]
    fn test_hunger_depletion_carrying_amulet_at_16() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            carrying_amulet: true,
            ..Default::default()
        };
        assert_eq!(compute_hunger_depletion(&ctx, 16), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 0), 1);
    }

    #[test]
    fn test_hunger_depletion_conflict_even() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            has_conflict: true,
            ..Default::default()
        };
        // Conflict applies at even 0,2,6,10,14,18.
        assert_eq!(compute_hunger_depletion(&ctx, 0), 2);
        assert_eq!(compute_hunger_depletion(&ctx, 6), 2);
        // Odd: no conflict.
        assert_eq!(compute_hunger_depletion(&ctx, 1), 1);
        // Even 4: no general even check.
        assert_eq!(compute_hunger_depletion(&ctx, 4), 1);
    }

    #[test]
    fn test_hunger_depletion_slow_digestion_armor_at_0() {
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            slow_digestion_from_armor: true,
            ..Default::default()
        };
        assert_eq!(compute_hunger_depletion(&ctx, 0), 2,
            "Slow_digestion from armor costs hunger at accessorytime=0");
        assert_eq!(compute_hunger_depletion(&ctx, 2), 1);
    }

    #[test]
    fn test_hunger_depletion_worst_case_average() {
        // Worst case: all sources active.
        let ctx = AccessoryHungerCtx {
            can_eat: true,
            has_regeneration: true,
            stressed_or_worse: true,
            has_hunger_property: true,
            has_conflict: true,
            left_ring_causes_hunger: true,
            right_ring_causes_hunger: true,
            amulet_causes_hunger: true,
            carrying_amulet: true,
            ..Default::default()
        };

        let total: i32 = (0..20).map(|t| compute_hunger_depletion(&ctx, t)).sum();
        let average = total as f64 / 20.0;

        // Per spec: max ~5 per turn on average.
        assert!(
            (2.0..=6.0).contains(&average),
            "worst case average depletion {:.2} should be ~3-5",
            average
        );
    }

    // ── Nutrition per bite (nmod) tests ──────────────────────────────

    #[test]
    fn test_hunger_nmod_food_ration() {
        // Food ration: nutrition=800, delay=5, full (oeaten=800).
        // nmod = -(800 / 5) = -160.
        let result = calc_nmod(5, 800);
        assert_eq!(result.nmod, -160, "food ration nmod should be -160");
    }

    #[test]
    fn test_hunger_nmod_zero_reqtime() {
        let result = calc_nmod(0, 100);
        assert_eq!(result.nmod, 0, "nmod should be 0 when reqtime is 0");
    }

    #[test]
    fn test_hunger_nmod_zero_oeaten() {
        let result = calc_nmod(5, 0);
        assert_eq!(result.nmod, 0, "nmod should be 0 when oeaten is 0");
    }

    #[test]
    fn test_hunger_nmod_small_oeaten() {
        // oeaten=2, reqtime=5: oeaten < reqtime, nmod = reqtime % oeaten = 5 % 2 = 1.
        let result = calc_nmod(5, 2);
        assert_eq!(result.nmod, 1, "nmod should be positive when oeaten < reqtime");
    }

    #[test]
    fn test_hunger_nmod_apple() {
        // Apple: nutrition=50, delay=1. nmod = -(50/1) = -50.
        let result = calc_nmod(1, 50);
        assert_eq!(result.nmod, -50, "apple nmod should be -50 (all nutrition in one bite)");
    }

    #[test]
    fn test_hunger_nmod_corpse_weight_400() {
        // Corpse cwt=400: reqtime = 3 + 400/64 = 9. nutrition=200.
        // nmod = -(200/9) = -22.
        let reqtime = eating_time_corpse(400);
        assert_eq!(reqtime, 9);
        let result = calc_nmod(reqtime, 200);
        assert_eq!(result.nmod, -22, "corpse nmod should be -(200/9)=-22");
    }

    // ── Partial eating time adjustment ───────────────────────────────

    #[test]
    fn test_hunger_partial_eating_time() {
        // Food ration: reqtime=5, base_nutrit=800, half eaten (oeaten=400).
        // rounddiv(5 * 400, 800) = rounddiv(2000, 800) = 2 (2000/800=2.5, rounds to 3?).
        // Actually: 2000 / 800 = 2, remainder 400, 2*400=800 >= 800, so +1 = 3.
        let t = adjust_eating_time_partial(5, 400, 800);
        assert_eq!(t, 3, "half-eaten food ration should take ~3 turns");
    }

    #[test]
    fn test_hunger_partial_eating_time_full() {
        // Full food: oeaten == base_nutrit.
        let t = adjust_eating_time_partial(5, 800, 800);
        assert_eq!(t, 5, "full food should take full time");
    }

    #[test]
    fn test_hunger_partial_eating_time_tiny() {
        // Almost empty: oeaten=1, base=800, reqtime=5.
        // rounddiv(5 * 1, 800) = rounddiv(5, 800) = 0.
        let t = adjust_eating_time_partial(5, 1, 800);
        assert_eq!(t, 0, "nearly empty food should take ~0 turns");
    }

    // ── Racial modifier tests ───────────────────────────────────────

    #[test]
    fn test_hunger_racial_lembas_elf() {
        // Lembas wafer with Elf: +25% (base * 5/4).
        let adjusted = adj_victual_nutrition(160, "lembas wafer", HeroRace::Elf);
        assert_eq!(adjusted, 200, "elf lembas per-bite should be 25% more");
    }

    #[test]
    fn test_hunger_racial_lembas_orc() {
        // Lembas wafer with Orc: -25% (base * 3/4).
        let adjusted = adj_victual_nutrition(160, "lembas wafer", HeroRace::Orc);
        assert_eq!(adjusted, 120, "orc lembas per-bite should be 25% less");
    }

    #[test]
    fn test_hunger_racial_cram_dwarf() {
        // Cram ration with Dwarf: +17% (base * 7/6).
        let adjusted = adj_victual_nutrition(120, "cram ration", HeroRace::Dwarf);
        assert_eq!(adjusted, 140, "dwarf cram per-bite should be ~17% more");
    }

    #[test]
    fn test_hunger_racial_lembas_human() {
        // Human: no modifier.
        let adjusted = adj_victual_nutrition(160, "lembas wafer", HeroRace::Human);
        assert_eq!(adjusted, 160, "human should have no racial modifier");
    }

    #[test]
    fn test_hunger_racial_food_ration_elf() {
        // Food ration for elf: no modifier (only lembas is special).
        let adjusted = adj_victual_nutrition(160, "food ration", HeroRace::Elf);
        assert_eq!(adjusted, 160, "elf eating food ration should have no modifier");
    }

    // ── Effective nutrition with racial modifiers ────────────────────

    #[test]
    fn test_hunger_racial_effective_lembas_elf() {
        // Lembas wafer: nutrition=800, delay=2, elf.
        // Base nmod = -(800/2) = -400 per bite.
        // With elf: each bite gives 400 * 5/4 = 500.
        // Total over 2 bites: 500 * 2 = 1000.
        let nmod = calc_nmod(2, 800);
        assert_eq!(nmod.nmod, -400);
        let per_bite = adj_victual_nutrition(400, "lembas wafer", HeroRace::Elf);
        assert_eq!(per_bite, 500);
        assert_eq!(per_bite * 2, 1000, "elf should get ~1000 effective nutrition from lembas");
    }

    #[test]
    fn test_hunger_racial_effective_lembas_orc() {
        // Orc with lembas: each bite gives 400 * 3/4 = 300.
        // Total: 300 * 2 = 600.
        let nmod = calc_nmod(2, 800);
        assert_eq!(nmod.nmod, -400);
        let per_bite = adj_victual_nutrition(400, "lembas wafer", HeroRace::Orc);
        assert_eq!(per_bite, 300);
        assert_eq!(per_bite * 2, 600, "orc should get ~600 effective nutrition from lembas");
    }

    // ── Tainted corpse tests ────────────────────────────────────────

    #[test]
    fn test_hunger_tainted_fresh_corpse() {
        let mut rng = test_rng();
        // Very young corpse: age_diff = 5, divisor = 10+10=20.
        // rotted = 5/20 = 0. Should be fresh.
        let result = check_tainted_corpse(105, 100, false, false, false, 10, &mut rng);
        assert_eq!(result, TaintedOutcome::Fresh);
    }

    #[test]
    fn test_hunger_tainted_old_corpse() {
        let mut rng = test_rng();
        // Very old corpse: age_diff = 200, divisor = 10+0=10.
        // rotted = 200/10 = 20. Should be tainted (> 5).
        let result = check_tainted_corpse(300, 100, false, false, false, 0, &mut rng);
        match result {
            TaintedOutcome::Tainted { turns_to_die } => {
                assert!(
                    turns_to_die >= 10 && turns_to_die <= 19,
                    "turns_to_die {} should be 10..19",
                    turns_to_die
                );
            }
            _ => panic!("very old corpse should be tainted, got {:?}", result),
        }
    }

    #[test]
    fn test_hunger_tainted_cursed_adds_2() {
        let mut rng = test_rng();
        // age_diff = 50, divisor = 10+5=15. rotted = 50/15 = 3.
        // With cursed: rotted = 3 + 2 = 5. Still <= 5, so not fully tainted,
        // but > 3 with rn2(5)!=0 check means possible MildlyIll.
        let result = check_tainted_corpse(150, 100, true, false, false, 5, &mut rng);
        // rotted = 5, which is NOT > 5 for tainted, but IS > 3 for mildly ill.
        assert!(
            matches!(result, TaintedOutcome::MildlyIll { .. } | TaintedOutcome::Fresh),
            "cursed corpse with rotted=5 should be mildly ill or fresh (rng)"
        );
    }

    #[test]
    fn test_hunger_tainted_blessed_subtracts_2() {
        let mut rng = test_rng();
        // age_diff = 50, divisor = 10+5=15. rotted = 50/15 = 3.
        // With blessed: rotted = 3 - 2 = 1. Should be fresh.
        let result = check_tainted_corpse(150, 100, false, true, false, 5, &mut rng);
        assert_eq!(result, TaintedOutcome::Fresh, "blessed should reduce rot");
    }

    #[test]
    fn test_hunger_tainted_nonrotting_always_fresh() {
        let mut rng = test_rng();
        // Nonrotting: always fresh regardless of age.
        let result = check_tainted_corpse(10000, 100, false, false, true, 0, &mut rng);
        assert_eq!(result, TaintedOutcome::Fresh, "nonrotting corpse is always fresh");
    }

    // ── Rotten non-corpse food tests ────────────────────────────────

    #[test]
    fn test_hunger_rotten_cursed_always() {
        let mut rng = test_rng();
        assert!(
            check_food_rotten(100, 50, true, false, false, false, &mut rng),
            "cursed food is always rotten"
        );
    }

    #[test]
    fn test_hunger_rotten_nonrotting_food() {
        let mut rng = test_rng();
        // Lembas wafer (non-rotting), not cursed: never rotten.
        assert!(
            !check_food_rotten(1000, 50, false, false, true, false, &mut rng),
            "non-rotting food should not rot"
        );
    }

    #[test]
    fn test_hunger_rotten_young_food() {
        let mut rng = test_rng();
        // age_diff = 10, threshold 30: not rotten.
        assert!(
            !check_food_rotten(60, 50, false, false, false, false, &mut rng),
            "young food should not be rotten"
        );
    }

    #[test]
    fn test_hunger_rotten_old_food_with_flag() {
        let mut rng = test_rng();
        // age_diff = 40 > 30 (uncursed threshold), orotten flag set.
        assert!(
            check_food_rotten(90, 50, false, false, false, true, &mut rng),
            "old food with orotten flag should be rotten"
        );
    }

    #[test]
    fn test_hunger_rotten_blessed_higher_threshold() {
        let mut rng = test_rng();
        // age_diff = 40, blessed threshold 50: not rotten (40 < 50).
        assert!(
            !check_food_rotten(90, 50, false, true, false, true, &mut rng),
            "blessed food has higher rot threshold (50 vs 30)"
        );
    }

    // ── Rotten food effect distribution ─────────────────────────────

    #[test]
    fn test_hunger_rotten_food_effect_distribution() {
        let mut rng = test_rng();
        let trials = 10000;
        let mut confusion = 0;
        let mut blindness = 0;
        let mut unconscious = 0;
        let mut nothing = 0;

        for _ in 0..trials {
            match rotten_food_effect(&mut rng) {
                RottenFoodEffect::Confusion { duration } => {
                    assert!(duration >= 2 && duration <= 8, "confusion duration {} out of range", duration);
                    confusion += 1;
                }
                RottenFoodEffect::Blindness { duration } => {
                    assert!(duration >= 2 && duration <= 20, "blindness duration {} out of range", duration);
                    blindness += 1;
                }
                RottenFoodEffect::Unconscious { duration } => {
                    assert!(duration >= 1 && duration <= 10, "unconscious duration {} out of range", duration);
                    unconscious += 1;
                }
                RottenFoodEffect::Nothing => nothing += 1,
            }
        }

        let total = trials as f64;
        // From spec: ~25% confusion, ~19% blindness, ~19% unconscious, ~37% nothing.
        // Actually: 1/4=25% confusion, 3/16=18.75% blindness, 3/16=18.75% unconscious,
        // 6/16=37.5% nothing.
        let conf_rate = confusion as f64 / total;
        let blind_rate = blindness as f64 / total;
        let uncon_rate = unconscious as f64 / total;
        let noth_rate = nothing as f64 / total;

        assert!((0.20..=0.30).contains(&conf_rate),
            "confusion rate {:.3} should be ~25%", conf_rate);
        assert!((0.13..=0.24).contains(&blind_rate),
            "blindness rate {:.3} should be ~19%", blind_rate);
        assert!((0.13..=0.24).contains(&uncon_rate),
            "unconscious rate {:.3} should be ~19%", uncon_rate);
        assert!((0.30..=0.45).contains(&noth_rate),
            "nothing rate {:.3} should be ~37%", noth_rate);
    }

    // ── Fainting tests ──────────────────────────────────────────────

    #[test]
    fn test_hunger_fainting_positive_nutrition_no_faint() {
        let mut rng = test_rng();
        // nutrition > 0: never faint.
        let result = check_fainting(1, false, false, &mut rng);
        assert_eq!(result, FaintingOutcome::NoFaint);
    }

    #[test]
    fn test_hunger_fainting_already_fainted_no_faint() {
        let mut rng = test_rng();
        // Already fainted: don't faint again.
        let result = check_fainting(-10, true, true, &mut rng);
        assert_eq!(result, FaintingOutcome::NoFaint);
    }

    #[test]
    fn test_hunger_fainting_from_weak_always_faints() {
        let mut rng = test_rng();
        // was_weak_or_worse = true: always faints.
        for _ in 0..100 {
            let result = check_fainting(0, true, false, &mut rng);
            match result {
                FaintingOutcome::Faint { duration } => {
                    // nutrition=0: div_by_10 = sgn(0)*((0+5)/10) = 0.
                    // duration = 10 - 0 = 10.
                    assert_eq!(duration, 10, "faint duration at nutrition=0 should be 10");
                }
                FaintingOutcome::NoFaint => {
                    panic!("should always faint when transitioning from Weak");
                }
            }
        }
    }

    #[test]
    fn test_hunger_fainting_duration_increases_with_negative_nutrition() {
        let mut rng = test_rng();
        // nutrition = -50: div_by_10 = -(55/10) = -5.
        // duration = 10 - (-5) = 15.
        let result = check_fainting(-50, true, false, &mut rng);
        match result {
            FaintingOutcome::Faint { duration } => {
                assert_eq!(duration, 15, "faint duration at nutrition=-50 should be 15");
            }
            _ => panic!("should faint from weak state"),
        }
    }

    #[test]
    fn test_hunger_fainting_probability_increases() {
        let mut rng = test_rng();
        // was_weak_or_worse = false: probability check applies.
        // nutrition = 0: div_by_10 = 0. rn2(20) >= 19 -> 1/20 chance.
        let trials = 10000;
        let mut fainted = 0;
        for _ in 0..trials {
            if let FaintingOutcome::Faint { .. } = check_fainting(0, false, false, &mut rng) {
                fainted += 1;
            }
        }
        let rate = fainted as f64 / trials as f64;
        assert!(
            (0.03..=0.08).contains(&rate),
            "fainting rate {:.3} at nutrition=0 should be ~5% (1/20)",
            rate
        );
    }

    // ── Strength penalty tests ──────────────────────────────────────

    #[test]
    fn test_hunger_strength_penalty_entering_weak() {
        assert_eq!(
            strength_penalty_change(HungerLevel::Hungry, HungerLevel::Weak),
            -1,
            "entering Weak should apply -1 strength"
        );
    }

    #[test]
    fn test_hunger_strength_penalty_recovering() {
        assert_eq!(
            strength_penalty_change(HungerLevel::Weak, HungerLevel::Hungry),
            1,
            "recovering from Weak should restore strength"
        );
    }

    #[test]
    fn test_hunger_strength_penalty_no_change() {
        assert_eq!(
            strength_penalty_change(HungerLevel::NotHungry, HungerLevel::Hungry),
            0,
            "NotHungry to Hungry should not change strength"
        );
        assert_eq!(
            strength_penalty_change(HungerLevel::Weak, HungerLevel::Fainting),
            0,
            "Weak to Fainting should not change strength (already weak)"
        );
    }

    #[test]
    fn test_hunger_strength_penalty_satiated_to_weak() {
        assert_eq!(
            strength_penalty_change(HungerLevel::Satiated, HungerLevel::Weak),
            -1,
            "Satiated to Weak should apply strength penalty"
        );
    }

    // ── Special food effects ────────────────────────────────────────

    #[test]
    fn test_hunger_special_wolfsbane() {
        let mut rng = test_rng();
        let effect = special_food_effect("sprig of wolfsbane", false, false, false, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::CureLycanthropy);
    }

    #[test]
    fn test_hunger_special_carrot() {
        let mut rng = test_rng();
        let effect = special_food_effect("carrot", false, false, false, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::CureBlindness);
    }

    #[test]
    fn test_hunger_special_fortune_cookie() {
        let mut rng = test_rng();
        let effect = special_food_effect("fortune cookie", false, false, false, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::FortuneCookie);
    }

    #[test]
    fn test_hunger_special_royal_jelly_uncursed() {
        let mut rng = test_rng();
        let effect = special_food_effect("lump of royal jelly", false, false, false, &mut rng);
        match effect {
            SpecialFoodEffect::RoyalJelly {
                hp_change,
                cure_wounded_legs,
                ..
            } => {
                assert!(hp_change >= 1 && hp_change <= 20,
                    "uncursed royal jelly hp_change {} should be 1..20", hp_change);
                assert!(cure_wounded_legs);
            }
            _ => panic!("should be RoyalJelly effect"),
        }
    }

    #[test]
    fn test_hunger_special_royal_jelly_cursed() {
        let mut rng = test_rng();
        let effect = special_food_effect("lump of royal jelly", true, false, false, &mut rng);
        match effect {
            SpecialFoodEffect::RoyalJelly { hp_change, .. } => {
                assert!(hp_change >= -20 && hp_change <= -1,
                    "cursed royal jelly hp_change {} should be -20..-1", hp_change);
            }
            _ => panic!("should be RoyalJelly effect"),
        }
    }

    #[test]
    fn test_hunger_special_eucalyptus_leaf_uncursed() {
        let mut rng = test_rng();
        let effect = special_food_effect("eucalyptus leaf", false, false, false, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::CureSickness);
    }

    #[test]
    fn test_hunger_special_eucalyptus_leaf_cursed() {
        let mut rng = test_rng();
        let effect = special_food_effect("eucalyptus leaf", true, false, false, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::None,
            "cursed eucalyptus leaf should have no effect");
    }

    #[test]
    fn test_hunger_special_cursed_apple_sleep() {
        let mut rng = test_rng();
        let effect = special_food_effect("apple", true, false, false, &mut rng);
        match effect {
            SpecialFoodEffect::FallAsleep { duration } => {
                assert!(duration >= 20 && duration <= 30,
                    "cursed apple sleep duration {} should be 20..30", duration);
            }
            _ => panic!("cursed apple should cause sleep"),
        }
    }

    #[test]
    fn test_hunger_special_cursed_apple_with_sleep_resist() {
        let mut rng = test_rng();
        let effect = special_food_effect("apple", true, false, true, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::None,
            "sleep-resistant hero should not fall asleep from cursed apple");
    }

    #[test]
    fn test_hunger_special_uncursed_apple_no_effect() {
        let mut rng = test_rng();
        let effect = special_food_effect("apple", false, false, false, &mut rng);
        assert_eq!(effect, SpecialFoodEffect::None,
            "uncursed apple should have no special effect");
    }

    // ── Nearly full warning ─────────────────────────────────────────

    #[test]
    fn test_hunger_nearly_full_warning() {
        assert!(should_warn_nearly_full(1500, false),
            "should warn at 1500");
        assert!(should_warn_nearly_full(1800, false),
            "should warn above 1500");
        assert!(!should_warn_nearly_full(1499, false),
            "should not warn below 1500");
        assert!(!should_warn_nearly_full(1500, true),
            "should not warn with Hunger property");
    }

    // ── Homemade tin nutrition cap ───────────────────────────────────

    #[test]
    fn test_hunger_tin_homemade_cap() {
        let mut rng = test_rng();
        // Homemade tin nutrition = min(50, monster_cnutrit).
        // Monster with cnutrit=30: should give 30.
        let n = tin_nutrition(TinVariety::Homemade, 30, false, false, &mut rng);
        assert_eq!(n, 30, "homemade tin should be capped at monster nutrition");

        // Monster with cnutrit=100: should give 50 (not 100).
        let n = tin_nutrition(TinVariety::Homemade, 100, false, false, &mut rng);
        assert_eq!(n, 50, "homemade tin should be capped at 50");
    }

    // ── Tin variety nutrition values ─────────────────────────────────

    #[test]
    fn test_hunger_tin_variety_nutrition() {
        let mut rng = test_rng();
        assert_eq!(tin_nutrition(TinVariety::Rotten, 100, false, false, &mut rng), -50);
        assert_eq!(tin_nutrition(TinVariety::Soup, 100, false, false, &mut rng), 20);
        assert_eq!(tin_nutrition(TinVariety::FrenchFried, 100, false, false, &mut rng), 40);
        assert_eq!(tin_nutrition(TinVariety::Pickled, 100, false, false, &mut rng), 40);
        assert_eq!(tin_nutrition(TinVariety::Boiled, 100, false, false, &mut rng), 50);
        assert_eq!(tin_nutrition(TinVariety::Smoked, 100, false, false, &mut rng), 50);
        assert_eq!(tin_nutrition(TinVariety::Dried, 100, false, false, &mut rng), 55);
        assert_eq!(tin_nutrition(TinVariety::DeepFried, 100, false, false, &mut rng), 60);
        assert_eq!(tin_nutrition(TinVariety::Szechuan, 100, false, false, &mut rng), 70);
        assert_eq!(tin_nutrition(TinVariety::Broiled, 100, false, false, &mut rng), 80);
        assert_eq!(tin_nutrition(TinVariety::StirFried, 100, false, false, &mut rng), 80);
        assert_eq!(tin_nutrition(TinVariety::Sauteed, 100, false, false, &mut rng), 95);
        assert_eq!(tin_nutrition(TinVariety::Candied, 100, false, false, &mut rng), 100);
        assert_eq!(tin_nutrition(TinVariety::Pureed, 100, false, false, &mut rng), 500);
    }

    // ── Greasy tin ──────────────────────────────────────────────────

    #[test]
    fn test_hunger_tin_greasy_varieties() {
        assert!(tin_is_greasy(TinVariety::FrenchFried));
        assert!(tin_is_greasy(TinVariety::DeepFried));
        assert!(tin_is_greasy(TinVariety::StirFried));
        assert!(!tin_is_greasy(TinVariety::Boiled));
        assert!(!tin_is_greasy(TinVariety::Rotten));
        assert!(!tin_is_greasy(TinVariety::Spinach));
    }

    // ── Corpse eating time formula ──────────────────────────────────

    #[test]
    fn test_hunger_corpse_eating_time_additional() {
        // Tiny monster (weight=3): 3 + 0 = 3.
        assert_eq!(eating_time_corpse(3), 3);
        // Medium (weight=128): 3 + 2 = 5.
        assert_eq!(eating_time_corpse(128), 5);
        // Heavy (weight=1500): 3 + 23 = 26.
        assert_eq!(eating_time_corpse(1500), 26);
    }

    // ── Glob eating time formula ────────────────────────────────────

    #[test]
    fn test_hunger_glob_eating_time() {
        assert_eq!(eating_time_glob(10), 3);
        assert_eq!(eating_time_glob(64), 4);
        assert_eq!(eating_time_glob(200), 6);
    }

    // ── Choking with Hunger property ────────────────────────────────

    #[test]
    fn test_hunger_choking_hunger_property_reduces_to_60() {
        let mut rng = test_rng();
        let outcome = check_choking(
            2500,  // nutrition
            true,  // canchoke
            false, // breathless
            true,  // has_hunger_property
            false, // strangled
            &mut rng,
        );
        match outcome {
            ChokeOutcome::Vomited { nutrition_lost } => {
                // With Hunger: reduces to 60. Lost = 2500 - 60 = 2440.
                assert_eq!(nutrition_lost, 2440,
                    "Hunger property should reduce nutrition to 60");
            }
            _ => panic!("Hunger property should guarantee survival"),
        }
    }

    // ── Choking while strangled ─────────────────────────────────────

    #[test]
    fn test_hunger_choking_strangled_always_fatal() {
        let mut rng = test_rng();
        // Strangled blocks the rn2(20) escape route.
        let mut died = 0;
        for _ in 0..1000 {
            let outcome = check_choking(
                CHOKING_THRESHOLD,
                true,
                false, // not breathless
                false, // no hunger property
                true,  // strangled
                &mut rng,
            );
            if outcome == ChokeOutcome::Choked {
                died += 1;
            }
        }
        assert_eq!(died, 1000, "strangled hero should always choke to death");
    }

    // ── Conduct classification ──────────────────────────────────────

    #[test]
    fn test_hunger_conduct_wax_is_vegetarian() {
        let class = classify_food_conduct(Material::Wax, false, None);
        assert_eq!(class, FoodConductClass::Vegetarian,
            "wax items should be vegetarian (not vegan)");
    }

    #[test]
    fn test_hunger_conduct_leather_is_meat() {
        let class = classify_food_conduct(Material::Leather, false, None);
        assert_eq!(class, FoodConductClass::Meat);
    }

    #[test]
    fn test_hunger_conduct_bone_is_meat() {
        let class = classify_food_conduct(Material::Bone, false, None);
        assert_eq!(class, FoodConductClass::Meat);
    }

    #[test]
    fn test_hunger_conduct_dragonhide_is_meat() {
        let class = classify_food_conduct(Material::DragonHide, false, None);
        assert_eq!(class, FoodConductClass::Meat);
    }

    // ── Rounddiv helper ─────────────────────────────────────────────

    #[test]
    fn test_hunger_rounddiv() {
        // rounddiv(5, 3) = 5/3=1 rem 2, 2*2=4 >= 3 -> 2.
        assert_eq!(rounddiv(5, 3), 2);
        // rounddiv(4, 3) = 4/3=1 rem 1, 2*1=2 < 3 -> 1.
        assert_eq!(rounddiv(4, 3), 1);
        // rounddiv(6, 3) = 6/3=2 rem 0, 2*0=0 < 3 -> 2.
        assert_eq!(rounddiv(6, 3), 2);
        // rounddiv(7, 2) = 7/2=3 rem 1, 2*1=2 >= 2 -> 4.
        assert_eq!(rounddiv(7, 2), 4);
        // rounddiv(0, 5) = 0.
        assert_eq!(rounddiv(0, 5), 0);
    }

    // ── Starvation death threshold exhaustive ───────────────────────

    #[test]
    fn test_hunger_starvation_threshold_range() {
        // Verify for all valid Constitution values (3..25).
        for con in 3..=25u8 {
            let threshold = starvation_threshold(con);
            let expected = -(100 + 10 * con as i32);
            assert_eq!(threshold, expected, "con={} threshold wrong", con);
            // At threshold: alive (strict <).
            assert!(!should_starve(threshold, con), "con={} at threshold should survive", con);
            // One below: dead.
            assert!(should_starve(threshold - 1, con), "con={} below threshold should die", con);
        }
    }

    // ── Eating time normal food ─────────────────────────────────────

    #[test]
    fn test_hunger_eating_time_normal_minimum() {
        // oc_delay=0 should be clamped to 1.
        assert_eq!(eating_time_normal(0), 1);
        assert_eq!(eating_time_normal(1), 1);
        assert_eq!(eating_time_normal(5), 5);
        assert_eq!(eating_time_normal(20), 20);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Corpse intrinsic by name lookup tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_corpse_intrinsic_fire_resistance() {
        let result = corpse_intrinsic_by_name("red dragon");
        assert_eq!(result, Some((CorpseIntrinsic::FireResistance, 33)));
        let result = corpse_intrinsic_by_name("fire ant");
        assert_eq!(result, Some((CorpseIntrinsic::FireResistance, 33)));
    }

    #[test]
    fn test_corpse_intrinsic_cold_resistance() {
        let result = corpse_intrinsic_by_name("white dragon");
        assert_eq!(result, Some((CorpseIntrinsic::ColdResistance, 33)));
        let result = corpse_intrinsic_by_name("winter wolf");
        assert_eq!(result, Some((CorpseIntrinsic::ColdResistance, 33)));
    }

    #[test]
    fn test_corpse_intrinsic_poison_resistance() {
        let result = corpse_intrinsic_by_name("killer bee");
        assert_eq!(result, Some((CorpseIntrinsic::PoisonResistance, 33)));
        let result = corpse_intrinsic_by_name("scorpion");
        assert_eq!(result, Some((CorpseIntrinsic::PoisonResistance, 33)));
    }

    #[test]
    fn test_corpse_intrinsic_telepathy() {
        let result = corpse_intrinsic_by_name("floating eye");
        assert_eq!(result, Some((CorpseIntrinsic::Telepathy, 100)));
    }

    #[test]
    fn test_corpse_intrinsic_teleportitis() {
        let result = corpse_intrinsic_by_name("leprechaun");
        assert_eq!(result, Some((CorpseIntrinsic::Teleportitis, 33)));
    }

    #[test]
    fn test_corpse_intrinsic_teleport_control() {
        let result = corpse_intrinsic_by_name("tengu");
        assert_eq!(result, Some((CorpseIntrinsic::TeleportControl, 33)));
    }

    #[test]
    fn test_corpse_intrinsic_see_invisible() {
        let result = corpse_intrinsic_by_name("stalker");
        assert_eq!(result, Some((CorpseIntrinsic::SeeInvisible, 33)));
    }

    #[test]
    fn test_corpse_intrinsic_strength() {
        let result = corpse_intrinsic_by_name("hill giant");
        assert_eq!(result, Some((CorpseIntrinsic::Strength, 50)));
        let result = corpse_intrinsic_by_name("titan");
        assert_eq!(result, Some((CorpseIntrinsic::Strength, 50)));
    }

    #[test]
    fn test_corpse_intrinsic_unknown_monster() {
        let result = corpse_intrinsic_by_name("newt");
        assert_eq!(result, None, "newt should not confer any intrinsic");
    }

    // ═══════════════════════════════════════════════════════════════════
    // eat_corpse_effects tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_eat_corpse_effects_cockatrice_stoning() {
        let mut rng = test_rng();
        let mut corpse = basic_corpse("cockatrice", 5, 100, 100, ResistanceSet::empty());
        corpse.flesh_petrifies = true;

        let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
        assert_eq!(
            effects.negative,
            Some(CorpseNegativeEffect::Stoning),
            "cockatrice corpse should cause stoning"
        );
        // No intrinsic gained when stoned to death.
        assert!(effects.intrinsic.is_none());
    }

    #[test]
    fn test_eat_corpse_effects_cockatrice_with_stone_res() {
        let mut rng = test_rng();
        let mut corpse = basic_corpse("cockatrice", 5, 100, 100, ResistanceSet::empty());
        corpse.flesh_petrifies = true;

        let effects = eat_corpse_effects(&corpse, false, true, &mut rng);
        assert!(
            effects.negative != Some(CorpseNegativeEffect::Stoning),
            "stone resistance should prevent stoning"
        );
    }

    #[test]
    fn test_eat_corpse_effects_poisonous_with_resistance() {
        let mut rng = test_rng();
        let mut corpse = basic_corpse("kobold", 1, 50, 50, ResistanceSet::empty());
        corpse.poisonous = true;

        // With poison resistance, should never get Poisoned effect.
        for _ in 0..100 {
            let effects = eat_corpse_effects(&corpse, true, false, &mut rng);
            match effects.negative {
                Some(CorpseNegativeEffect::Poisoned { .. }) => {
                    panic!("should not be poisoned with resistance")
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_eat_corpse_effects_poisonous_without_resistance() {
        let mut rng = test_rng();
        let mut corpse = basic_corpse("kobold", 1, 50, 50, ResistanceSet::empty());
        corpse.poisonous = true;

        let mut got_poisoned = false;
        for _ in 0..100 {
            let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
            if let Some(CorpseNegativeEffect::Poisoned { hp_damage, str_loss }) = effects.negative {
                assert!(hp_damage >= 1 && hp_damage <= 15);
                assert!(str_loss >= 1 && str_loss <= 4);
                got_poisoned = true;
            }
        }
        assert!(got_poisoned, "should sometimes be poisoned without resistance");
    }

    #[test]
    fn test_eat_corpse_effects_domestic_aggravation() {
        let mut rng = test_rng();
        let mut corpse = basic_corpse("dog", 4, 150, 200, ResistanceSet::empty());
        corpse.is_domestic = true;

        let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
        assert_eq!(
            effects.negative,
            Some(CorpseNegativeEffect::Aggravation),
            "eating domestic animal should cause aggravation"
        );
    }

    #[test]
    fn test_eat_corpse_effects_wraith_gain_level() {
        let mut rng = test_rng();
        let corpse = basic_corpse("wraith", 6, 100, 0, ResistanceSet::empty());

        let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
        assert_eq!(effects.post_effect, CorpsePostEffect::GainLevel);
    }

    #[test]
    fn test_eat_corpse_effects_quantum_mechanic_speed() {
        let mut rng = test_rng();
        let corpse = basic_corpse("quantum mechanic", 7, 100, 20, ResistanceSet::empty());

        let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
        assert!(effects.speed_toggle, "quantum mechanic should toggle speed");
        assert_eq!(effects.post_effect, CorpsePostEffect::ToggleSpeed);
    }

    #[test]
    fn test_eat_corpse_effects_lizard_cure() {
        let mut rng = test_rng();
        let corpse = basic_corpse("lizard", 5, 100, 40, ResistanceSet::empty());

        let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
        assert_eq!(effects.post_effect, CorpsePostEffect::LizardCure);
    }

    #[test]
    fn test_eat_corpse_effects_werewolf_lycanthropy() {
        let mut rng = test_rng();
        let corpse = basic_corpse("werewolf", 5, 100, 250, ResistanceSet::empty());

        let effects = eat_corpse_effects(&corpse, false, false, &mut rng);
        assert_eq!(effects.post_effect, CorpsePostEffect::Lycanthropy);
    }

    // ═══════════════════════════════════════════════════════════════════
    // Tin variety tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_random_tin_variety_distribution() {
        let mut rng = test_rng();
        let mut counts = [0u32; 16];
        let trials = 15000;
        for _ in 0..trials {
            let v = random_tin_variety(&mut rng);
            counts[v as usize] += 1;
        }
        // Each variety should appear (15 varieties, each with ~1/15 chance).
        for i in 0..15 {
            assert!(
                counts[i] > 0,
                "variety {} should appear at least once in {} trials",
                i,
                trials
            );
        }
        // Spinach (index 15) should never appear from random_tin_variety.
        assert_eq!(counts[15], 0, "spinach should not appear randomly");
    }

    #[test]
    fn test_tin_description_rotten() {
        let desc = tin_description(TinVariety::Rotten, "kobold");
        assert_eq!(desc, "It smells terrible!");
    }

    #[test]
    fn test_tin_description_homemade() {
        let desc = tin_description(TinVariety::Homemade, "newt");
        assert_eq!(desc, "It smells like newt soup.");
    }

    #[test]
    fn test_tin_description_spinach() {
        let desc = tin_description(TinVariety::Spinach, "anything");
        assert_eq!(desc, "This makes you feel like Strongo!");
    }

    #[test]
    fn test_tin_description_french_fried() {
        let desc = tin_description(TinVariety::FrenchFried, "lichen");
        assert_eq!(desc, "It is French fried lichen meat.");
    }

    // ═══════════════════════════════════════════════════════════════════
    // Eating edge case tests
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_is_cannibalism_human() {
        assert!(is_cannibalism("human", "human"));
        assert!(is_cannibalism("human", "shopkeeper"));
        assert!(is_cannibalism("human", "guard"));
        assert!(is_cannibalism("human", "wizard"));
        assert!(!is_cannibalism("human", "gnome"));
    }

    #[test]
    fn test_is_cannibalism_elf() {
        assert!(is_cannibalism("elf", "elf"));
        assert!(is_cannibalism("elf", "Woodland-elf"));
        assert!(is_cannibalism("elf", "Green-elf"));
        assert!(!is_cannibalism("elf", "human"));
    }

    #[test]
    fn test_is_cannibalism_dwarf() {
        assert!(is_cannibalism("dwarf", "dwarf lord"));
        assert!(is_cannibalism("dwarf", "dwarf king"));
        assert!(!is_cannibalism("dwarf", "elf"));
    }

    #[test]
    fn test_is_cannibalism_gnome() {
        assert!(is_cannibalism("gnome", "gnome"));
        assert!(is_cannibalism("gnome", "gnome lord"));
        assert!(!is_cannibalism("gnome", "dwarf"));
    }

    #[test]
    fn test_is_cannibalism_orc() {
        assert!(is_cannibalism("orc", "orc"));
        assert!(is_cannibalism("orc", "orc captain"));
        assert!(!is_cannibalism("orc", "human"));
    }

    #[test]
    fn test_cannibalism_effects_first_time() {
        let effects = cannibalism_effects(true);
        assert_eq!(effects.len(), 3);
        assert_eq!(effects[0], CannibalismEffect::AlignmentPenalty(-15));
        assert!(matches!(&effects[1], CannibalismEffect::Message(s) if s.contains("cannibal")));
        assert_eq!(effects[2], CannibalismEffect::AggravateMonsters);
    }

    #[test]
    fn test_cannibalism_effects_repeat() {
        let effects = cannibalism_effects(false);
        assert_eq!(effects.len(), 2);
        assert!(!effects.iter().any(|e| matches!(e, CannibalismEffect::AggravateMonsters)));
    }

    #[test]
    fn test_violates_conduct_meat() {
        assert_eq!(violates_conduct("corpse"), ConductViolation::Vegetarian);
        assert_eq!(violates_conduct("tin"), ConductViolation::Vegetarian);
        assert_eq!(violates_conduct("meat stick"), ConductViolation::Vegetarian);
        assert_eq!(violates_conduct("huge chunk of meat"), ConductViolation::Vegetarian);
    }

    #[test]
    fn test_violates_conduct_dairy() {
        assert_eq!(violates_conduct("egg"), ConductViolation::Vegan);
        assert_eq!(violates_conduct("cream pie"), ConductViolation::Vegan);
        assert_eq!(violates_conduct("candy bar"), ConductViolation::Vegan);
        assert_eq!(violates_conduct("lump of royal jelly"), ConductViolation::Vegan);
    }

    #[test]
    fn test_violates_conduct_plant() {
        assert_eq!(violates_conduct("food ration"), ConductViolation::None);
        assert_eq!(violates_conduct("apple"), ConductViolation::None);
        assert_eq!(violates_conduct("kelp frond"), ConductViolation::None);
    }

    #[test]
    fn test_eat_amulet_effects() {
        assert_eq!(eat_amulet_effect("amulet of strangulation"), AmuletEatEffect::Choke);
        assert_eq!(eat_amulet_effect("amulet of unchanging"), AmuletEatEffect::Unchanging);
        assert_eq!(eat_amulet_effect("amulet of life saving"), AmuletEatEffect::LifeSaving);
        assert_eq!(
            eat_amulet_effect("cheap plastic imitation of the Amulet of Yendor"),
            AmuletEatEffect::Plastic
        );
        assert_eq!(eat_amulet_effect("Amulet of Yendor"), AmuletEatEffect::RealAmulet);
        assert_eq!(eat_amulet_effect("amulet of reflection"), AmuletEatEffect::Nutrition(20));
    }

    #[test]
    fn test_eat_ring_intrinsics() {
        assert_eq!(
            eat_ring_effect("ring of fire resistance"),
            RingEatEffect::GainIntrinsic("fire_resistance")
        );
        assert_eq!(
            eat_ring_effect("ring of teleportation"),
            RingEatEffect::GainIntrinsic("teleportitis")
        );
        assert_eq!(
            eat_ring_effect("ring of polymorph control"),
            RingEatEffect::GainIntrinsic("polymorph_control")
        );
        assert_eq!(
            eat_ring_effect("ring of slow digestion"),
            RingEatEffect::GainIntrinsic("slow_digestion")
        );
        // Unknown ring gives nutrition
        assert_eq!(eat_ring_effect("ring of adornment"), RingEatEffect::Nutrition(15));
    }

    #[test]
    fn test_lizard_corpse_effect_events() {
        let events = lizard_corpse_effect();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_can_cure_stoning() {
        assert!(can_cure_stoning("lizard corpse"));
        assert!(can_cure_stoning("acid"));
        assert!(can_cure_stoning("potion of acid"));
        assert!(!can_cure_stoning("food ration"));
        assert!(!can_cure_stoning("apple"));
    }
}
