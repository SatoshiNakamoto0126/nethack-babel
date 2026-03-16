## NetHack Babel — English message catalog
## Fluent (.ftl) format — https://projectfluent.org/

## ============================================================================
## Combat — melee
## ============================================================================

melee-hit-bare = { $attacker } hits { $defender }!
melee-hit-weapon = { $attacker } hits { $defender } with { $weapon }!
melee-hit-slash = { $attacker } slashes { $defender }!
melee-hit-stab = { $attacker } stabs { $defender }!
melee-hit-bash = { $attacker } bashes { $defender }!
melee-hit-whip = { $attacker } lashes { $defender }!
melee-hit-bite = { $attacker } bites { $defender }!
melee-hit-claw = { $attacker } claws { $defender }!
melee-hit-sting = { $attacker } stings { $defender }!
melee-hit-butt = { $attacker } butts { $defender }!
melee-hit-kick = { $attacker } kicks { $defender }!
melee-miss = { $attacker } misses { $defender }.
melee-miss-barely = { $attacker } just barely misses { $defender }.
critical-hit = Critical hit!
backstab = { $attacker } backstabs { $defender }!
joust-hit = { $attacker } jousts { $defender }!
joust-lance-breaks = Your lance shatters from the impact!
attack-blocked = { $defender } blocks the attack.
attack-parried = { $defender } parries the attack.

## ============================================================================
## Combat — ranged
## ============================================================================

ranged-hit = { $attacker } hits { $defender } with { $projectile }!
ranged-miss = { $projectile } misses { $defender }.
ranged-miss-wide = { $projectile } flies wide of { $defender }.
throw-hit = { $projectile } hits { $defender }!
throw-miss = { $projectile } misses { $defender } and falls to the ground.
spell-hit = The { $spell } hits { $defender }!
spell-miss = The { $spell } misses { $defender }.
spell-fizzle = The spell fizzles out.
wand-zap = You zap { $wand }!
wand-nothing = Nothing happens.
wand-wrested = You wrest one last charge from { $wand }.

## ============================================================================
## Combat — damage descriptions
## ============================================================================

damage-barely = The attack barely scratches { $defender }.
damage-light = { $defender } is lightly wounded.
damage-moderate = { $defender } is moderately wounded.
damage-heavy = { $defender } is heavily wounded.
damage-severe = { $defender } is severely wounded.
damage-critical = { $defender } is critically wounded!

## ============================================================================
## Combat — passive damage
## ============================================================================

passive-acid = The acid burns!
passive-fire = You get burned!
passive-cold = You get frozen!
passive-shock = You get zapped!
passive-poison = You feel sick!
passive-drain = You feel drained!
passive-corrode = Your { $item } corrodes!
passive-stun = You stagger from the blow!
passive-slow = You feel yourself slow down!
passive-paralyze = You are paralyzed!

## ============================================================================
## Combat — death messages
## ============================================================================

entity-killed = { $entity } is killed!
entity-destroyed = { $entity } is destroyed!
entity-dissolved = { $entity } dissolves!
entity-evaporates = { $entity } evaporates!
entity-turns-to-dust = { $entity } turns to dust!
you = You
you-hit-monster = You hit the { $monster }!
you-miss-monster = You miss the { $monster }.
you-kill-monster = You kill the { $monster }!
you-destroy-monster = You destroy the { $monster }!
you-dissolve-monster = The { $monster } dissolves!
monster-hits-you = The { $monster } hits you!
monster-misses-you = The { $monster } misses you.
monster-kills-you = The { $monster } kills you!
monster-turns-to-stone = The { $monster } turns to stone!
monster-flees = The { $monster } turns to flee!
monster-falls-asleep = The { $monster } falls asleep.

## ============================================================================
## Movement — doors
## ============================================================================

door-opened = The door opens.
door-closed = The door closes.
door-locked = This door is locked.
door-broken = You break open the door!
door-unlock = You unlock the door.
door-lock = You lock the door.
door-kick = You kick the door!
door-kick-fail = WHAMMM!!! The door holds.
door-resist = The door resists!
door-jammed = The door is stuck.

## ============================================================================
## Movement — collision and terrain
## ============================================================================

bump-wall = You bump into a wall.
bump-boulder = You push the boulder.
bump-boulder-fail = The boulder doesn't budge.
bump-closed-door = Ouch! You walk into a closed door.
bump-monster = You bump into { $monster }.
swim-lava = You are swimming in molten lava!
swim-water = You fall into the water!
swim-sink = You sink beneath the surface!
terrain-ice = The ice is slippery!
terrain-ice-slip = You slip on the ice!
terrain-mud = You get stuck in the mud.

## ============================================================================
## Movement — traps
## ============================================================================

trap-triggered = You trigger a { $trap }!
trap-disarmed = You disarm the { $trap }.
trap-pit-fall = You fall into a pit!
trap-spiked-pit = You fall into a pit of spikes!
trap-arrow = An arrow shoots out at you!
trap-dart = A little dart shoots out at you!
trap-bear = You step on a bear trap!
trap-teleport = You feel a wrenching sensation!
trap-level-teleport = You feel a sudden shift!
trap-magic-portal = You feel dizzy for a moment.
trap-fire = A tower of flame erupts!
trap-rolling-boulder = A boulder rolls toward you!
trap-squeaky-board = A board beneath you squeaks loudly.
trap-web = You are caught in a web!
trap-rust = A gush of water hits you!
trap-polymorph = You feel a change coming over you!

## ============================================================================
## Movement — stairs and level changes
## ============================================================================

stairs-up = You climb the stairs.
stairs-down = You descend the stairs.
stairs-nothing-up = You can't go up here.
stairs-nothing-down = You can't go down here.
level-change = You enter { $level }.
level-feeling = You have { $feeling ->
    [good] a good feeling
    [bad] a bad feeling
   *[neutral] an uncertain feeling
    } about this level.
level-feeling-objects = You sense valuable objects on this level.
level-enter-shop = You enter { $shopkeeper }'s { $shoptype }.
level-leave-shop = You leave the shop.
elbereth-engrave = You engrave "Elbereth" on the ground.
elbereth-warn = The { $monster } looks frightened!
elbereth-fade = The engraving fades.

## ============================================================================
## Movement — environment interactions
## ============================================================================

fountain-drink = You drink from the fountain.
fountain-dry = The fountain dries up!
fountain-shimmer = You see a shimmering pool.
sink-drink = You drink from the sink.
sink-ring = You hear a ring bouncing down the drainpipe.
altar-pray = You begin praying to { $god }.
altar-sacrifice = You offer the { $corpse } to { $god }.
altar-desecrate = You feel a dark presence.
throne-sit = You sit on the throne.
grave-dig = You dig up the grave.
grave-disturb = You disturb the remains of { $entity }.

## ============================================================================
## Status — hunger
## ============================================================================

hunger-satiated = You are satiated.
hunger-not-hungry = You are not hungry.
hunger-hungry = You are hungry.
hunger-weak = You feel weak.
hunger-fainting = You faint from lack of food.
hunger-starved = You die from starvation.

## ============================================================================
## Status — HP and level
## ============================================================================

hp-gained = You feel { $amount ->
    [one] a little
   *[other] much
    } better.
hp-lost = You feel { $amount ->
    [one] a little
   *[other] much
    } worse.
hp-full = You feel completely healed.
hp-restored = Your health is restored.
level-up = Welcome to experience level { $level }!
level-down = You feel less experienced.
level-max = You feel all-powerful!

## ============================================================================
## Status — properties gained/lost
## ============================================================================

speed-gain = You feel fast!
speed-lose = You slow down.
speed-very-fast = You feel very fast!
strength-gain = You feel strong!
strength-lose = You feel weak!
telepathy-gain = You feel a strange mental acuity.
telepathy-lose = Your senses return to normal.
invisibility-gain = You feel transparent!
invisibility-lose = You become visible again.
see-invisible-gain = You feel perceptive!
see-invisible-lose = You feel less perceptive.
stealth-gain = You feel stealthy!
stealth-lose = You feel clumsy.
fire-resist-gain = You feel a chill.
fire-resist-lose = You feel warmer.
cold-resist-gain = You feel warm.
cold-resist-lose = You feel cold.
shock-resist-gain = You feel insulated.
shock-resist-lose = You feel conductive.
poison-resist-gain = You feel healthy.
poison-resist-lose = You feel less healthy.

## ============================================================================
## Status — conditions
## ============================================================================

poison-affected = You feel { $severity ->
    [mild] slightly ill
    [moderate] ill
   *[severe] very ill
    }.
confusion-start = You feel confused.
confusion-end = You feel less confused now.
blindness-start = You can't see!
blindness-end = You can see again.
stun-start = You stagger...
stun-end = You feel steadier now.
hallucination-start = Oh wow! Everything looks so cosmic!
hallucination-end = You are back to normal.
sleep-start = You feel drowsy...
sleep-end = You wake up.
petrification-start = You are turning to stone!
petrification-cure = You feel more limber.
lycanthropy-start = You feel feverish.
lycanthropy-cure = You feel better.
levitation-start = You float up!
levitation-end = You gently descend.

## ============================================================================
## Status — encumbrance
## ============================================================================

encumbrance-unencumbered = Your movements are unencumbered.
encumbrance-burdened = You are burdened.
encumbrance-stressed = You are stressed.
encumbrance-strained = You are strained!
encumbrance-overtaxed = You are overtaxed!
encumbrance-overloaded = You are overloaded and cannot move!

## ============================================================================
## Items — pickup and drop
## ============================================================================

item-picked-up = { $actor } picks up { $item }.
item-dropped = { $actor } drops { $item }.
you-pick-up = You pick up { $item }.
you-drop = You drop { $item }.
you-pick-up-gold = You pick up { $amount } gold piece{ $amount ->
    [one] {""}
   *[other] s
    }.
nothing-to-pick-up = There is nothing here to pick up.
too-many-items = You have too many items!

## ============================================================================
## Items — wielding weapons
## ============================================================================

item-wielded = { $actor } wields { $item }.
you-wield = You wield { $weapon }.
    .two-handed = (weapon in both hands)
you-wield-already = You are already wielding that!
you-unwield = You put away { $weapon }.
you-wield-nothing = You are empty-handed.
weapon-weld-cursed = { $weapon } welds itself to your hand!

## ============================================================================
## Items — armor
## ============================================================================

item-worn = { $actor } wears { $item }.
item-removed = { $actor } takes off { $item }.
you-wear = You put on { $armor }.
you-remove = You take off { $armor }.
you-remove-cursed = You can't take off { $armor }. It is cursed!
armor-crumbles = Your { $armor } crumbles to dust!

## ============================================================================
## Items — identification and status
## ============================================================================

item-identified = You now know that { $item } is { $identity }.
item-damaged = { $item } is damaged!
item-destroyed = { $item } is destroyed!
item-cursed = { $item } is cursed!
item-blessed = { $item } is blessed.
item-enchanted = { $item } glows { $color } for a moment.
item-rusted = { $item } is rusted.
item-burnt = { $item } is burnt!
item-rotted = { $item } is rotted.
item-corroded = { $item } is corroded.
item-eroded-away = { $item } erodes away completely!

## ============================================================================
## Items — food and eating
## ============================================================================

eat-start = You begin eating { $food }.
eat-finish = You finish eating { $food }.
eat-delicious = Delicious!
eat-disgusting = Blecch! That tasted terrible!
eat-poisoned = Ecch - that must have been poisoned!
eat-rotten = Ulch - that food was rotten!
eat-cannibal = You cannibal! You feel deathly ill.
eat-corpse-old = This { $monster } corpse tastes stale.

