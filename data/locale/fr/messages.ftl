## NetHack Babel — French message catalog
## Fluent (.ftl) format — https://projectfluent.org/

## ============================================================================
## Item naming — BUC labels (gender-agreeing)
## ============================================================================

item-buc-blessed = { $gender ->
    [feminine] bénie
   *[masculine] béni
}
item-buc-uncursed = { $gender ->
    [feminine] non maudite
   *[masculine] non maudit
}
item-buc-cursed = { $gender ->
    [feminine] maudite
   *[masculine] maudit
}

## ============================================================================
## Item naming — erosion adjectives (gender-agreeing)
## ============================================================================

item-erosion-rusty = { $gender ->
    [feminine] rouillée
   *[masculine] rouillé
}
item-erosion-very-rusty = { $gender ->
    [feminine] très rouillée
   *[masculine] très rouillé
}
item-erosion-thoroughly-rusty = { $gender ->
    [feminine] complètement rouillée
   *[masculine] complètement rouillé
}
item-erosion-corroded = { $gender ->
    [feminine] corrodée
   *[masculine] corrodé
}
item-erosion-very-corroded = { $gender ->
    [feminine] très corrodée
   *[masculine] très corrodé
}
item-erosion-thoroughly-corroded = { $gender ->
    [feminine] complètement corrodée
   *[masculine] complètement corrodé
}
item-erosion-burnt = { $gender ->
    [feminine] brûlée
   *[masculine] brûlé
}
item-erosion-very-burnt = { $gender ->
    [feminine] très brûlée
   *[masculine] très brûlé
}
item-erosion-thoroughly-burnt = { $gender ->
    [feminine] complètement brûlée
   *[masculine] complètement brûlé
}
item-erosion-rotted = { $gender ->
    [feminine] pourrie
   *[masculine] pourri
}
item-erosion-very-rotted = { $gender ->
    [feminine] très pourrie
   *[masculine] très pourri
}
item-erosion-thoroughly-rotted = { $gender ->
    [feminine] complètement pourrie
   *[masculine] complètement pourri
}
item-erosion-rustproof = anti-rouille
item-erosion-fireproof = ignifuge
item-erosion-corrodeproof = anti-corrosion
item-erosion-rotproof = imputrescible

## ============================================================================
## Item naming — class-specific base name patterns
## ============================================================================

item-potion-identified = potion de { $name }
item-potion-called = potion appelée { $called }
item-potion-appearance = potion { $appearance }
item-potion-generic = potion

item-scroll-identified = parchemin de { $name }
item-scroll-called = parchemin appelé { $called }
item-scroll-labeled = parchemin étiqueté { $label }
item-scroll-appearance = parchemin { $appearance }
item-scroll-generic = parchemin

item-wand-identified = baguette de { $name }
item-wand-called = baguette appelée { $called }
item-wand-appearance = baguette { $appearance }
item-wand-generic = baguette

item-ring-identified = anneau de { $name }
item-ring-called = anneau appelé { $called }
item-ring-appearance = anneau { $appearance }
item-ring-generic = anneau

item-amulet-called = amulette appelée { $called }
item-amulet-appearance = amulette { $appearance }
item-amulet-generic = amulette

item-spellbook-identified = grimoire de { $name }
item-spellbook-called = grimoire appelé { $called }
item-spellbook-appearance = grimoire { $appearance }
item-spellbook-generic = grimoire

item-gem-stone = pierre
item-gem-gem = gemme
item-gem-called-stone = pierre appelée { $called }
item-gem-called-gem = gemme appelée { $called }
item-gem-appearance-stone = pierre { $appearance }
item-gem-appearance-gem = gemme { $appearance }

item-generic-called = { $base } appelé { $called }

## ============================================================================
## Item naming — connectors and suffixes
## ============================================================================

item-named-suffix = { $gender ->
    [feminine] nommée { $name }
   *[masculine] nommé { $name }
}

