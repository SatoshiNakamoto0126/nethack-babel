# Test Oracle Documentation

## Purpose
Define how to verify Rust Babel's behavior against C NetHack without access to the C test suite.

## Strategy A: Monte Carlo Statistical Tests
For probability-dependent mechanics (combat hit rates, trap chances, loot drops):
1. Run the Rust function 10,000+ times with varied RNG seeds
2. Compare mean/stddev against the C formula's expected distribution
3. Assert within 2 sigma (95% confidence) or 3 sigma (99.7% confidence)

Example:
```rust
#[test]
fn test_mc_combat_hit_rate() {
    let mut hits = 0;
    for seed in 0..10_000 {
        let mut rng = StdRng::seed_from_u64(seed);
        if roll_to_hit(player_thac0: 15, target_ac: 5, &mut rng) {
            hits += 1;
        }
    }
    let rate = hits as f64 / 10_000.0;
    // C formula: need to roll (20 - THAC0 + AC) or higher on d20
    // Expected: 50% hit rate for THAC0=15, AC=5
    assert!((rate - 0.50).abs() < 0.03, "Hit rate {rate} outside 3s of 0.50");
}
```

## Strategy B: Mock RNG Deterministic Tests
For state machines (polymorph->revert, stoning countdown, hunger stages):
1. Use `StdRng::seed_from_u64(FIXED_SEED)` for deterministic RNG
2. Step through every state transition
3. Assert exact state at each step

Example:
```rust
#[test]
fn test_det_hunger_stage_transitions() {
    let mut rng = StdRng::seed_from_u64(42);
    let mut nutrition = 900; // starts SATIATED
    assert_eq!(hunger_stage(nutrition), HungerStage::Satiated);
    nutrition = 150;
    assert_eq!(hunger_stage(nutrition), HungerStage::Hungry);
    nutrition = 50;
    assert_eq!(hunger_stage(nutrition), HungerStage::Weak);
    nutrition = 0;
    assert_eq!(hunger_stage(nutrition), HungerStage::Fainting);
}
```

## Strategy C: Golden Master (Future)
1. Compile C NetHack in wizard mode
2. Script a sequence of actions (#genesis, #wish, movement)
3. Capture output/state as JSON
4. Assert Rust produces identical state for identical inputs
5. Store golden master files in `tests/golden/`

## Strategy D: Wiki-Extracted Oracles
1. Extract factual statements from NetHack wiki
2. Convert to test assertions
3. Example: "A blessed potion of full healing restores all HP and increases max HP by 1-8"
   -> Test that blessed full healing sets HP=maxHP and maxHP increases

Example:
```rust
#[test]
fn test_wiki_potion_full_healing_blessed() {
    let mut world = TestWorld::new(42);
    let player = world.spawn_player(hp: 20, max_hp: 30);
    let potion = world.spawn_item(ObjectType::PotionOfFullHealing, Buc::Blessed);
    world.quaff(player, potion);
    let hp = world.get::<HitPoints>(player);
    assert_eq!(hp.current, hp.max);
    assert!(hp.max >= 31 && hp.max <= 38, "Blessed full healing adds 1-8 max HP");
}
```

## Test Naming Convention
- `test_mc_<system>_<property>` -- Monte Carlo tests
- `test_det_<system>_<scenario>` -- Deterministic mock-RNG tests
- `test_gm_<system>_<scenario>` -- Golden master tests
- `test_wiki_<system>_<fact>` -- Wiki-extracted tests