## ============================================================================
## Items — potions and scrolls
## ============================================================================

potion-drink = You drink { $potion }.
potion-shatter = { $potion } shatters!
potion-boil = { $potion } boils and evaporates.
potion-freeze = { $potion } freezes and shatters!
scroll-read = As you read the scroll, it disappears.
scroll-blank = This scroll seems to be blank.
scroll-cant-read = You can't read while blind!
spellbook-read = You begin to study { $spellbook }.
spellbook-learn = You learn the spell of { $spell }!
spellbook-forget = You have forgotten the spell of { $spell }.
spellbook-too-hard = This spellbook is too difficult for you.

## ============================================================================
## Items — gold
## ============================================================================

gold-pick-up = You pick up { $amount } gold piece{ $amount ->
    [one] {""}
   *[other] s
    }.
gold-drop = You drop { $amount } gold piece{ $amount ->
    [one] {""}
   *[other] s
    }.
gold-paid = You pay { $amount } gold piece{ $amount ->
    [one] {""}
   *[other] s
    }.
gold-received = You receive { $amount } gold piece{ $amount ->
    [one] {""}
   *[other] s
    }.

## ============================================================================
## Items — containers
## ============================================================================

container-open = You open { $container }.
container-close = You close { $container }.
container-empty = { $container } is empty.
container-locked = { $container } is locked.
container-trap = You set off a trap on { $container }!
container-looted = You loot { $container }.

## ============================================================================
## Monster — actions
## ============================================================================

monster-moves = The { $monster } moves.
monster-picks-up = The { $monster } picks up { $item }.
monster-wields = The { $monster } wields { $item }!
monster-wears = The { $monster } puts on { $item }.
monster-eats = The { $monster } eats { $food }.
monster-drinks = The { $monster } drinks { $potion }!
monster-casts = The { $monster } casts a spell!
monster-breathes = The { $monster } breathes { $element }!
monster-summons = The { $monster } summons help!
monster-steals = The { $monster } steals { $item }!
monster-grabs = The { $monster } grabs you!
monster-throws = The { $monster } throws { $item }!
monster-zaps = The { $monster } zaps { $wand }!

## ============================================================================
## Monster — sounds
## ============================================================================

sound-growl = You hear a low growl.
sound-roar = You hear a roar!
sound-hiss = You hear a hiss!
sound-buzz = You hear a buzzing sound.
sound-chug = You hear a chugging sound.
sound-splash = You hear a splash.
sound-clank = You hear a clanking sound.
sound-scream = You hear a scream!
sound-squeak = You hear a squeak.
sound-laugh = You hear maniacal laughter!
sound-wail = You hear a wailing sound.
sound-whisper = You hear whispering.
sound-coins = You hear the chime of coins.
sound-footsteps = You hear footsteps.
sound-digging = You hear digging.

## ============================================================================
## Monster — pet messages
## ============================================================================

pet-eats = Your { $pet } eats the { $food }.
pet-drops = Your { $pet } drops { $item }.
pet-picks-up = Your { $pet } picks up { $item }.
pet-whimper = Your { $pet } whimpers.
pet-happy = Your { $pet } looks happy.
pet-wag = Your { $pet } wags its tail.
pet-hostile = Your { $pet } has gone feral!
pet-tame = You tame the { $monster }.
pet-name = What do you want to call your { $pet }?

## ============================================================================
## UI — prompts
## ============================================================================

more-prompt = --More--
quit-prompt = Are you sure you want to quit?
really-quit = Really quit?
prompt-direction = In what direction?
prompt-eat = What do you want to eat?
prompt-drink = What do you want to drink?
prompt-read = What do you want to read?
prompt-zap = What do you want to zap?
prompt-throw = What do you want to throw?
prompt-name = What do you want to name?
prompt-call = Call it:
prompt-confirm = Are you sure? [yn]
prompt-pay = Pay { $amount } for { $item }?

## ============================================================================
## UI — game over and scoring
## ============================================================================

game-over = You died. Score: { $score }.
game-over-escaped = You escaped the dungeon!
game-over-ascended = You ascended to demigod status!
game-over-quit = You quit.
game-over-possessions = Do you want your possessions identified?
game-over-topten = You made the top ten!
game-over-score-final = Final score: { $score }.
game-over-turns = You lasted { $turns } turn{ $turns ->
    [one] {""}
   *[other] s
    }.
game-over-killer = Killed by { $killer }.
game-over-epitaph = Rest in peace, { $name }.

## ============================================================================
## UI — welcome and status
## ============================================================================

welcome = Welcome to NetHack Babel, { $name } the { $role }!
welcome-back = Welcome back to NetHack Babel, { $name } the { $role }!
character-description = { $name } the { $race } { $role }
dungeon-level = Dungeon level { $depth }
status-line = HP:{ $hp }/{ $maxhp } Pw:{ $pw }/{ $maxpw } AC:{ $ac } Xp:{ $level }

## ============================================================================
## UI — help
## ============================================================================

help-title = NetHack Babel Help
help-move = Move with hjklyubn or arrow keys.
help-attack = Walk into a monster to attack it.
help-wait = Press . or s to wait one turn.
help-search = Press s to search for hidden things.
help-inventory = Press i to view your inventory.
help-pickup = Press , to pick up items.
help-drop = Press d to drop items.
help-stairs-up = Press < to go upstairs.
help-stairs-down = Press > to go downstairs.
help-eat = Press e to eat.
help-quaff = Press q to quaff (drink) a potion.
help-read = Press r to read a scroll or spellbook.
help-wield = Press w to wield a weapon.
help-wear = Press W to wear armor.
help-remove = Press T to take off armor.
help-zap = Press z to zap a wand.

# Help — movement diagram
help-move-diagram =
    {"  "}y k u     NW  N  NE
    {"  "}h . l      W  .   E
    {"  "}b j n     SW  S  SE

# Help — symbols
help-symbols-title = Symbols:
help-symbol-player = @  = you (the player)
help-symbol-floor = .  = floor
help-symbol-corridor = #  = corridor
help-symbol-door-closed = +  = closed door
help-symbol-door-open = |  = open door
help-symbol-stairs-up = <  = stairs up
help-symbol-stairs-down = >  = stairs down
help-symbol-water = {"}"}  = water/lava
help-symbol-fountain = {"{"} = fountain

# Help — additional commands
help-options = Press O to open settings.
help-look = Press : to look at the floor.
help-history = Press Ctrl+P to view message history.
help-shift-run = Shift+direction = run in direction.
help-arrows = Arrow keys also work for movement.

## ============================================================================
## System — save and load
## ============================================================================

save-game = Saving game...
save-complete = Game saved.
save-failed = Save failed!
load-game = Restoring saved game...
load-complete = Game restored.
load-failed = Cannot restore game.
load-version-mismatch = Save file version mismatch.

## ============================================================================
## System — configuration
## ============================================================================

config-loaded = Configuration loaded.
config-error = Configuration error: { $message }.
config-option-set = Option "{ $option }" set to "{ $value }".
config-option-unknown = Unknown option: { $option }.
config-language-set = Language set to { $language }.
config-language-unknown = Unknown language: { $language }.

## ============================================================================
## System — error messages
## ============================================================================

error-generic = Something went wrong.
error-save-corrupt = Save file is corrupted.
error-out-of-memory = Out of memory!
error-file-not-found = File not found: { $file }.
error-permission = Permission denied.
error-panic = Panic: { $message }
error-impossible = Program in disorder: { $message }

## ============================================================================
## Miscellaneous — look and search
## ============================================================================

nothing-here = There is nothing here.
something-here = You see { $item } here.
several-things = There are several things here.
look-position = You see { $description }.
search-nothing = You find nothing.
search-found = You find { $item }!
search-secret-door = You find a secret door!
search-secret-passage = You find a secret passage!
search-trap = You find a { $trap }!

## ============================================================================
## Miscellaneous — carry capacity and reach
## ============================================================================

cannot-reach = You can't reach that.
too-heavy = { $item } is too heavy.
inventory-full = Your inventory is full.
pay-prompt = Pay { $amount } for { $item }?
cannot-do-while-blind = You can't do that while blind.
cannot-do-while-confused = You are too confused.
cannot-do-while-stunned = You are too stunned.
cannot-do-while-hallu = You can't concentrate right now.

## ============================================================================
## Items — scrolls (reading effects)
## ============================================================================

scroll-dust = The scroll turns to dust as you read it.
scroll-enchant-weapon = Your { $weapon } glows { $color } for a moment.
scroll-enchant-armor = Your { $armor } glows { $color } for a moment.
scroll-identify = You feel more knowledgeable.
scroll-identify-prompt = What do you want to identify?
scroll-identify-all = You identify everything in your pack!
scroll-remove-curse = You feel as if someone is helping you.
scroll-remove-curse-nothing = You don't feel any different.
scroll-teleport = You feel a wrenching sensation.
scroll-teleport-no-effect = You feel disoriented for a moment.
scroll-create-monster = You hear a low humming sound.
scroll-scare-monster = You hear maniacal laughter in the distance.
scroll-confuse-monster = Your hands begin to glow { $color }.
scroll-magic-mapping = You have a vision of the dungeon!
scroll-fire = The scroll erupts in a tower of flame!
scroll-earth = The ground shakes beneath you!
scroll-amnesia = You feel that something has been forgotten.
scroll-punishment = You are being punished for your misbehavior!
scroll-stinking-cloud = A stinking cloud billows from the scroll.
scroll-charging = You feel a magical surge.
scroll-genocide = A thunderous voice booms through the cavern!
scroll-light = A light fills the room!
scroll-food-detection = You sense the presence of food.
scroll-gold-detection = You sense the presence of gold.
scroll-destroy-armor = Your { $armor } crumbles to dust!
scroll-taming = You feel charismatic.
scroll-mail = You receive a scroll of mail.
scroll-blank-paper = This scroll appears to be blank.

## ============================================================================
## Items — wands (zapping effects)
## ============================================================================

wand-fire = A blast of fire shoots from { $wand }!
wand-cold = A blast of cold shoots from { $wand }!
wand-lightning = A bolt of lightning shoots from { $wand }!
wand-magic-missile = A magic missile shoots from { $wand }!
wand-sleep = A sleep ray shoots from { $wand }.
wand-death = A death ray shoots from { $wand }!
wand-polymorph = A shimmering ray shoots from { $wand }.
wand-striking = A beam shoots from { $wand }!
wand-slow = A slow ray shoots from { $wand }.
wand-speed = A beam of speed shoots from { $wand }.
wand-undead-turning = A beam of undead turning shoots from { $wand }.
wand-opening = A beam of opening shoots from { $wand }.
wand-locking = A beam of locking shoots from { $wand }.
wand-probing = A probe shoots from { $wand }.
wand-digging = A digging beam shoots from { $wand }!
wand-teleportation = A teleportation beam shoots from { $wand }.
wand-create-monster = You hear a low humming sound.
wand-cancellation = A cancellation beam shoots from { $wand }!
wand-make-invisible = A beam of invisibility shoots from { $wand }.
wand-light = A flood of light issues from { $wand }!
wand-darkness = A cloud of darkness issues from { $wand }.
wand-wishing = You may wish for an object.
wand-ray-reflect = The beam reflects off { $surface }!
wand-ray-bounce = The beam bounces off the wall!
wand-ray-absorb = { $entity } absorbs the ray.
wand-break = { $wand } breaks apart and explodes!
wand-break-effect = A cloud of { $element } billows out!
wand-charge-empty = { $wand } seems to have no charges remaining.
wand-recharge = { $wand } glows for a moment.
wand-recharge-fail = { $wand } vibrates violently and explodes!
wand-turn-to-dust = { $wand } turns to dust.