## ============================================================================
## Item naming — articles (gender+case aware)
## ============================================================================

item-article-indefinite = { $gender ->
    [feminine] une
   *[masculine] un
}
item-article-definite = { $gender ->
    [feminine] la
   *[masculine] le
}
item-article-your = { $gender ->
    [feminine] votre
   *[masculine] votre
}

## ============================================================================
## Item naming — plural selection (CLDR-aware)
## ============================================================================

item-count-name = { $count ->
    [one] { $singular }
   *[other] { $plural }
}

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
## Menu d'options
## ============================================================================

ui-options-title = Options
ui-options-game = Paramètres de jeu
ui-options-display = Paramètres d'affichage
ui-options-sound = Paramètres sonores

## Options de jeu

opt-autopickup = Ramassage automatique
opt-autopickup-types = Types de ramassage
opt-legacy = Introduction narrative

## Options d'affichage

opt-map-colors = Couleurs de la carte
opt-message-colors = Couleurs des messages
opt-buc-highlight = Surbrillance BUC
opt-minimap = Mini-carte
opt-mouse-hover = Info au survol
opt-nerd-fonts = Polices Nerd

## Options sonores

opt-sound-enabled = Son activé
opt-volume = Volume

## Valeurs d'option

opt-on = OUI
opt-off = NON

## ============================================================================
## Récit d'introduction (legacy)
## ============================================================================

legacy-intro =
    Il est écrit dans le Livre de { $deity } :

        Après la Création, le cruel dieu Moloch se rebella
        contre l'autorité de Marduk le Créateur.
        Moloch vola à Marduk le plus puissant de tous
        les artefacts des dieux, l'Amulette de Yendor,
        et il la cacha dans les cavités obscures de la Géhenne,
        le Monde Souterrain, où il se terre encore,
        attendant son heure.

    Votre { $deity } cherche à posséder l'Amulette, et avec elle
    à obtenir l'ascendance méritée sur les autres dieux.

    Vous, un(e) { $role } fraîchement formé(e), avez été
    annoncé(e) dès la naissance comme l'instrument de { $deity }.
    Vous êtes destiné(e) à récupérer l'Amulette pour votre
    divinité, ou à mourir en essayant. Votre heure est venue.
    Pour nous tous : Allez bravement avec { $deity } !

## ============================================================================
## Interface — aide
## ============================================================================

help-title = Aide de NetHack Babel
help-move = Déplacez-vous avec hjklyubn ou les touches fléchées.
help-attack = Marchez vers un monstre pour l'attaquer.
help-wait = Appuyez sur . ou s pour attendre un tour.
help-search = Appuyez sur s pour chercher des choses cachées.
help-inventory = Appuyez sur i pour voir votre inventaire.
help-pickup = Appuyez sur , pour ramasser des objets.
help-drop = Appuyez sur d pour lâcher des objets.
help-stairs-up = Appuyez sur < pour monter les escaliers.
help-stairs-down = Appuyez sur > pour descendre les escaliers.
help-eat = Appuyez sur e pour manger.
help-quaff = Appuyez sur q pour boire une potion.
help-read = Appuyez sur r pour lire un parchemin ou grimoire.
help-wield = Appuyez sur w pour manier une arme.
help-wear = Appuyez sur W pour porter une armure.
help-remove = Appuyez sur T pour retirer une armure.
help-zap = Appuyez sur z pour utiliser une baguette.

# Aide — diagramme de mouvement
help-move-diagram =
    {"  "}y k u     NO  N  NE
    {"  "}h . l      O  .   E
    {"  "}b j n     SO  S  SE

# Aide — symboles
help-symbols-title = Symboles :
help-symbol-player = @  = vous (le joueur)
help-symbol-floor = .  = sol
help-symbol-corridor = #  = couloir
help-symbol-door-closed = +  = porte fermée
help-symbol-door-open = |  = porte ouverte
help-symbol-stairs-up = <  = escalier montant
help-symbol-stairs-down = >  = escalier descendant
help-symbol-water = {"}"}  = eau/lave
help-symbol-fountain = {"{"} = fontaine

