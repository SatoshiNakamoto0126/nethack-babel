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
offer-what = Offrir quoi ?

## Discussion
npc-chat-no-response = La créature ne semble pas vouloir discuter.
chat-nobody-there = Il n'y a personne à qui parler.

## Déplacement / voyage
ride-not-available = Il n'y a rien à chevaucher ici.
enhance-not-available = Vous ne pouvez améliorer aucune compétence pour le moment.
enhance-success = Vous améliorez { $skill } au niveau { $level }.
travel-not-implemented = Le voyage n'est pas encore disponible.
two-weapon-not-implemented = Le combat à deux armes n'est pas encore disponible.
two-weapon-enabled = Vous commencez à combattre avec deux armes.
two-weapon-disabled = Vous cessez de combattre avec deux armes.
name-not-implemented = Le nommage n'est pas encore disponible.
adjust-not-implemented = L'ajustement d'inventaire n'est pas encore disponible.

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