## ============================================================================
## Items — potions (quaffing effects)
## ============================================================================

potion-healing = You feel better.
potion-extra-healing = You feel much better!
potion-full-healing = You feel completely healed!
potion-gain-ability = You feel { $stat ->
    [str] strong
    [dex] agile
    [con] tough
    [int] smart
    [wis] wise
   *[cha] beautiful
    }!
potion-gain-level = You rise up, through the ceiling!
potion-gain-energy = Magical energy flows through you!
potion-speed = You feel very fast!
potion-invisibility = You feel transparent!
potion-see-invisible = You feel perceptive!
potion-levitation = You start to float in the air!
potion-confusion = Huh, what? Where am I?
potion-blindness = Everything goes dark!
potion-hallucination = Oh wow! Everything looks so cosmic!
potion-sleeping = You feel very drowsy.
potion-paralysis = You can't move!
potion-poison = You feel very ill.
potion-acid = The acid burns!
potion-oil = That was smooth.
potion-water = This tastes like water.
potion-holy-water = You feel purified.
potion-unholy-water = You feel a malignant aura surround you.
potion-object-detection = You sense the presence of objects.
potion-monster-detection = You sense the presence of monsters.
potion-sickness = You vomit.
potion-restore-ability = You feel your strength returning!
potion-polymorph = You feel a change coming over you.
potion-booze = Ooph! This tastes like { $liquid }!
potion-fruit-juice = This tastes like { $fruit } juice.
potion-mix = The potions mix and produce { $result }.
potion-dilute = { $potion } becomes diluted.
potion-vapor = You inhale a { $effect } vapor.

## ============================================================================
## Items — item interaction messages
## ============================================================================

pickup-with-quantity = You pick up { $quantity } { $item }.
drop-with-quantity = You drop { $quantity } { $item }.
cannot-carry-more = You can't carry any more.
knapsack-full = Your knapsack cannot accommodate any more items.
encumbrance-prevents = You are too encumbered to do that.
encumbrance-warning-burdened = Your load slows you down.
encumbrance-warning-stressed = You stagger under your heavy load.
encumbrance-warning-strained = You can barely move under this load!
encumbrance-warning-overtaxed = You are about to collapse under your load!
identify-result = { $item } is { $identity }.
identify-already-known = You already know that.
altar-buc-blessed = { $item } glows with a bright amber light!
altar-buc-uncursed = { $item } glows with a faint amber light.
altar-buc-cursed = { $item } glows with a black aura.
altar-buc-unknown = Nothing seems to happen.
item-name-prompt = What do you want to name { $item }?
item-called-prompt = What do you want to call { $item_class }?
item-name-set = You name { $item } "{ $name }".
item-called-set = You call { $item_class } "{ $name }".
nothing-to-drop = You don't have anything to drop.
nothing-to-eat = You don't have anything to eat.
nothing-to-drink = You don't have anything to drink.
nothing-to-read = You don't have anything to read.
nothing-to-wield = You don't have anything to wield.
nothing-to-wear = You don't have anything to wear.
nothing-to-remove = You aren't wearing anything to take off.
nothing-to-zap = You don't have anything to zap.
nothing-to-throw = You don't have anything to throw.
nothing-to-apply = You don't have anything to apply.

## ============================================================================
## Combat — ranged and throwing
## ============================================================================

throw-weapon = You throw { $weapon }!
throw-hits-wall = { $projectile } hits the wall.
throw-falls-short = { $projectile } falls short.
throw-lands = { $projectile } lands on the ground.
throw-breaks = { $projectile } shatters!
throw-multishot = You fire { $count } { $projectile }!
throw-boomerang-return = { $projectile } returns to you!
throw-boomerang-miss = { $projectile } doesn't come back!
ranged-ammo-break = { $projectile } breaks!
ranged-ammo-lost = { $projectile } is lost.
ranged-quiver-empty = Your quiver is empty.
ranged-no-ammo = You have nothing appropriate to fire.
ranged-not-ready = You aren't ready to fire.
shoot-hit = { $projectile } strikes { $defender }!
shoot-miss = { $projectile } whizzes past { $defender }.
shoot-kill = { $projectile } destroys { $defender }!
multishot-fire = You shoot { $count } { $projectile } at { $defender }!
launcher-wield = You ready { $launcher }.
launcher-no-ammo = You have no ammunition for { $launcher }.

## ============================================================================
## Items — rings and amulets
## ============================================================================

ring-put-on = You put on { $ring }.
ring-remove = You remove { $ring }.
ring-cursed-remove = { $ring } is cursed! You can't remove it.
ring-effect-gain = You feel { $effect }.
ring-effect-lose = You no longer feel { $effect }.
ring-shock = The ring shocks you!
ring-hunger = You feel a pang of hunger.
ring-sink-vanish = { $ring } slides off your finger and vanishes down the drain!
amulet-put-on = You put on { $amulet }.
amulet-remove = You remove { $amulet }.
amulet-cursed = { $amulet } is welded to your neck!
amulet-strangulation = The amulet strangles you!
amulet-lifesave = Your amulet breaks into pieces!
amulet-lifesave-msg = But wait... Your medallion feels warm!
amulet-reflection = The { $attack } reflects off your amulet!

## ============================================================================
## Items — tools
## ============================================================================

tool-apply = You apply { $tool }.
tool-lamp-on = { $lamp } begins to shine.
tool-lamp-off = { $lamp } goes out.
tool-lamp-fuel = { $lamp } runs out of fuel.
tool-pick-locked = You succeed in picking the lock.
tool-pick-fail = You fail to pick the lock.
tool-horn-blow = You produce a { $effect } sound!
tool-mirror-reflect = You reflect { $attack } with your mirror!
tool-mirror-look = You see your face in the mirror.
tool-stethoscope = You listen to { $target }.
tool-tinning-kit = You begin tinning { $corpse }.
tool-leash-attach = You attach { $leash } to { $pet }.
tool-leash-detach = You remove { $leash } from { $pet }.
tool-camera-flash = You snap a photo of { $target }!
tool-whistle-blow = You blow the whistle.
tool-whistle-magic = You produce a strange whistling sound!

## ============================================================================
## Dungeon features — special rooms and events
## ============================================================================

shop-enter = You enter { $shopkeeper }'s { $shoptype }.
shop-leave = { $shopkeeper } says "Come back again!"
shop-price = "For you, { $item } only { $price } gold piece{ $price ->
    [one] {""}
   *[other] s
    }."
shop-stolen = You have unpaid merchandise!
shop-damage = { $shopkeeper } says "You'll pay for the damage!"
shop-shoplift = "Stop, thief!" shrieks { $shopkeeper }.
temple-enter = You enter a temple of { $god }.
temple-donate = { $priest } accepts your donation.
temple-protection = { $priest } bestows protection upon you.
vault-guard = Suddenly one of the vault's guards appears!
vault-guard-ask = "Who are you, and what are you doing here?"

## ============================================================================
## Traps — expanded trap messages
## ============================================================================

trap-bear-leg = A bear trap closes on your leg!
trap-bear-escape = You pull free from the bear trap.
trap-bear-stuck = You are stuck in a bear trap!
trap-pit-climb = You climb out of the pit.
trap-pit-cant-climb = You try to climb out of the pit, but you can't!
trap-spiked-damage = The spikes were poisoned!
trap-arrow-dodge = You dodge the arrow!
trap-dart-poison = The dart was poisoned!
trap-land-mine = KABOOM! You triggered a land mine!
trap-land-mine-set = You set the land mine.
trap-sleeping-gas = A cloud of gas puts you to sleep!
trap-hole = You fall through a hole in the floor!
trap-trapdoor = A trapdoor opens up under you!
trap-magic-trap = You are caught in a magical explosion!
trap-anti-magic = You feel drained of magical energy!
trap-statue = The statue comes to life!
trap-vibrating-square = You feel a strange vibration under your feet.
trap-seen = You see a { $trap } here.
trap-monster-trigger = The { $monster } triggers a { $trap }!
trap-monster-pit = The { $monster } falls into a pit!
trap-monster-bear = The { $monster } is caught in a bear trap!
trap-monster-web = The { $monster } is caught in a web!
trap-monster-teleport = The { $monster } vanishes!
trap-set-fail = You fail to set the trap.
trap-set-success = You set the { $trap }.

## ============================================================================
## Artifacts — special effects and messages
## ============================================================================

artifact-resist = The artifact resists!
artifact-evade = { $artifact } evades your grasp!
artifact-blast = { $artifact } blasts you!
artifact-glow-fire = { $artifact } glows with divine fire!
artifact-glow-cold = { $artifact } glows with an icy blue light!
artifact-glow-warning = { $artifact } glows with a warning light!
artifact-invoke = You invoke the power of { $artifact }.
artifact-invoke-fail = Nothing seems to happen.
artifact-gift = { $god } grants you { $artifact }!
artifact-touch-blast = { $artifact } sears your flesh!
artifact-speak = { $artifact } speaks to you!
artifact-sing = { $artifact } sings in your hand.
artifact-thirst = { $artifact } thirsts for blood!
artifact-kill-msg = { $artifact } strikes { $defender } with deadly force!
artifact-bisect = { $artifact } bisects { $defender }!
artifact-drain-life = { $artifact } drains the life from { $defender }!
artifact-found = You sense the presence of { $artifact } nearby.
artifact-already-exists = An artifact by that name already exists in this game.
artifact-wish-denied = For a moment you feel something in your hands, but it disappears!
artifact-name-change = { $artifact }'s name changes before your eyes!

## ============================================================================
## Shop — expanded messages
## ============================================================================

shop-owe = You owe { $shopkeeper } { $amount } zorkmid{ $amount ->
    [one] {""}
   *[other] s
    }.
shop-bill-total = Your bill comes to { $amount } zorkmid{ $amount ->
    [one] {""}
   *[other] s
    }.
shop-pay-success = You pay { $amount } zorkmid{ $amount ->
    [one] {""}
   *[other] s
    } to { $shopkeeper }.
shop-no-money = You don't have enough money!
shop-buy = You buy { $item } for { $price } zorkmid{ $price ->
    [one] {""}
   *[other] s
    }.
shop-sell = You sell { $item } for { $price } zorkmid{ $price ->
    [one] {""}
   *[other] s
    }.
shop-credit = You have { $amount } zorkmid{ $amount ->
    [one] {""}
   *[other] s
    } credit.
shop-door-block = { $shopkeeper } blocks the doorway!
shop-angry = { $shopkeeper } gets angry!
shop-kops = The Keystone Kops are coming!
shop-kops-arrive = The Keystone Kops arrive!
shop-use-unpaid = "You are using unpaid merchandise!" yells { $shopkeeper }.
shop-broke-item = You broke { $item }! { $shopkeeper } demands { $price } zorkmid{ $price ->
    [one] {""}
   *[other] s
    }.
shop-welcome-back = { $shopkeeper } says "Welcome back! You owe { $amount } zorkmid{ $amount ->
    [one] {""}
   *[other] s
    }."
shop-closed = The shop is closed.

## ============================================================================
## Religion — prayer, sacrifice, crowning
## ============================================================================