# Aide — commandes supplémentaires
help-options = Appuyez sur O pour les options.
help-look = Appuyez sur : pour regarder le sol.
help-history = Appuyez sur Ctrl+P pour l'historique.
help-shift-run = Shift+direction = courir.
help-arrows = Les touches fléchées fonctionnent aussi.

## ============================================================================
## Interface — inventaire
## ============================================================================

ui-inventory-title = Inventaire
ui-inventory-empty = Vous ne portez rien.

inv-class-weapon = Armes
inv-class-armor = Armures
inv-class-ring = Anneaux
inv-class-amulet = Amulettes
inv-class-tool = Outils
inv-class-food = Nourriture
inv-class-potion = Potions
inv-class-scroll = Parchemins
inv-class-spellbook = Grimoires
inv-class-wand = Baguettes
inv-class-coin = Pièces
inv-class-gem = Gemmes/Pierres
inv-class-rock = Rochers
inv-class-ball = Boulets
inv-class-chain = Chaînes
inv-class-venom = Venin
inv-class-other = Autre

# Marqueurs BUC d'inventaire
inv-buc-marker-blessed = [B]
inv-buc-marker-cursed = [M]
inv-buc-tag-blessed = (béni)
inv-buc-tag-cursed = (maudit)
inv-buc-tag-uncursed = (non maudit)
ui-pickup-title = Ramasser quoi ?

# Messages TUI courants
ui-never-mind = Tant pis.
ui-no-such-item = Vous n'avez pas cet objet.
ui-not-implemented = Pas encore implémenté.
ui-empty-handed = Vous êtes les mains vides.

## ============================================================================
## Dispatch d'actions
## ============================================================================

eat-generic = Vous mangez la nourriture.
eat-what = Manger quoi ?
quaff-generic = Vous buvez la potion.
quaff-what = Boire quoi ?
read-generic = Vous lisez le parchemin.
read-what = Lire quoi ?
quest-expelled = You are not yet permitted to descend into the quest.
quest-completed = Your quest is complete.
quest-leader-first = { $leader } greets you and weighs your worth.
quest-leader-next = { $leader } studies you again, deciding whether you are ready.
quest-leader-assigned = { $leader } reminds you to defeat { $nemesis }.
quest-leader-nemesis-dead = { $leader } recognizes your return with { $artifact }.
quest-leader-reject = { $leader } rejects you as { $reason }.
quest-guardian = { $guardian } warns you to stay true to your quest.
quest-nemesis-first = { $nemesis } bars your path.
quest-nemesis-next = { $nemesis } is still waiting for you.
quest-nemesis-artifact = { $nemesis } snarls at the sight of the quest artifact.
quest-nemesis-dead = The stench of { $nemesis }'s defeat hangs in the air.
invocation-complete = The invocation ritual succeeds and a magic portal opens!
invocation-incomplete = The runes flare, but the invocation does not complete.
invocation-missing-bell = The ritual falters without the Bell of Opening.
invocation-missing-candelabrum = The ritual falters without the Candelabrum of Invocation.
invocation-needs-bell-rung = The Bell of Opening must be rung here before the ritual can begin.
invocation-needs-candelabrum-ready = The Candelabrum of Invocation must be lit with seven candles.
invocation-items-cursed = The cursed invocation items twist the ritual out of shape.
read-dead-book = The Book of the Dead whispers with sepulchral power.
zap-generic = Vous utilisez la baguette.

## Portes
door-open-success = La porte s'ouvre.
door-locked = Cette porte est verrouillée.
door-already-open = Cette porte est déjà ouverte.
door-not-here = Il n'y a pas de porte ici.
door-close-success = La porte se ferme.
door-already-closed = Cette porte est déjà fermée.