pray-start = You begin praying to { $god }.
pray-feel-warm = You feel a warm glow.
pray-feel-at-peace = You feel at peace.
pray-full-heal = You feel much better!
pray-uncurse = You feel as if { $god } is helping you.
pray-resist = You feel resistant!
pray-angry-god = { $god } is displeased!
pray-ignored = { $god } does not seem to be listening.
pray-punish = { $god } punishes you!
pray-gift-weapon = { $god } grants you a gift!
pray-mollified = { $god } seems mollified.
pray-reconciled = { $god } seems to have forgiven you.
sacrifice-accept = Your sacrifice is consumed in a burst of flame!
sacrifice-reject = { $god } is not impressed.
sacrifice-already-full = You have a feeling of inadequacy.
sacrifice-wrong-altar = You feel guilty.
sacrifice-convert = The altar converts to { $god }!
sacrifice-gift = { $god } is pleased and bestows a gift upon you!
crown-msg = You hear a voice boom: "Thou art chosen!"
crown-gain = You feel the power of { $god } coursing through you!

## ============================================================================
## Pet — expanded messages
## ============================================================================

pet-hungry = Your { $pet } seems hungry.
pet-very-hungry = Your { $pet } is very hungry!
pet-starving = Your { $pet } is starving!
pet-refuses-food = Your { $pet } refuses to eat { $food }.
pet-loyal = Your { $pet } looks at you adoringly.
pet-growl = Your { $pet } growls at you!
pet-confused = Your { $pet } looks confused.
pet-injured = Your { $pet } looks injured.
pet-healed = Your { $pet } looks healthier.
pet-level-up = Your { $pet } seems more experienced!
pet-died = Your { $pet } is killed!
pet-revived = Your { $pet } has been revived!
pet-attack-monster = Your { $pet } attacks the { $monster }!
pet-fetch = Your { $pet } fetches { $item }.
pet-saddle = You put a saddle on your { $pet }.

## ============================================================================
## Hunger — eating effects, corpse effects, intrinsics
## ============================================================================

eat-gain-strength = You feel strong!
eat-gain-telepathy = You feel a strange mental acuity.
eat-gain-invisibility = You feel transparent!
eat-gain-poison-resist = You feel healthy!
eat-gain-fire-resist = You feel a chill.
eat-gain-cold-resist = You feel warm.
eat-gain-sleep-resist = You feel wide awake!
eat-gain-shock-resist = You feel insulated.
eat-tainted = Ulch - that food was tainted!
eat-corpse-taste = This { $corpse } tastes { $taste ->
    [terrible] terrible
    [bland] bland
    [okay] okay
   *[normal] like { $corpse }
    }!
eat-petrify = You feel yourself turning to stone!
eat-polymorph = You feel a change coming over you!
eat-stun = You stagger momentarily.
eat-hallucinate = Oh wow! You feel trippy!
eat-acidic = The acidic food burns your stomach!

## ============================================================================
## Conduct — violations and achievements
## ============================================================================

conduct-vegetarian-break = You have broken vegetarian conduct.
conduct-vegan-break = You have broken vegan conduct.
conduct-foodless-break = You have broken foodless conduct.
conduct-atheist-break = You have broken atheist conduct.
conduct-weaponless-break = You have broken weaponless conduct.
conduct-pacifist-break = You have broken pacifist conduct.
conduct-illiterate-break = You have broken illiterate conduct.
conduct-genocideless-break = You have broken genocideless conduct.
conduct-polypileless-break = You have broken polypileless conduct.
conduct-polyself-break = You have broken polyselfless conduct.
achievement-unlock = Achievement unlocked: { $name }!
achievement-sokoban = You solved the Sokoban puzzle!
achievement-mines-end = You reached the bottom of the Gnomish Mines!
achievement-medusa = You defeated Medusa!
achievement-castle = You breached the Castle!
achievement-amulet = You obtained the Amulet of Yendor!

## ============================================================================
## Monster AI — item use, covetous behavior
## ============================================================================

monster-reads = The { $monster } reads a scroll!
monster-uses-wand = The { $monster } zaps a wand of { $wand_type }!
monster-quaffs = The { $monster } quaffs a potion!
monster-puts-on = The { $monster } puts on { $item }.
monster-removes = The { $monster } takes off { $item }.
monster-heals = The { $monster } looks healthier!
monster-teleport-away = The { $monster } teleports away!
monster-covetous-approach = The { $monster } approaches menacingly!
monster-covetous-steal = The { $monster } steals the { $item } from you!
monster-covetous-flee = The { $monster } retreats with { $item }!
monster-unlock = The { $monster } unlocks the door.
monster-open-door = The { $monster } opens the door.
monster-close-door = The { $monster } closes the door.
monster-break-door = The { $monster } breaks the door!
monster-dig = The { $monster } digs through the wall!

## ============================================================================
## Special levels — Sokoban, Mines, Oracle, etc.
## ============================================================================

level-sokoban-enter = You enter what seems to be a puzzle room.
level-sokoban-solve = Click! You hear a door unlock.
level-sokoban-cheat = You hear a rumbling sound.
level-mines-enter = You enter the Gnomish Mines.
level-mines-town = You enter Mine Town.
level-oracle-enter = You see a large room with a strange fountain.
level-oracle-speak = The Oracle speaks...
level-oracle-consult = The Oracle offers to share wisdom for { $price } zorkmid{ $price ->
    [one] {""}
   *[other] s
    }.
level-oracle-rumor = The Oracle reveals: "{ $rumor }"
level-castle-enter = You feel a sense of dread as you enter.
level-vlad-tower = You feel a chilling presence.
level-sanctum-enter = You have a strange forbidding feeling...
level-astral-enter = You arrive on the Astral Plane!

## ============================================================================
## Score and endgame — expanded messages
## ============================================================================

score-display = Score: { $score }
score-rank = You are ranked { $rank }.
score-high-new = NEW HIGH SCORE!
score-high-list-title = Top Scores
score-high-entry = { $rank }. { $name } the { $role } ({ $score } points)
score-gold-collected = Gold collected: { $amount }
score-monsters-killed = Monsters killed: { $count }
score-deepest-level = Deepest level reached: { $depth }
score-death-by = Killed by { $killer } on dungeon level { $depth }.
score-escaped-with = You escaped with { $score } points.
score-ascended-with = You ascended with { $score } points!
game-over-conduct-title = Voluntary challenges:
game-over-conduct-item = You followed { $conduct } conduct.
game-over-dungeon-overview = Dungeon overview:
game-over-vanquished = Vanquished creatures:
game-over-genocided = Genocided species:


## ============================================================================
## Engine i18n keys — movement
## ============================================================================

diagonal-squeeze-blocked = You can't squeeze through that diagonal gap.
door-no-closed = There is no closed door there.
door-no-open = There is no open door there.
door-no-kick = There is no door to kick there.
pet-swap = You swap places with your pet.
pet-nearby = Your { $pet } is nearby.

## ============================================================================
## Engine i18n keys — scrolls (extended)
## ============================================================================

scroll-identify-one = You identify an item.
scroll-identify-count = You identify { $count } items.
scroll-nothing-to-identify = You have nothing to identify.
scroll-enchant-weapon-fragile = Your weapon feels fragile.
scroll-enchant-weapon-film = Your weapon is covered by a thin film.
scroll-enchant-weapon-evaporate = Your weapon evaporates!
scroll-enchant-weapon-vibrate = Your weapon suddenly vibrates unexpectedly.
scroll-enchant-armor-skin = Your skin glows then fades.
scroll-enchant-armor-fragile = Your armor feels fragile.
scroll-enchant-armor-film = Your armor is covered by a thin film.
scroll-enchant-armor-evaporate = Your armor evaporates!
scroll-enchant-armor-vibrate = Your armor suddenly vibrates unexpectedly.
scroll-remove-curse-malignant = You feel a malignant aura surround you.
scroll-remove-curse-blessed = You feel in touch with the Universal Oneness.
scroll-remove-curse-punishment = Your punishment is removed!
scroll-disintegrate = The scroll disintegrates.
scroll-confuse-cursed = Your hands twitch.
scroll-teleport-disoriented = You feel very disoriented.
scroll-trap-detection = You sense the presence of traps.
scroll-scare-wailing = You hear sad wailing in the distance.
scroll-scare-dust = The scroll turns to dust as you pick it up.
scroll-fire-burn = The scroll catches fire and you burn your hands.
scroll-earth-rocks = Rocks fall around you!
scroll-earth-boulders = Boulders fall around you!
scroll-amnesia-spells = You forget your spells!
scroll-destroy-armor-itch = Your skin itches.
scroll-destroy-armor-glow = Your armor glows.
scroll-destroy-armor-crumble = Your armor crumbles to dust!
scroll-taming-growl = You hear angry growling!
scroll-genocide-guilty = You feel guilty.
scroll-genocide-prompt = What monster do you want to genocide?
scroll-genocide-prompt-class = What class of monsters do you wish to genocide?
scroll-light-sparkle = Tiny lights sparkle around you.
scroll-charging-drained = You feel drained.
scroll-charging-id = This is a charging scroll.
scroll-charging-nothing = You don't have anything to charge.
scroll-magic-mapping-fail = Unfortunately, you can't grasp the details.
scroll-create-monster-horde = A horde of monsters appears!

## ============================================================================
## Engine i18n keys — traps (extended)
## ============================================================================

trap-arrow-shoot = An arrow shoots out at you!
trap-dart-shoot = A little dart shoots out at you!
trap-dart-poison-resist = The dart was poisoned, but the poison doesn't seem to affect you.
trap-trapdoor-ceiling = A trap door in the ceiling opens, but nothing falls out!
trap-sleeping-gas-sleep = A cloud of gas puts you to sleep!
trap-fire-resist = A tower of flame erupts from the floor! But you resist the effects.
trap-rolling-boulder-trigger = Click! You trigger a rolling boulder trap!
trap-teleport-wrench = You feel a wrenching sensation.
trap-web-tear = You tear through the web!
trap-web-free = You pull free from the web.
trap-web-stuck = You are stuck in the web.
trap-magic-trap-blind = You are blinded by a flash of light!
trap-door-booby = The door was booby-trapped!
trap-gas-puff = A puff of gas engulfs you!
trap-gas-cloud = A cloud of gas surrounds you!
trap-shock = You receive an electric shock!
trap-chest-explode = KABOOM!! The chest explodes!
trap-pit-float = You float out of the pit.
trap-bear-rip-free = You rip free from the bear trap!
trap-cannot-disarm = You don't see any trap to disarm here.
trap-disarm-fail = You fail to disarm the trap.

## ============================================================================
## Engine i18n keys — teleportation
## ============================================================================

teleport-random = You are teleported!
teleport-controlled = Where do you want to teleport to?
teleport-invalid-target = You can't teleport there!
teleport-level = You are teleported to another level!
teleport-same-level = You shudder for a moment.
teleport-restricted = A mysterious force prevents you from teleporting!
teleport-branch = You feel yourself yanked to another branch of the dungeon!
teleport-monster = A monster vanishes from sight!
teleport-no-portal = You feel a wrenching sensation, but nothing happens.
teleport-trap-controlled = You are teleported by a trap! You have teleport control.
teleport-trap-restricted = A mysterious force prevents you from teleporting!

## ============================================================================
## Engine i18n keys — movement (extended)
## ============================================================================

ice-slide = You slide across the ice!
ice-fumble-fall = You slip on the ice and fall!
water-float-over = You float over the water.
water-swim = You swim through the water.
water-drown-danger = You are drowning!
lava-float-over = You float over the lava.
lava-resist = The lava burns you, but you resist the worst of it.
lava-burn = The lava sears you terribly!
fumble-trip = You trip over your own feet!

## ============================================================================
## Engine i18n keys — engulfment
## ============================================================================

engulf-attack-interior = You hit the interior of the monster!
engulf-escaped = You escape from the engulfing monster!
engulf-monster-dies = The engulfing monster dies!