## Serrures
lock-nothing-to-force = Il n'y a rien à forcer ici.

## Prière
pray-begin = Vous commencez à prier votre dieu...

## Offrandes
offer-generic = Vous faites une offrande sur l'autel.
offer-amulet-rejected = L'Amulette est rejetée et retombe près de vous !
offer-what = Offrir quoi ?

## Discussion
npc-chat-no-response = La créature ne semble pas vouloir discuter.
npc-chat-sleeping = La créature ne semble même pas vous remarquer.
chat-nobody-there = Il n'y a personne à qui parler.

## Déplacement / voyage
peaceful-monster-blocks = Vous vous arrêtez. { $monster } vous bloque le passage.
ride-not-available = Il n'y a rien à chevaucher ici.
enhance-not-available = Vous ne pouvez améliorer aucune compétence pour le moment.
enhance-success = Vous améliorez { $skill } au niveau { $level }.
travel-not-implemented = Le voyage n'est pas encore disponible.
two-weapon-not-implemented = Le combat à deux armes n'est pas encore disponible.
two-weapon-enabled = Vous commencez à combattre avec deux armes.
two-weapon-disabled = Vous cessez de combattre avec deux armes.
name-not-implemented = Le nommage n'est pas encore disponible.
adjust-not-implemented = L'ajustement d'inventaire n'est pas encore disponible.

## Mode assistant (debug)
wizard-identify-all = Vous connaissez soudain tout votre équipement.
wizard-map-revealed = Une image de votre environnement se forme dans votre esprit !
wizard-vague-nervous = Vous vous sentez vaguement nerveux.
wizard-black-glow = Vous remarquez une lueur noire autour de vous.
wizard-aggravate = Des bruits lointains résonnent tandis que le donjon s’éveille brusquement.
wizard-respawned = Le Magicien de Yendor revient à la vie !
wizard-respawned-boom = Une voix retentit...
wizard-respawned-taunt = Tu pensais donc pouvoir me {$verb}, imbécile.
wizard-steal-amulet = Le Magicien de Yendor vole l'Amulette !
wizard-summon-nasties = De nouvelles horreurs apparaissent de nulle part !
wizard-taunt-laughs = { $wizard } éclate d'un rire démoniaque.
wizard-taunt-relinquish = Abandonne l'Amulette, { $insult } !
wizard-taunt-panic = Déjà ta force vitale s'épuise, { $insult } !
wizard-taunt-return = Je reviendrai.
wizard-taunt-general = { $malediction }, { $insult } !
amulet-feels-hot = L'Amulette est brûlante !
amulet-feels-very-warm = L'Amulette est très chaude.
amulet-feels-warm = L'Amulette est chaude.
wizard-detect-all = Vous sentez tout autour de vous.
wizard-genesis = Un { $monster } apparaît à côté de vous.
wizard-genesis-failed = Rien ne répond à votre demande de { $monster }.
wizard-wish = Votre souhait est exaucé : { $item }.
wizard-wish-adjusted = Votre souhait est ajusté en : { $item }.
wizard-wish-floor = Votre souhait est exaucé : { $item } tombe à vos pieds.
wizard-wish-adjusted-floor = Votre souhait est ajusté : { $item } tombe à vos pieds.
wizard-wish-failed = Rien ne répond à votre souhait de « { $wish } ».
wizard-kill = Vous effacez { $count } monstre(s) sur ce niveau.
wizard-kill-none = Il n'y a aucun monstre ici à effacer.
wizard-where-current = Vous êtes sur { $location } (profondeur absolue { $absolute }) en { $x },{ $y }.
wizard-where-special = { $level } se trouve sur { $location }.

## ============================================================================
## Quêtes / Dialogue PNJ
## ============================================================================