## ============================================================================
## Engine i18n keys — potions (extended)
## ============================================================================

potion-blindness-cure = Your vision clears.
potion-gain-ability-str = You feel strong!
potion-paralysis-brief = You stiffen momentarily.
potion-no-effect = You feel a lack of something.
potion-sickness-deadly = You feel deathly sick.
potion-booze-passout = You pass out.
potion-enlightenment = You feel self-knowledgeable...

## ============================================================================
## Engine i18n keys — hunger (extended)
## ============================================================================

eat-choke = You choke on your food!
eat-dread = You feel a sense of dread.
eat-corpse-effect = You feel an unusual effect from eating that corpse.
eat-weakened = You feel weakened.
eat-greasy = Your fingers are very greasy.
eat-poison-resist = You seem unaffected by the poison.

## ============================================================================
## Engine i18n keys — religion (extended)
## ============================================================================

sacrifice-own-kind-anger = You have angered your god by sacrificing your own kind!
sacrifice-own-kind-pleased = Your god is pleased by the sacrifice of your own kind.
sacrifice-pet-guilt = You feel guilty about sacrificing your former pet.
sacrifice-reduce-timeout = Your sacrifice reduces the time until your next prayer.
pray-partial = Your prayer is only partially heard.

## ============================================================================
## Engine i18n keys — artifacts (extended)
## ============================================================================

artifact-invoke-heal = You feel better.
artifact-invoke-energy = You feel a surge of magical energy!
artifact-invoke-enlighten = You feel self-knowledgeable...
artifact-invoke-conflict = You feel like a rabble-rouser.
artifact-invoke-invisible = You feel quite transparent.
artifact-invoke-levitate = You start to float in the air!
artifact-invoke-untrap = You feel skilled at disarming traps.
artifact-invoke-charge = You may charge an object.
artifact-invoke-teleport = You feel a wrenching sensation.
artifact-invoke-portal = You feel a shimmering in the air.
artifact-invoke-arrows = A volley of arrows appears!
artifact-invoke-brandish = You brandish the artifact menacingly!
artifact-invoke-venom = You fling a venomous glob!
artifact-invoke-cold = A blast of cold erupts!
artifact-invoke-fire = A ball of fire erupts!
artifact-invoke-light = A blinding ray of light shoots forth!

## ============================================================================
## Engine i18n keys — wands (extended)
## ============================================================================

wand-enlightenment = You feel self-knowledgeable.
wand-secret-door-detect = You sense the presence of secret doors.

## ============================================================================
## Engine i18n keys — shop (extended)
## ============================================================================

shop-free = You got that for free!
shop-return = { $shopkeeper } accepts the return.
shop-not-interested = { $shopkeeper } is not interested.
shop-angry-take = "Thank you, scum!"
shop-restock = { $shopkeeper } seems grateful for restocking.
shop-no-debt = You don't owe anything.
shop-credit-covers = Your credit covers the bill.
shop-stolen-amount = You stole { $amount } zorkmids worth of merchandise.

## ============================================================================
## Item naming — BUC labels
## ============================================================================

item-buc-blessed = blessed
item-buc-uncursed = uncursed
item-buc-cursed = cursed

## ============================================================================
## Item naming — erosion adjectives
## ============================================================================

item-erosion-rusty = rusty
item-erosion-very-rusty = very rusty
item-erosion-thoroughly-rusty = thoroughly rusty
item-erosion-corroded = corroded
item-erosion-very-corroded = very corroded
item-erosion-thoroughly-corroded = thoroughly corroded
item-erosion-burnt = burnt
item-erosion-very-burnt = very burnt
item-erosion-thoroughly-burnt = thoroughly burnt
item-erosion-rotted = rotted
item-erosion-very-rotted = very rotted
item-erosion-thoroughly-rotted = thoroughly rotted
item-erosion-rustproof = rustproof
item-erosion-fireproof = fireproof
item-erosion-corrodeproof = corrodeproof
item-erosion-rotproof = rotproof

## ============================================================================
## Item naming — class-specific base name patterns
## ============================================================================

item-potion-identified = potion of { $name }
item-potion-called = potion called { $called }
item-potion-appearance = { $appearance } potion
item-potion-generic = potion

item-scroll-identified = scroll of { $name }
item-scroll-called = scroll called { $called }
item-scroll-labeled = scroll labeled { $label }
item-scroll-appearance = { $appearance } scroll
item-scroll-generic = scroll

item-wand-identified = wand of { $name }
item-wand-called = wand called { $called }
item-wand-appearance = { $appearance } wand
item-wand-generic = wand

item-ring-identified = ring of { $name }
item-ring-called = ring called { $called }
item-ring-appearance = { $appearance } ring
item-ring-generic = ring

item-amulet-called = amulet called { $called }
item-amulet-appearance = { $appearance } amulet
item-amulet-generic = amulet

item-spellbook-identified = spellbook of { $name }
item-spellbook-called = spellbook called { $called }
item-spellbook-appearance = { $appearance } spellbook
item-spellbook-generic = spellbook

item-gem-stone = stone
item-gem-gem = gem
item-gem-called-stone = stone called { $called }
item-gem-called-gem = gem called { $called }
item-gem-appearance-stone = { $appearance } stone
item-gem-appearance-gem = { $appearance } gem

item-generic-called = { $base } called { $called }

## ============================================================================
## Item naming — connectors and suffixes
## ============================================================================

item-named-suffix = named { $name }

## ============================================================================
## Item naming — articles
## ============================================================================

item-article-the = the
item-article-your = your

## ============================================================================
## Item naming — plural selection (CLDR-aware)
## ============================================================================

item-count-name = { $count ->
    [one] { $singular }
   *[other] { $plural }
    }

## ============================================================================
## Status line labels
## ============================================================================

status-satiated = Satiated
status-hungry = Hungry
status-weak = Weak
status-fainting = Fainting
status-not-hungry = {""}
status-starved = Starved

## ============================================================================
## UI — titles and labels
## ============================================================================

ui-inventory-title = Inventory
ui-inventory-empty = You are not carrying anything.
ui-equipment-title = Equipment
ui-equipment-empty = You are not wearing anything special.
ui-help-title = NetHack Babel Help
ui-message-history-title = Message History
ui-select-language = Select language
ui-more = --More--
ui-save-prompt = Saving game...
ui-save-success = Game saved.
ui-save-goodbye = Game saved. Goodbye!
ui-goodbye = Goodbye!
ui-game-over-thanks = Game Over. Thanks for playing!
ui-unknown-command = Unknown command: '{ $key }'. Press ? for help.

## ============================================================================
## UI — prompts
## ============================================================================

prompt-drop = Drop what? [a-zA-Z or ?*]
prompt-wield = Wield what? [a-zA-Z or - for bare hands]
prompt-wear = Wear what? [a-zA-Z or ?*]
prompt-takeoff = Take off what? [a-zA-Z or ?*]
prompt-puton = Put on what? [a-zA-Z or ?*]
prompt-remove = Remove what? [a-zA-Z or ?*]
prompt-apply = Apply what? [a-zA-Z or ?*]
prompt-throw-item = Throw what? [a-zA-Z or ?*]
prompt-throw-dir = In what direction?
prompt-zap-item = Zap what? [a-zA-Z or ?*]
prompt-zap-dir = In what direction?
prompt-open-dir = Open in what direction?
prompt-close-dir = Close in what direction?
prompt-fight-dir = Fight in what direction?
prompt-pickup = Pick up what?
prompt-dip-item = Dip what? [a-zA-Z]
prompt-dip-into = Dip into what? [a-zA-Z]

## ============================================================================
## UI — inventory class headers
## ============================================================================

inv-class-weapon = Weapons
inv-class-armor = Armor
inv-class-ring = Rings
inv-class-amulet = Amulets
inv-class-tool = Tools
inv-class-food = Comestibles
inv-class-potion = Potions
inv-class-scroll = Scrolls
inv-class-spellbook = Spellbooks
inv-class-wand = Wands
inv-class-coin = Coins
inv-class-gem = Gems/Stones
inv-class-rock = Rocks
inv-class-ball = Iron balls
inv-class-chain = Chains
inv-class-venom = Venom
inv-class-other = Other

# Inventory BUC markers
inv-buc-marker-blessed = [B]
inv-buc-marker-cursed = [C]
inv-buc-tag-blessed = (blessed)
inv-buc-tag-cursed = (cursed)
inv-buc-tag-uncursed = (uncursed)
ui-pickup-title = Pick up what?

## ============================================================================
## Event messages — fallbacks (used when engine event has no FTL key)
## ============================================================================

event-hp-gained = You feel better.
event-hp-lost = Ouch!
event-pw-gained = You feel magical energy returning.
event-you-see-here = You see { $terrain } here.
event-dungeon-welcome = You find yourself in a dungeon. Good luck!
event-player-role = You are { $name } the { $race } { $role } { $align }.

## ============================================================================
## Terrain names
## ============================================================================

terrain-floor = a floor
terrain-corridor = a corridor
terrain-wall = a wall
terrain-closed-door = a closed door
terrain-open-door = an open door
terrain-stairs-up = stairs going up
terrain-stairs-down = stairs going down
terrain-fountain = a fountain
terrain-altar = an altar
terrain-water = water
terrain-lava = lava
terrain-trap = a trap
terrain-tree = a tree
terrain-iron-bars = iron bars

## ============================================================================
## Engine — trap messages
## ============================================================================

trap-shiver = You shiver suddenly.
trap-howl = You hear a distant howling.
trap-yearning = You feel a strange yearning.
trap-pack-shakes = Your pack shakes violently!
trap-fumes = You smell acrid fumes.
trap-tired = You feel tired all of a sudden.

## ============================================================================
## Engine — identification class names
## ============================================================================

id-class-potion = potion
id-class-scroll = scroll
id-class-ring = ring
id-class-wand = wand
id-class-spellbook = spellbook
id-class-amulet = amulet
id-class-weapon = weapon
id-class-armor = armor
id-class-tool = tool
id-class-food = food
id-class-coin = gold piece
id-class-gem = gem
id-class-rock = rock
id-class-ball = iron ball
id-class-chain = iron chain
id-class-venom = splash of venom
id-class-unknown = thing
id-unknown-object = strange object
id-something = something

## ============================================================================
## Engine — shop type names
## ============================================================================

shop-type-general = general store
shop-type-armor = used armor dealership
shop-type-book = second-hand bookstore
shop-type-liquor = liquor emporium
shop-type-weapon = antique weapons outlet
shop-type-deli = delicatessen
shop-type-jewel = jewelers
shop-type-apparel = quality apparel and accessories
shop-type-hardware = hardware store
shop-type-rare-book = rare books
shop-type-health = health food store
shop-type-lighting = lighting store

## ============================================================================
## Engine — pet kind names
## ============================================================================

pet-little-dog = little dog
pet-kitten = kitten
pet-pony = pony

## ============================================================================
## Engine — alignment names
## ============================================================================

align-law = Law
align-balance = Balance
align-chaos = Chaos

## ============================================================================
## BUC tags (for inventory display)
## ============================================================================

buc-tag-blessed = (blessed)
buc-tag-cursed = (cursed)
buc-tag-uncursed = (uncursed)
buc-marker-blessed = [B]
buc-marker-cursed = [C]

## ============================================================================
## Character creation — roles
## ============================================================================

role-archeologist = Archeologist
role-barbarian = Barbarian
role-caveperson = Caveperson
role-healer = Healer
role-knight = Knight
role-monk = Monk
role-priest = Priest
role-ranger = Ranger
role-rogue = Rogue
role-samurai = Samurai
role-tourist = Tourist
role-valkyrie = Valkyrie
role-wizard = Wizard