quest-leader-greeting = Bienvenue, { $role }. Je vous attendais.
quest-assignment =
    Écoutez bien, { $role }. Le { $nemesis } a volé { $artifact }.
    Vous devez descendre dans les profondeurs pour le récupérer.
    Notre destin dépend de vous.

## Marchand
shop-welcome = Bienvenue dans la boutique de { $shopkeeper } — { $shoptype } !
shop-buy-prompt = { $shopkeeper } dit : « Ce sera en espèces ou à crédit ? »
shop-unpaid-warning = { $shopkeeper } dit : « Vous avez des articles impayés ! »
shop-theft-warning = { $shopkeeper } crie : « Au voleur ! Payez immédiatement ! »

## Prêtre
priest-welcome = Le prêtre prononce une bénédiction sur vous.
priest-protection-offer = Le prêtre offre la protection divine pour { $cost } pièces d'or.
priest-donation-thanks = Le prêtre vous remercie pour votre généreuse donation.
priest-cranky-1 = Le prêtre rétorque : « Tu veux parler ? Je vais te dire deux mots ! »
priest-cranky-2 = Le prêtre gronde : « Parler ? Voici ce que j'ai à dire ! »
priest-cranky-3 = Le prêtre dit : « Pèlerin, je n'ai plus rien à te dire. »

## ============================================================================
## Contenu (rumeurs, oracles)
## ============================================================================

rumor-fortune-cookie = Vous ouvrez le biscuit de fortune. Il dit : « { $rumor } »
oracle-consultation = { $text }

## ============================================================================
## Identification de symboles
## ============================================================================

whatis-prompt = Que voulez-vous identifier ? (choisissez une position)
whatis-terrain = { $description } (terrain)
whatis-monster = { $description } (monstre)
whatis-object = { $description } (objet)
whatis-nothing = Vous ne voyez rien de spécial là.