## ============================================================================
## Character creation — races
## ============================================================================

race-human = Human
race-elf = Elf
race-dwarf = Dwarf
race-gnome = Gnome
race-orc = Orc

## ============================================================================
## Character creation — alignments
## ============================================================================

alignment-lawful = Lawful
alignment-neutral = Neutral
alignment-chaotic = Chaotic

## ============================================================================
## Character creation — prompts
## ============================================================================

chargen-pick-role = Pick a role:
chargen-pick-race = Pick a race:
chargen-pick-alignment = Pick an alignment:
chargen-who-are-you = Who are you? [default: { $default }]

## ============================================================================
## Status bar labels — line 1 (attributes)
## ============================================================================

stat-label-str = St
stat-label-dex = Dx
stat-label-con = Co
stat-label-int = In
stat-label-wis = Wi
stat-label-cha = Ch

## ============================================================================
## Status bar labels — line 2 (dungeon stats)
## ============================================================================

stat-label-dlvl = Dlvl
stat-label-gold = $
stat-label-hp = HP
stat-label-pw = Pw
stat-label-ac = AC
stat-label-xp = Xp
stat-label-turn = T

## ============================================================================
## Options menu
## ============================================================================

ui-options-title = Options
ui-options-game = Game Settings
ui-options-display = Display Settings
ui-options-sound = Sound Settings

## Game options

opt-autopickup = Autopickup
opt-autopickup-types = Autopickup types
opt-legacy = Legacy intro

## Display options

opt-map-colors = Map colors
opt-message-colors = Message colors
opt-buc-highlight = BUC highlight
opt-minimap = Minimap
opt-mouse-hover = Mouse hover info
opt-nerd-fonts = Nerd fonts

## Sound options

opt-sound-enabled = Sound enabled
opt-volume = Volume

## Option values

opt-on = ON
opt-off = OFF

## ============================================================================
## Legacy intro narrative
## ============================================================================

legacy-intro =
    It is written in the Book of { $deity }:

        After the Creation, the cruel god Moloch rebelled
        against the authority of Marduk the Creator.
        Moloch stole from Marduk the most powerful of all
        the artifacts of the gods, the Amulet of Yendor,
        and he hid it in the dark cavities of Gehennom, the
        Under World, where he now lurks, and bides his time.

    Your { $deity } seeks to possess the Amulet, and with it
    to gain deserved ascendance over the other gods.

    You, a newly trained { $role }, have been heralded
    from birth as the instrument of { $deity }. You are destined
    to recover the Amulet for your deity, or die in the
    attempt. Your hour of destiny has come. For the sake
    of us all: Go bravely with { $deity }!

## ============================================================================
## TUI common messages
## ============================================================================

ui-never-mind = Never mind.
ui-no-such-item = You don't have that item.
ui-not-implemented = Not yet implemented.
ui-empty-handed = You are empty handed.

## ============================================================================
## Action dispatch
## ============================================================================

eat-generic = You eat the food.
eat-what = Eat what?
quaff-generic = You drink the potion.
quaff-what = Drink what?
read-generic = You read the scroll.
read-what = Read what?
zap-generic = You zap the wand.

## Doors
door-open-success = The door opens.
door-already-open = This door is already open.
door-not-here = You see no door there.
door-close-success = The door closes.
door-already-closed = This door is already closed.

## Locks
lock-nothing-to-force = You see nothing to force open here.

## Prayer
pray-begin = You begin praying to your deity...

## Offerings
offer-generic = You offer a sacrifice at the altar.
offer-what = Sacrifice what?

## Chat
npc-chat-no-response = The creature doesn't seem to want to chat.
chat-nobody-there = There is nobody there to talk to.

## Movement / travel
ride-not-available = There is nothing here to ride.
enhance-not-available = You cannot enhance any skills right now.
travel-not-implemented = Travel is not yet available.
two-weapon-not-implemented = Two-weapon combat is not yet available.
name-not-implemented = Naming is not yet available.
adjust-not-implemented = Inventory adjustment is not yet available.

## ============================================================================
## Quest / NPC dialogue
## ============================================================================

quest-leader-greeting = Welcome, { $role }. I have been expecting you.
quest-assignment =
    Listen well, { $role }. The { $nemesis } has stolen the { $artifact }.
    You must descend into the depths and retrieve it.
    Our fate depends on you.

## Shopkeeper
shop-welcome = Welcome to { $shopkeeper }'s { $shoptype }!
shop-buy-prompt = { $shopkeeper } says: "Will that be cash or credit?"
shop-unpaid-warning = { $shopkeeper } says: "You have unpaid items!"
shop-theft-warning = { $shopkeeper } shouts: "Thief! Pay up or else!"

## Priest
priest-welcome = The priest intones a blessing upon you.
priest-protection-offer = The priest offers divine protection for { $cost } gold.
priest-donation-thanks = The priest thanks you for your generous donation.

## ============================================================================
## Content delivery (rumors, oracles)
## ============================================================================

rumor-fortune-cookie = You open the fortune cookie. It reads: "{ $rumor }"
oracle-consultation = { $text }

## ============================================================================
## Symbol identification
## ============================================================================

whatis-prompt = What do you want to identify? (pick a position)
whatis-terrain = { $description } (terrain)
whatis-monster = { $description } (monster)
whatis-object = { $description } (object)
whatis-nothing = You see nothing special there.

## Discoveries
discoveries-title = Discoveries
discoveries-empty = You haven't discovered anything yet.

## ============================================================================
## Attack — elemental hits (monster attacks on you)
## ============================================================================

attack-acid-hit = You are splashed by acid!
attack-acid-resisted = The acid doesn't seem to affect you.
attack-cold-hit = You are covered in frost!
attack-cold-resisted = You feel mildly chilly.
attack-fire-hit = You are engulfed in flames!
attack-fire-resisted = You feel mildly warm.
attack-shock-hit = You are jolted by electricity!
attack-shock-resisted = You are only mildly tingled.
attack-breath = { $monster } breathes at you!
attack-engulf = { $monster } engulfs you!

## ============================================================================
## Attack — special monster attacks
## ============================================================================

attack-disease = You feel very sick.
attack-disintegrate = You are disintegrated!
attack-disintegrate-resisted = You are not disintegrated.
attack-drain-level = You feel your life force draining away!
attack-hug-crush = You are being crushed!
attack-paralyze = You are frozen in place!
attack-poisoned = You feel very sick!
attack-sleep = You feel drowsy...
attack-slowed = You feel yourself moving more slowly.
attack-stoning-start = You are starting to turn to stone!

## ============================================================================
## Boulder
## ============================================================================

boulder-blocked = The boulder is wedged in.
boulder-fills-pit = The boulder fills a pit!
boulder-push = You push the boulder.

## ============================================================================
## Choking / Suffocation
## ============================================================================

choke-blood-trouble = You find it hard to breathe.
choke-consciousness-fading = Your consciousness is fading...
choke-gasping-for-air = You are gasping for air!
choke-hard-to-breathe = You find it hard to breathe.
choke-neck-constricted = Your neck is being constricted!
choke-neck-pressure = You feel pressure on your neck.
choke-no-longer-breathe = You can no longer breathe.
choke-suffocate = You suffocate.
choke-turning-blue = You're turning blue.

## ============================================================================
## Container
## ============================================================================

container-put-in = You put { $item } into { $container }.
container-take-out = You take { $item } out of { $container }.

## ============================================================================
## Crystal ball
## ============================================================================

crystal-ball-cloudy = All you see is a swirling mess.
crystal-ball-nothing-new = You see nothing new.

## ============================================================================
## Cursed items
## ============================================================================

cursed-cannot-remove = You can't remove it, it's cursed!

## ============================================================================
## Detection
## ============================================================================

detect-food-none = You don't sense any food.
detect-gold-none = You don't sense any gold.
detect-monsters-none = You don't sense any monsters.
detect-objects-none = You don't sense any objects.
detect-traps-none = You don't sense any traps.
clairvoyance-nothing-new = You sense nothing new.
magic-mapping-nothing-new = You are already aware of your surroundings.
reveal-monsters-none = There are no monsters to reveal.

## ============================================================================
## Digging
## ============================================================================

dig-blocked = This is too hard to dig in.
dig-floor-blocked = The floor here is too hard to dig in.
dig-floor-hole = You dig a hole through the floor!
dig-ray-nothing = The digging ray has no effect.
dig-wall-done = You finish digging through the wall.

## ============================================================================
## Dipping
## ============================================================================

dip-acid-nothing = Nothing happens.
dip-acid-repair = Your { $item } looks as good as new!
dip-amethyst-cure = You feel less confused.
dip-diluted = Your { $item } is diluted.
dip-excalibur = As you dip the sword, a strange light plays over it! Your sword is now named Excalibur!
dip-fountain-cursed = The water glows for a moment.
dip-fountain-nothing = Nothing seems to happen.
dip-fountain-rust = Your { $item } rusts!
dip-holy-water = You dip your { $item } in the holy water.
dip-no-fountain = There is no fountain here to dip into.
dip-not-a-potion = That's not a potion!
dip-nothing-happens = Nothing seems to happen.
dip-poison-weapon = You coat your { $item } with poison.
dip-unholy-water = You dip your { $item } in the unholy water.
dip-unicorn-horn-cure = You feel better.

## ============================================================================
## Djinni / Ghost from bottle
## ============================================================================

djinni-from-bottle = An enormous djinni emerges from the bottle!
ghost-from-bottle = As you open the bottle, something emerges.

## ============================================================================
## Drawbridge
## ============================================================================

drawbridge-destroyed = The drawbridge is destroyed!
drawbridge-lowers = The drawbridge lowers!
drawbridge-raises = The drawbridge raises!
drawbridge-resists = The drawbridge resists!

## ============================================================================
## End game
## ============================================================================

end-ascension-offering = You offer the Amulet of Yendor to { $god }...
end-do-not-pass-go = Do not pass go. Do not collect 200 zorkmids.

## ============================================================================
## Engrave
## ============================================================================

engrave-elbereth = You engrave "Elbereth" into the floor.

## ============================================================================
## Engulf
## ============================================================================

engulf-ejected = You are expelled from { $monster }!
engulf-escape-killed = You kill { $monster } from inside!

## ============================================================================
## Fountain
## ============================================================================

fountain-chill = You feel a sudden chill.
fountain-curse-items = A feeling of loss comes over you.
fountain-dip-curse = The water glows for a moment.
fountain-dip-nothing = Nothing seems to happen.
fountain-dip-uncurse = The water glows for a moment.
fountain-dried-up = The fountain has dried up!
fountain-dries-up = The fountain dries up!
fountain-find-gem = You feel a gem here!
fountain-foul = The water is foul! You gag and vomit.
fountain-gush = Water gushes forth from the overflowing fountain!
fountain-no-position = You can't dip from this position.
fountain-not-here = There is no fountain here.
fountain-nothing = A large bubble rises to the surface and pops.
fountain-poison = The water is contaminated!
fountain-refresh = The cool draught refreshes you.
fountain-see-invisible = You feel self-knowledgeable...
fountain-see-monsters = You feel the presence of evil.
fountain-self-knowledge = You feel self-knowledgeable...
fountain-tingling = A strange tingling runs up your arm.
fountain-water-demon = An endless stream of snakes pours forth!
fountain-water-moccasin = An endless stream of snakes pours forth!
fountain-water-nymph = A wisp of vapor escapes the fountain...
fountain-wish = Grateful for your release, the water demon grants you a wish!

## ============================================================================
## Grave
## ============================================================================

grave-corpse = You find a corpse in the grave.
grave-empty = The grave is unoccupied. Strange...

## ============================================================================
## Guard / Vault
## ============================================================================

guard-halt = "Halt, thief! You're under arrest!"
guard-no-gold = The guard finds no gold on you.
vault-guard-disappear = The guard disappears.
vault-guard-escort = The guard escorts you out.
vault-guard-state-change = The guard changes stance.

## ============================================================================
## Guardian angel
## ============================================================================

guardian-angel-rebukes = Your guardian angel rebukes you!

## ============================================================================
## Hunger
## ============================================================================

hunger-faint = You faint from lack of food.
hunger-starvation = You die from starvation.

## ============================================================================
## Instrument / Music
## ============================================================================

instrument-no-charges = The instrument is out of charges.
play-bugle = You play the bugle.
play-drum = You beat the drum.
play-earthquake = The entire dungeon is shaking around you!
play-horn-noise = You produce a frightful, horrible sound.
play-magic-flute = You produce very attractive music.
play-magic-harp = You produce very attractive music.
play-music = You play some music.
play-nothing = You can't think of anything appropriate to play.

## ============================================================================
## Intrinsics gained
## ============================================================================

intrinsic-acid-res-temp = You feel a momentary tingle.
intrinsic-cold-res = You feel full of hot air.
intrinsic-disint-res = You feel very firm.
intrinsic-fire-res = You feel a momentary chill.
intrinsic-invisibility = You feel rather airy.
intrinsic-poison-res = You feel healthy.
intrinsic-see-invisible = You feel perceptive!
intrinsic-shock-res = Your health currently feels amplified!
intrinsic-sleep-res = You feel wide awake.
intrinsic-stone-res-temp = You feel unusually limber.
intrinsic-strength = You feel strong!
intrinsic-telepathy = You feel a strange mental acuity.
intrinsic-teleport-control = You feel in control of yourself.
intrinsic-teleportitis = You feel very jumpy.

## ============================================================================
## Invoke artifact
## ============================================================================

invoke-no-power = Nothing seems to happen.
invoke-not-wielded = You must be wielding it to invoke its power.

## ============================================================================
## Jumping
## ============================================================================

jump-no-ability = You don't know how to jump.
jump-out-of-range = That spot is too far away!
jump-success = You jump!
jump-too-burdened = You are carrying too much to jump!

## ============================================================================
## Kicking
## ============================================================================

kick-door-held = The door is held shut!
kick-door-open = The door crashes open!
kick-hurt-foot = Ouch! That hurts!
kick-item-blocked = Something blocks your kick.
kick-item-moved = You kick something.
kick-nothing = You kick at empty space.
kick-sink-ring = Something rattles around in the sink.

## ============================================================================
## Knowledge
## ============================================================================

known-nothing = You don't know anything yet.

## ============================================================================
## Levitation
## ============================================================================

levitating-cant-go-down = You are floating high above the floor.
levitating-cant-pickup = You cannot reach the floor.
levitation-float-lower = You float gently to the floor.
levitation-wobble = You wobble in midair.

## ============================================================================
## Light
## ============================================================================

light-extinguished = Your { $item } goes out.
light-lit = Your { $item } is now lit.
light-no-fuel = Your { $item } has no fuel.
light-not-a-source = That is not a light source.

## ============================================================================
## Lizard corpse
## ============================================================================

lizard-cures-confusion = You feel less confused.
lizard-cures-stoning = You feel limber!

## ============================================================================
## Lock
## ============================================================================

lock-already-locked = It is already locked.
lock-door-locked = The door is locked.
lock-force-container-success = You force the lock open!
lock-force-fail = You fail to force the lock.
lock-force-success = You force the lock open!
lock-lockpick-breaks = Your lockpick breaks!
lock-need-key = You need a key to lock this.
lock-no-door = There is no door here.
lock-no-target = There is nothing here to lock or unlock.
lock-pick-container-success = You succeed in picking the lock.
lock-pick-fail = You fail to pick the lock.
lock-pick-success = You succeed in picking the lock.

## ============================================================================
## Lycanthropy
## ============================================================================

lycanthropy-cured = You feel purified.
lycanthropy-full-moon-transform = You feel feverish tonight.
lycanthropy-infected = You feel feverish.

## ============================================================================
## mhitm — Monster vs monster
## ============================================================================

mhitm-passive-stoning = { $monster } turns to stone!

## ============================================================================
## Monster abilities
## ============================================================================

monster-ability-used = { $monster } uses a special ability!
monster-no-ability = You don't have that ability in your current form.
monster-not-polymorphed = You are not polymorphed.
monster-scared-elbereth = { $monster } is scared by the Elbereth engraving!
monster-teleport-near = { $monster } appears from thin air!

## ============================================================================
## Mounting
## ============================================================================

already-mounted = You are already riding.
mount-not-monster = That is not a creature you can ride.
mount-not-tame = That creature is not tame enough to ride.
mount-too-far = That creature is too far away.
mount-too-weak = You are too weak to ride.
not-mounted = You are not riding anything.
steed-stops-galloping = Your steed slows to a halt.
steed-swims = Your steed paddles through the water.

## ============================================================================
## Miscellaneous
## ============================================================================

already-punished = You are already being punished.
call-empty-name = You didn't give a name.
cannot-do-that = You can't do that.
chronicle-empty = Your chronicle is empty.
fire-no-ammo = You have nothing appropriate to fire.
no-fountain-here = There is no fountain here.
not-a-drawbridge = That is not a drawbridge.
not-a-raised-drawbridge = That drawbridge is not raised.
not-carrying-anything = You are not carrying anything.
not-punished = You are not being punished.
not-wearing-that = You are not wearing that.
wait = Time passes...
see-item-here = You see an item here.
see-items-here = You see items here.
scroll-cant-read-blind = You can't read while blind!

## ============================================================================
## Polymorph
## ============================================================================

polymorph-controlled = What monster do you want to turn into?
polymorph-dismount = You can no longer ride your steed.
polymorph-newman-survive = You survive your attempted polymorph.
polymorph-revert = You return to your normal form.
polymorph-system-shock = Your body shudders and undergoes a violent transformation!
polymorph-system-shock-fatal = The system shock from the polymorph kills you!

## ============================================================================
## Potions (additional)
## ============================================================================

potion-acid-resist = Your affinity to acid disappears!
potion-see-invisible-cursed = You thought you saw something.
potion-sickness-mild = Yecch! This stuff tastes like poison.
potion-uneasy = You feel uneasy.

## ============================================================================
## Prayer (additional)
## ============================================================================

pray-angry-curse = You feel that your possessions are less effective.
pray-angry-displeased = You feel that { $god } is displeased.
pray-angry-lose-wis = Your wisdom diminishes.
pray-angry-punished = You are punished for your misbehavior!
pray-angry-summon = { $god } summons hostile monsters!
pray-angry-zap = Suddenly, a bolt of lightning strikes you!
pray-bless-weapon = Your weapon softly glows.
pray-castle-tune = You hear a voice echo: "The passtune sounds like..."
pray-cross-altar-penalty = You have a strange forbidding feeling.
pray-demon-rejected = { $god } is not deterred...
pray-fix-trouble = { $god } fixes your trouble.
pray-gehennom-no-help = { $god } does not seem to be able to reach you in Gehennom.
pray-golden-glow = A golden glow surrounds you.
pray-grant-intrinsic = You feel the power of { $god }.
pray-grant-spell = Divine knowledge fills your mind!
pray-indifferent = { $god } seems indifferent.
pray-moloch-laughter = Moloch laughs at your prayers.
pray-pleased = You feel that { $god } is pleased.
pray-uncurse-all = You feel like someone is helping you.
pray-undead-rebuke = You feel unworthy.

## ============================================================================
## Priest
## ============================================================================

priest-angry = The priest gets angry!
priest-calmed = The priest calms down.
priest-virtues-of-poverty = The priest preaches the virtues of poverty.
priest-wrong-alignment = The priest mutters disapprovingly.

## ============================================================================
## Punishment
## ============================================================================

punishment-applied = You are punished!
punishment-removed = You feel the iron ball disappear.

## ============================================================================
## Quest
## ============================================================================

quest-assigned = Your quest has been assigned.

## ============================================================================
## Region
## ============================================================================

region-fog-obscures = A cloud of fog obscures your vision!

## ============================================================================
## Riding (additional)
## ============================================================================

phaze-feeling-bloated = You feel bloated.
phaze-feeling-flabby = You feel flabby.

## ============================================================================
## Rub / apply
## ============================================================================

rub-lamp-djinni = You rub the lamp and a djinni emerges!
rub-lamp-nothing = Nothing happens.
rub-no-effect = Nothing seems to happen.
rub-touchstone = You rub against the touchstone.

## ============================================================================
## Rump (sit in water)
## ============================================================================

rump-gets-wet = Your rump gets wet.

## ============================================================================
## Sacrifice (additional)
## ============================================================================

sacrifice-alignment-convert = You feel the power of { $god } over you.
sacrifice-altar-convert = The altar is converted!
sacrifice-altar-reject = { $god } rejects your sacrifice!
sacrifice-conversion-rejected = You hear a thunderclap!
sacrifice-nothing = Your sacrifice disappears!
sacrifice-unicorn-insult = { $god } finds your sacrifice insulting.

## ============================================================================
## Sanctum
## ============================================================================

sanctum-be-gone = "Be gone, mortal!"
sanctum-desecrate = You desecrate the high altar!
sanctum-infidel = "You dare enter the sanctum, infidel!"

## ============================================================================
## Scroll (additional)
## ============================================================================

scroll-confuse-cure = You feel less confused.
scroll-confuse-self = You feel confused.
scroll-destroy-armor-disenchant = Your armor is less effective!
scroll-destroy-armor-id = Your armor glows then fades.
scroll-fire-confused = Your scroll erupts in flame!
scroll-genocide-reverse = You create a swarm of monsters!
scroll-genocide-reverse-self = You feel a change coming over you.
scroll-identify-self = You feel self-knowledgeable...

## ============================================================================
## Sickness
## ============================================================================

sick-deaths-door = You are at death's door.
sick-illness-severe = You feel deathly sick.

## ============================================================================
## Sitting
## ============================================================================

sit-already-riding = You are already riding something.
sit-in-water = You sit in the water.
sit-no-seats = There is nothing here to sit on.
sit-on-air = Having fun sitting on the air?
sit-on-altar = You sit on the altar.
sit-on-floor = Having fun sitting on the floor?
sit-on-grave = You sit on the headstone.
sit-on-ice = The ice feels cold.
sit-on-lava = The lava burns you!
sit-on-sink = You sit on the sink.
sit-on-stairs = You sit on the stairs.
sit-on-throne = You feel a strange sensation.
sit-tumble-in-place = You tumble in place.

## ============================================================================
## Slime
## ============================================================================

slime-burned-away = The slime is burned away!
sliming-become-slime = You have become a green slime!
sliming-limbs-oozy = Your limbs are getting oozy.
sliming-skin-peeling = Your skin begins to peel.
sliming-turning-green = You are turning a little green.
sliming-turning-into = You are turning into a green slime!

## ============================================================================
## Sleep / Drowsy
## ============================================================================

sleepy-yawn = You yawn.

## ============================================================================
## Spell messages (additional)
## ============================================================================