## Découvertes
discoveries-title = Découvertes
discoveries-empty = Vous n'avez encore rien découvert.
guardian-angel-appears = Un ange gardien apparaît à vos côtés !
temple-enter = Vous entrez dans un temple de { $god }.
temple-forbidding = Une sainteté austère emplit les lieux.
temple-peace = Une paix profonde envahit le temple.
temple-unusual-peace = Le temple paraît étrangement paisible.
temple-eerie = Vous avez un étrange pressentiment...
temple-ghost-appears = Un fantôme apparaît devant vous !
temple-shiver = Vous frissonnez.
temple-watched = Vous avez l'impression que quelqu'un vous observe.
sanctum-infidel = « Vous osez entrer dans le sanctuaire, infidèle ! »
sanctum-be-gone = « Hors d'ici, mortel ! »
sanctum-desecrate = Vous profanez le grand autel !
priest-ale-gift = Le prêtre vous donne { $amount } pièces d'or pour aller boire.
priest-cheapskate = Le prêtre considère votre maigre offrande avec scepticisme.
priest-small-thanks = Le prêtre vous remercie pour le peu que vous pouvez offrir.
priest-pious = Le prêtre dit que vous êtes véritablement pieux.
priest-clairvoyance = Le prêtre vous accorde un instant de lucidité.
status-clairvoyance-end = Votre clairvoyance s'estompe.
priest-selfless-generosity = Le prêtre apprécie profondément votre générosité désintéressée.
priest-cleansing = La bénédiction du prêtre allège votre fardeau spirituel.
priest-not-enough-gold = Le prêtre demande { $cost } pièces d'or.
priest-protection-granted = Le prêtre vous accorde une protection divine pour { $cost } pièces d'or.
shk-welcome = { $shopkeeper } dit : « Bienvenue dans ma boutique, { $honorific }. »
shk-angry-greeting = { $shopkeeper } vous lance un regard furieux.
shk-follow-reminder = { $shopkeeper } dit : « Bonjour, { $honorific } ! N'avez-vous pas oublié de payer ? »
shk-bill-total = { $shopkeeper } dit que votre note s'élève à { $amount } pièces d'or.
shk-debit-reminder = { $shopkeeper } vous rappelle que vous devez { $amount } pièces d'or.
shk-credit-reminder = { $shopkeeper } vous encourage à utiliser vos { $amount } pièces d'or de crédit.
shk-robbed-greeting = { $shopkeeper } dit : « Je n'ai pas oublié ce vol, { $honorific }. »
shk-surcharge-greeting = { $shopkeeper } dit : « Les prix sont plus élevés pour vous maintenant, { $honorific }. »
shk-business-bad = { $shopkeeper } se plaint que les affaires vont mal.
shk-business-good = { $shopkeeper } dit que les affaires vont bien.
shk-shoplifters = { $shopkeeper } parle du problème des voleurs à l'étalage.
shk-geico-pitch = { $shopkeeper } says: "Fifteen minutes could save you fifteen zorkmids."
npc-laugh-giggles = { $monster } giggles.
npc-laugh-chuckles = { $monster } chuckles.
npc-laugh-snickers = { $monster } snickers.
npc-laugh-laughs = { $monster } laughs.
npc-gecko-geico-pitch = { $monster } says: "Fifteen minutes could save you fifteen zorkmids."
npc-mumble-incomprehensible = { $monster } mumbles incomprehensibly.
npc-bones-rattle = { $monster } rattles noisily.
npc-shriek = { $monster } shrieks.
npc-bark-barks = { $monster } barks.
npc-bark-whines = { $monster } whines.
npc-bark-howls = { $monster } howls.
npc-bark-yips = { $monster } yips.
npc-mew-mews = { $monster } mews.
npc-mew-yowls = { $monster } yowls.
npc-mew-meows = { $monster } meows.
npc-mew-purrs = { $monster } purrs.
npc-growl-growls = { $monster } growls!
npc-growl-snarls = { $monster } snarls.
npc-roar-roars = { $monster } roars!
npc-bellow-bellows = { $monster } bellows!
npc-squeak-squeaks = { $monster } squeaks.
npc-squawk-squawks = { $monster } squawks.
npc-squawk-nevermore = { $monster } says: "Nevermore!"
npc-chirp-chirps = { $monster } chirps.
npc-hiss-hisses = { $monster } hisses!
npc-buzz-drones = { $monster } drones.
npc-buzz-angry = { $monster } buzzes angrily.
npc-grunt-grunts = { $monster } grunts.
npc-neigh-neighs = { $monster } neighs.
npc-neigh-whinnies = { $monster } whinnies.
npc-neigh-whickers = { $monster } whickers.
npc-humanoid-threatens = { $monster } threatens you.
npc-humanoid-avoid = { $monster } wants nothing to do with you.
npc-humanoid-moans = { $monster } moans.
npc-humanoid-huh = { $monster } says: "Huh?"
npc-humanoid-what = { $monster } says: "What?"
npc-humanoid-eh = { $monster } says: "Eh?"
npc-humanoid-cant-see = { $monster } says: "I can't see!"
npc-humanoid-trapped = { $monster } says: "I'm trapped!"
npc-humanoid-healing = { $monster } asks for a potion of healing.
npc-humanoid-hungry = { $monster } says: "I'm hungry."
npc-humanoid-curses-orcs = { $monster } curses orcs.
npc-humanoid-mining = { $monster } talks about mining.
npc-humanoid-spellcraft = { $monster } talks about spellcraft.
npc-humanoid-hunting = { $monster } discusses hunting.
npc-humanoid-gnome = { $monster } says: "Many enter the dungeon, and few return to the sunlit lands."
npc-humanoid-one-ring = { $monster } asks you about the One Ring.
npc-humanoid-aloha = { $monster } says: "Aloha."
npc-humanoid-spelunker-today = { $monster } describes a recent article in "Spelunker Today" magazine.
npc-humanoid-dungeon-exploration = { $monster } discusses dungeon exploration.
npc-boast-gem-collection = { $monster } boasts about its gem collection.
npc-boast-mutton = { $monster } complains about a diet of mutton.
npc-boast-fee-fie-foe-foo = { $monster } shouts "Fee Fie Foe Foo!" and guffaws.
npc-arrest-facts-maam = { $monster } says: "Just the facts, Ma'am."
npc-arrest-facts-sir = { $monster } says: "Just the facts, Sir."
npc-arrest-anything-you-say = { $monster } says: "Anything you say can be used against you."
npc-arrest-under-arrest = { $monster } says: "You're under arrest!"
npc-arrest-stop-law = { $monster } says: "Stop in the name of the Law!"
npc-djinni-no-wishes = { $monster } says: "Sorry, I'm all out of wishes."
npc-djinni-free = { $monster } says: "I'm free!"
npc-djinni-get-me-out = { $monster } says: "Get me out of here."
npc-djinni-disturb = { $monster } says: "This will teach you not to disturb me!"
npc-cuss-curses = { $monster } curses.
npc-cuss-imprecates = { $monster } imprecates.
npc-cuss-not-too-late = { $monster } says: "It's not too late."
npc-cuss-doomed = { $monster } says: "We're all doomed."
npc-spell-cantrip = { $monster } seems to mutter a cantrip.
npc-seduce-hello-sailor = { $monster } says: "Hello, sailor."
npc-seduce-comes-on = { $monster } comes on to you.
npc-seduce-cajoles = { $monster } cajoles you.
npc-nurse-put-weapon-away = { $monster } says: "Put that weapon away before you hurt someone!"
npc-nurse-doc-cooperate = { $monster } says: "Doc, I can't help you unless you cooperate."
npc-nurse-please-undress = { $monster } says: "Please undress so I can examine you."
npc-nurse-take-off-shirt = { $monster } says: "Take off your shirt, please."
npc-nurse-relax = { $monster } says: "Relax, this won't hurt a bit."
npc-guard-drop-gold = { $monster } says: "Please drop that gold and follow me."
npc-guard-follow-me = { $monster } says: "Please follow me."
npc-soldier-pay = { $monster } says: "What lousy pay we're getting here!"
npc-soldier-food = { $monster } says: "The food's not fit for Orcs!"
npc-soldier-feet = { $monster } says: "My feet hurt, I've been on them all day!"
npc-soldier-resistance = { $monster } says: "Resistance is useless!"
npc-soldier-dog-meat = { $monster } says: "You're dog meat!"
npc-soldier-surrender = { $monster } says: "Surrender!"
npc-were-shrieks = { $monster } lets out a blood curdling shriek!
npc-were-howls = { $monster } lets out a blood curdling howl!
npc-were-moon = { $monster } whispers inaudibly. All you can make out is "moon".
npc-moo-moos = { $monster } moos.
npc-wail-wails = { $monster } wails.
npc-gurgle-gurgles = { $monster } gurgles.
npc-burble-burbles = { $monster } burbles.
npc-trumpet-trumpets = { $monster } trumpets.
npc-groan-groans = { $monster } groans.
shop-enter-digging-tool = Une voix venue de la boutique vous avertit de laisser votre outil de creusement dehors.
shop-enter-steed = Une voix venue de la boutique exige que vous laissiez { $steed } dehors.
shop-enter-invisible = Une voix soupçonneuse avertit que les clients invisibles ne sont pas les bienvenus.
shop-leave-warning = { $shopkeeper } crie : « Payez avant de partir ! »
shop-repair = { $shopkeeper } répare les dégâts.
shop-keeper-dead = { $shopkeeper } est mort. La boutique est abandonnée.
shop-restock = { $shopkeeper } semble reconnaissant pour ce réassort.
god-roars-suffer = Une voix tonitruante rugit : « Souffrez pour votre blasphème ! »
god-how-dare-harm-servant = Une voix tonitruante rugit : « Comment osez-vous nuire à mon serviteur ? »
god-profane-shrine = Une voix tonitruante rugit : « Vous avez profané mon sanctuaire ! »
ambient-gehennom-damned = You hear the howling of the damned!
ambient-gehennom-groans = You hear groans and moans!
ambient-gehennom-laughter = You hear diabolical laughter!
ambient-gehennom-brimstone = You smell brimstone!
ambient-mines-money = You hear someone counting money.
ambient-mines-register = You hear the chime of a cash register.
ambient-mines-cart = You hear a sound reminiscent of a straining mine cart.
ambient-shop-shoplifters = You hear someone cursing shoplifters.
ambient-shop-register = You hear the chime of a cash register.
ambient-shop-prices = You hear someone mumbling about prices.
ambient-temple-praise = You hear someone praising the gods.
ambient-temple-beseech = You hear someone beseeching the gods.
ambient-temple-sacrifice = You hear an animal carcass being offered in sacrifice.
ambient-temple-donations = You hear a strident plea for donations.
ambient-oracle-wind = You hear a strange wind.
ambient-oracle-ravings = You hear convulsive ravings.
ambient-oracle-snakes = You hear snoring snakes.
ambient-barracks-honed = You hear blades being honed.
ambient-barracks-snoring = You hear loud snoring.
ambient-barracks-dice = You hear dice being thrown.
ambient-barracks-macarthur = You hear General MacArthur!
ambient-swamp-mosquitoes = You hear mosquitoes!
ambient-swamp-marsh-gas = You smell marsh gas!
ambient-swamp-donald-duck = You hear Donald Duck!
ambient-fountain-bubbling = You hear bubbling water.
ambient-fountain-coins = You hear water falling on coins.
ambient-fountain-naiad = You hear the splashing of a naiad.
ambient-fountain-soda = You hear a soda fountain!
ambient-sink-drip = You hear a slow drip.
ambient-sink-gurgle = You hear a gurgling noise.
ambient-sink-dishes = You hear dishes being washed!
ambient-vault-counting = You hear someone counting money.
ambient-vault-searching = You hear someone searching.
ambient-vault-footsteps = You hear the footsteps of a guard on patrol.
ambient-deep-crunching = You hear a crunching sound.
ambient-deep-hollow = You hear a hollow sound.
ambient-deep-rumble = You hear a rumble.
ambient-deep-roar = You hear a distant roar.
ambient-deep-digging = You hear someone digging.
ambient-shallow-door-open = You hear a door open.
ambient-shallow-door-close = You hear a door close.
ambient-shallow-water = You hear water dripping.
ambient-shallow-moving = You hear someone moving around.
ambient-court-conversation = You hear the tones of courtly conversation.
ambient-court-judgment = You hear a sceptre pounded in judgment.
ambient-court-off-with-your-head = You hear someone shout, "Off with your head!"
ambient-court-beruthiel = You hear Queen Beruthiel's cats!
ambient-beehive-buzzing = You hear a low buzzing.
ambient-beehive-drone = You hear an angry drone.
ambient-beehive-bonnet = You hear bees in your bonnet!
ambient-morgue-quiet = You suddenly realize it is unnaturally quiet.
ambient-morgue-neck-hair = The hair on the back of your neck stands up.
ambient-morgue-head-hair = The hair on your head seems to stand up.
ambient-zoo-elephant = You hear a sound reminiscent of an elephant stepping on a peanut.
ambient-zoo-seal = You hear a sound reminiscent of a seal barking.
ambient-zoo-dolittle = You hear Doctor Dolittle!
ambient-oracle-woodchucks = You hear someone say, "No more woodchucks!"
ambient-oracle-zot = You hear a loud ZOT!
ambient-vault-scrooge = You hear Ebenezer Scrooge!
ambient-vault-quarterback = You hear the quarterback calling the play.
ambient-shop-neiman-marcus = You hear Neiman and Marcus arguing!