spell-aggravation = You feel as if something is very angry.
spell-book-full = You know too many spells already.
spell-cancellation-hit = { $target } is covered by a shimmering light!
spell-cancellation-miss = You miss { $target }.
spell-cast-fail = You fail to cast the spell correctly.
spell-cause-fear-none = No monsters are frightened.
spell-charm-monster-hit = { $target } is charmed!
spell-charm-monster-miss = { $target } resists!
spell-clairvoyance = You sense your surroundings.
spell-confuse-monster-hit = { $target } seems confused!
spell-confuse-monster-miss = { $target } resists!
spell-confuse-monster-touch = Your hands begin to glow red.
spell-create-familiar = A familiar creature appears!
spell-create-monster = A monster appears!
spell-cure-blindness-not-blind = You aren't blind.
spell-cure-sickness-not-sick = You aren't sick.
spell-curse-items = You feel as if you need an exorcist.
spell-destroy-armor = Your armor crumbles away!
spell-detect-monsters-none = You don't sense any monsters.
spell-detect-unseen-none = You don't sense any unseen things.
spell-dig-nothing = The digging ray has no effect here.
spell-drain-life-hit = { $target } suddenly seems weaker!
spell-drain-life-miss = You miss { $target }.
spell-finger-of-death-kill = { $target } dies!
spell-finger-of-death-resisted = { $target } resists!
spell-haste-self = You feel yourself moving more quickly.
spell-healing = You feel better.
spell-identify = You feel self-knowledgeable...
spell-insufficient-power = You don't have enough energy to cast that spell.
spell-invisibility = You feel rather airy.
spell-jumping = You jump!
spell-jumping-blocked = Something blocks your jump.
spell-knock = A door opens!
spell-light = A lit field surrounds you!
spell-magic-mapping = A map of your surroundings appears!
spell-need-direction = In what direction?
spell-no-spellbook = You don't have any spellbooks to study.
spell-polymorph-hit = { $target } undergoes a transformation!
spell-polymorph-miss = You miss { $target }.
spell-protection-disappears = Your golden glow fades.
spell-protection-less-dense = Your golden haze becomes less dense.
spell-remove-curse = You feel like someone is helping you.
spell-restore-ability-nothing = You feel momentarily refreshed.
spell-restore-ability-restored = Wow! This makes you feel great!
spell-sleep-hit = { $target } falls asleep!
spell-sleep-miss = { $target } resists!
spell-slow-monster-hit = { $target } seems to slow down.
spell-slow-monster-miss = { $target } resists!
spell-stone-to-flesh-cured = You feel limber!
spell-stone-to-flesh-nothing = Nothing happens.
spell-summon-insects = You summon insects!
spell-summon-monster = You summon a monster!
spell-teleport-away-hit = { $target } disappears!
spell-teleport-away-miss = { $target } resists!
spell-turn-undead-hit = { $target } flees!
spell-turn-undead-miss = { $target } resists!
spell-unknown = You don't know that spell.
spell-weaken = { $target } suddenly seems weaker!
spell-wizard-lock = A door locks shut!

## ============================================================================
## Stairs
## ============================================================================

stairs-at-top = You are at the top of the dungeon.
stairs-not-here = You don't see any stairs here.

## ============================================================================
## Status effects (additional)
## ============================================================================

status-blindness-end = You can see again.
status-confusion-end = You feel less confused now.
status-fall-asleep = You fall asleep.
status-fumble-trip = You trip over something.
status-fumbling-end = You feel less clumsy.
status-fumbling-start = You feel clumsy.
status-hallucination-end = Everything looks SO boring now.
status-invisibility-end = You are no longer invisible.
status-levitation-end = You float gently to the floor.
status-paralysis-end = You can move again.
status-paralyzed-cant-move = You can't move!
status-sick-cured = What a relief!
status-sick-recovered = You feel better.
status-sleepy-end = You feel awake.
status-sleepy-start = You feel drowsy.
status-speed-end = You feel yourself slow down.
status-stun-end = You feel less stunned now.
status-vomiting-end = You feel less nauseated now.
status-vomiting-start = You feel nauseated.
status-wounded-legs-healed = Your legs feel better.
status-wounded-legs-start = Your legs are in bad shape!

## ============================================================================
## Steal
## ============================================================================

steal-item-from-you = { $monster } steals { $item }!
steal-no-gold = { $monster } finds no gold on you.
steal-nothing-to-take = { $monster } finds nothing to steal.

## ============================================================================
## Stoning
## ============================================================================

stoning-limbs-stiffening = Your limbs are stiffening.
stoning-limbs-stone = Your limbs have turned to stone.
stoning-slowing-down = You are slowing down.
stoning-turned-to-stone = You have turned to stone.
stoning-you-are-statue = You are a statue.

## ============================================================================
## Swap weapons
## ============================================================================

swap-no-secondary = You have no secondary weapon.
swap-success = You swap your weapons.
swap-welded = Your weapon is welded to your hand!

## ============================================================================
## Swimming
## ============================================================================

swim-lava-burn = You burn to a crisp in the lava!
swim-water-paddle = You paddle in the water.

## ============================================================================
## Temple
## ============================================================================

temple-eerie = You have an eerie feeling...
temple-ghost-appears = A ghost appears before you!
temple-shiver = You shiver.
temple-watched = You feel as though someone is watching you.

## ============================================================================
## Throne
## ============================================================================

throne-genocide = A voice echoes: "Thou shalt choose who lives and who dies!"
throne-identify = You feel self-knowledgeable...
throne-no-position = You can't sit there from this position.
throne-not-here = There is no throne here.
throne-nothing = Nothing seems to happen.
throne-vanishes = The throne vanishes in a puff of logic.
throne-wish = A voice echoes: "Thy wish is granted!"

## ============================================================================
## Tip container
## ============================================================================

tip-cannot-reach = You can't reach that.
tip-empty = That container is empty.
tip-locked = That container is locked.

## ============================================================================
## Tool messages (additional)
## ============================================================================

tool-bell-cursed-summon = The bell summons hostile creatures!
tool-bell-cursed-undead = The bell summons undead!
tool-bell-no-sound = But the sound is muffled.
tool-bell-opens = Something opens...
tool-bell-reveal = Things open around you...
tool-bell-ring = The bell rings.
tool-bell-wake-nearby = The ringing wakes nearby monsters!
tool-bullwhip-crack = You crack the bullwhip!
tool-camera-no-target = There is nothing to photograph.
tool-candelabrum-extinguish = The candelabrum's candles are extinguished.
tool-candelabrum-no-candles = The candelabrum has no candles attached.
tool-candle-extinguish = You extinguish the candle.
tool-candle-light = You light the candle.
tool-cream-pie-face = You get cream pie on your face!
tool-dig-no-target = There is nothing to dig here.
tool-drum-earthquake = The entire dungeon is shaking around you!
tool-drum-no-charges = The drum is out of charges.
tool-drum-thump = You beat the drum.
tool-figurine-hostile = The figurine transforms into a hostile monster!
tool-figurine-peaceful = The figurine transforms into a peaceful monster.
tool-figurine-tame = The figurine transforms into a pet!
tool-grease-empty = The can of grease is empty.
tool-grease-hands = Your hands are too slippery to hold anything!
tool-grease-slip = Your greased { $item } slips off!
tool-horn-no-charges = The horn is out of charges.
tool-horn-toot = You produce a frightful, horrible sound.
tool-leash-no-pet = There is no pet nearby to leash.
tool-lockpick-breaks = Your lockpick breaks!
tool-magic-lamp-djinni = A djinni emerges from the lamp!
tool-magic-whistle = You produce a strange whistling sound.
tool-mirror-self = You look ugly in the mirror.
tool-no-locked-door = There is no locked door there.
tool-nothing-happens = Nothing seems to happen.
tool-polearm-no-target = There is nothing to hit there.
tool-saddle-no-mount = There is nothing to put a saddle on.
tool-tin-whistle = You produce a high whistling sound.
tool-tinning-no-corpse = There is no corpse to tin here.
tool-touchstone-identify = You identify the gems by rubbing them on the touchstone.
tool-touchstone-shatter = The gem shatters!
tool-touchstone-streak = The gem leaves a streak on the touchstone.
tool-towel-cursed-gunk = Your face is covered with gunk!
tool-towel-cursed-nothing = You can't get the gunk off!
tool-towel-cursed-slimy = Your face feels slimy.
tool-towel-nothing = Your face is already clean.
tool-towel-wipe-face = You've got the glop off.
tool-unihorn-cured = You feel better!
tool-unihorn-cursed = The unicorn horn is cursed!
tool-unihorn-nothing = Nothing seems to happen.
tool-unlock-fail = You fail to unlock it.
tool-unlock-success = You unlock it.
tool-whistle-no-pets = You don't have any pets nearby.

## ============================================================================
## Tunnel
## ============================================================================

tunnel-blocked = There is no room to tunnel here.

## ============================================================================
## Turn undead
## ============================================================================

turn-no-undead = There are no undead to turn.
turn-not-clerical = You don't know how to turn undead.

## ============================================================================
## Untrap
## ============================================================================

untrap-failed = You fail to disarm the trap.
untrap-no-trap = You don't find any traps.
untrap-success = You disarm the trap!
untrap-triggered = You triggered the trap!

## ============================================================================
## Vanquished
## ============================================================================

vanquished-none = No monsters have been vanquished yet.

## ============================================================================
## Vomiting
## ============================================================================

vomiting-about-to = You are about to vomit.
vomiting-cant-think = You can't think straight.
vomiting-incredibly-sick = You feel incredibly sick.
vomiting-mildly-nauseated = You feel mildly nauseated.
vomiting-slightly-confused = You feel slightly confused.
vomiting-vomit = You vomit!

## ============================================================================
## Wand (additional)
## ============================================================================

wand-cancel-monster = { $target } is covered by a shimmering light!
wand-digging-miss = The digging beam misses.

## ============================================================================
## Wipe
## ============================================================================

wipe-cream-off = You wipe the cream off your face.
wipe-cursed-towel = The towel is cursed!
wipe-nothing = There is nothing to wipe off.

## ============================================================================
## Wizard (Wizard of Yendor)
## ============================================================================

wizard-curse-items = You feel as if you need an exorcist.
wizard-detect-monsters = You feel as if something is watching you.
wizard-detect-objects = You sense the presence of objects.
wizard-detect-traps = You feel warned about nearby traps.
wizard-double-trouble = "Double Trouble..."
wizard-identify-all = You feel self-knowledgeable...
wizard-map-revealed = An image of your surroundings forms in your mind!
wizard-steal-amulet = The Wizard of Yendor steals the Amulet!
wizard-summon-nasties = New nasties appear from thin air!

## ============================================================================
## Worm
## ============================================================================

worm-grows = The long worm grows longer!
worm-shrinks = The long worm shrinks!

## ============================================================================
## Worn items (additional)
## ============================================================================

worn-gauntlets-power-off = You feel weaker.
worn-gauntlets-power-on = You feel stronger!
worn-helm-brilliance-off = You feel ordinary.
worn-helm-brilliance-on = You feel brilliant!

## ============================================================================
## Write
## ============================================================================

write-no-marker = You don't have a magic marker.
write-not-enough-charges = Your marker is too dry to write that!
write-scroll-fail-daiyen-fansen = Your marker dries out!
write-spellbook-fail = The spellbook warps strangely, then turns blank.
write-spellbook-success = You successfully write the spellbook!

## ============================================================================
## God lightning bolt
## ============================================================================

god-lightning-bolt = Suddenly, a bolt of lightning strikes you!

## ============================================================================
## Wizard mode (debug)
## ============================================================================

wizard-detect-all = You sense everything around you.
wizard-kill = You die.
wizard-where = You sense where everything is.
