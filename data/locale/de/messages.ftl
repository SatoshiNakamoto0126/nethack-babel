## NetHack Babel — German message catalog
## Fluent (.ftl) format — https://projectfluent.org/

## ============================================================================
## Item naming — BUC labels (gender-agreeing, case-aware)
## ============================================================================

item-buc-blessed = { $gender ->
    [masculine] { $case ->
        [accusative] gesegneten
        [dative] gesegnetem
       *[nominative] gesegneter
    }
    [feminine] { $case ->
        [accusative] gesegnete
        [dative] gesegneter
       *[nominative] gesegnete
    }
   *[neuter] { $case ->
        [accusative] gesegnetes
        [dative] gesegnetem
       *[nominative] gesegnetes
    }
}
item-buc-uncursed = { $gender ->
    [masculine] { $case ->
        [accusative] unverfluchten
        [dative] unverfluchtem
       *[nominative] unverfluchter
    }
    [feminine] { $case ->
        [accusative] unverfluchte
        [dative] unverfluchter
       *[nominative] unverfluchte
    }
   *[neuter] { $case ->
        [accusative] unverfluchtes
        [dative] unverfluchtem
       *[nominative] unverfluchtes
    }
}
item-buc-cursed = { $gender ->
    [masculine] { $case ->
        [accusative] verfluchten
        [dative] verfluchtem
       *[nominative] verfluchter
    }
    [feminine] { $case ->
        [accusative] verfluchte
        [dative] verfluchter
       *[nominative] verfluchte
    }
   *[neuter] { $case ->
        [accusative] verfluchtes
        [dative] verfluchtem
       *[nominative] verfluchtes
    }
}

## ============================================================================
## Item naming — erosion adjectives
## ============================================================================

item-erosion-rusty = { $gender ->
    [masculine] rostiger
    [feminine] rostige
   *[neuter] rostiges
}
item-erosion-very-rusty = { $gender ->
    [masculine] sehr rostiger
    [feminine] sehr rostige
   *[neuter] sehr rostiges
}
item-erosion-thoroughly-rusty = { $gender ->
    [masculine] völlig rostiger
    [feminine] völlig rostige
   *[neuter] völlig rostiges
}
item-erosion-corroded = { $gender ->
    [masculine] korrodierter
    [feminine] korrodierte
   *[neuter] korrodiertes
}
item-erosion-very-corroded = { $gender ->
    [masculine] sehr korrodierter
    [feminine] sehr korrodierte
   *[neuter] sehr korrodiertes
}
item-erosion-thoroughly-corroded = { $gender ->
    [masculine] völlig korrodierter
    [feminine] völlig korrodierte
   *[neuter] völlig korrodiertes
}
item-erosion-burnt = { $gender ->
    [masculine] verbrannter
    [feminine] verbrannte
   *[neuter] verbranntes
}
item-erosion-very-burnt = { $gender ->
    [masculine] sehr verbrannter
    [feminine] sehr verbrannte
   *[neuter] sehr verbranntes
}
item-erosion-thoroughly-burnt = { $gender ->
    [masculine] völlig verbrannter
    [feminine] völlig verbrannte
   *[neuter] völlig verbranntes
}
item-erosion-rotted = { $gender ->
    [masculine] verfaulter
    [feminine] verfaulte
   *[neuter] verfaultes
}
item-erosion-very-rotted = { $gender ->
    [masculine] sehr verfaulter
    [feminine] sehr verfaulte
   *[neuter] sehr verfaultes
}
item-erosion-thoroughly-rotted = { $gender ->
    [masculine] völlig verfaulter
    [feminine] völlig verfaulte
   *[neuter] völlig verfaultes
}
item-erosion-rustproof = rostfrei
item-erosion-fireproof = feuerfest
item-erosion-corrodeproof = korrosionsbeständig
item-erosion-rotproof = fäulnisbeständig

## ============================================================================
## Item naming — class-specific base name patterns
## ============================================================================

item-potion-identified = Trank von { $name }
item-potion-called = Trank namens { $called }
item-potion-appearance = { $appearance } Trank
item-potion-generic = Trank

item-scroll-identified = Schriftrolle von { $name }
item-scroll-called = Schriftrolle namens { $called }
item-scroll-labeled = Schriftrolle beschriftet { $label }
item-scroll-appearance = { $appearance } Schriftrolle
item-scroll-generic = Schriftrolle

item-wand-identified = Zauberstab von { $name }
item-wand-called = Zauberstab namens { $called }
item-wand-appearance = { $appearance } Zauberstab
item-wand-generic = Zauberstab

item-ring-identified = Ring von { $name }
item-ring-called = Ring namens { $called }
item-ring-appearance = { $appearance } Ring
item-ring-generic = Ring

item-amulet-called = Amulett namens { $called }
item-amulet-appearance = { $appearance } Amulett
item-amulet-generic = Amulett

item-spellbook-identified = Zauberbuch von { $name }
item-spellbook-called = Zauberbuch namens { $called }
item-spellbook-appearance = { $appearance } Zauberbuch
item-spellbook-generic = Zauberbuch

item-gem-stone = Stein
item-gem-gem = Edelstein
item-gem-called-stone = Stein namens { $called }
item-gem-called-gem = Edelstein namens { $called }
item-gem-appearance-stone = { $appearance } Stein
item-gem-appearance-gem = { $appearance } Edelstein

item-generic-called = { $base } namens { $called }

## ============================================================================
## Item naming — connectors and suffixes
## ============================================================================

item-named-suffix = namens { $name }

## ============================================================================
## Item naming — articles (gender+case aware)
## ============================================================================

item-article-indefinite = { $gender ->
    [masculine] { $case ->
        [accusative] einen
        [dative] einem
       *[nominative] ein
    }
    [feminine] { $case ->
        [accusative] eine
        [dative] einer
       *[nominative] eine
    }
   *[neuter] { $case ->
        [accusative] ein
        [dative] einem
       *[nominative] ein
    }
}
item-article-definite = { $gender ->
    [masculine] { $case ->
        [accusative] den
        [dative] dem
       *[nominative] der
    }
    [feminine] { $case ->
        [accusative] die
        [dative] der
       *[nominative] die
    }
   *[neuter] { $case ->
        [accusative] das
        [dative] dem
       *[nominative] das
    }
}
item-article-your = { $gender ->
    [masculine] { $case ->
        [accusative] deinen
        [dative] deinem
       *[nominative] dein
    }
    [feminine] { $case ->
        [accusative] deine
        [dative] deiner
       *[nominative] deine
    }
   *[neuter] { $case ->
        [accusative] dein
        [dative] deinem
       *[nominative] dein
    }
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
## Optionsmenü
## ============================================================================

ui-options-title = Optionen
ui-options-game = Spieleinstellungen
ui-options-display = Anzeigeeinstellungen
ui-options-sound = Toneinstellungen

## Spieloptionen

opt-autopickup = Automatisch aufheben
opt-autopickup-types = Aufheben-Typen
opt-legacy = Einführungsgeschichte

## Anzeigeoptionen

opt-map-colors = Kartenfarben
opt-message-colors = Nachrichtenfarben
opt-buc-highlight = BUC-Hervorhebung
opt-minimap = Minikarte
opt-mouse-hover = Maus-Hover-Info
opt-nerd-fonts = Nerd-Schriften

## Tonoptionen

opt-sound-enabled = Ton aktiviert
opt-volume = Lautstärke

## Optionswerte

opt-on = AN
opt-off = AUS

## ============================================================================
## Einleitungserzählung (Legacy)
## ============================================================================

legacy-intro =
    Es steht geschrieben im Buch von { $deity }:

        Nach der Schöpfung rebellierte der grausame Gott
        Moloch gegen die Autorität von Marduk dem Schöpfer.
        Moloch stahl Marduk das mächtigste aller Artefakte
        der Götter, das Amulett von Yendor, und verbarg es
        in den dunklen Höhlen von Gehennom, der Unterwelt,
        wo er nun lauert und seine Zeit abwartet.

    Euer { $deity } trachtet danach, das Amulett zu besitzen,
    und damit den verdienten Aufstieg über die anderen Götter
    zu erlangen.

    Ihr, ein frisch ausgebildeter { $role }, wurdet von Geburt
    an als Werkzeug von { $deity } verkündet. Ihr seid bestimmt,
    das Amulett für Eure Gottheit zurückzuholen, oder bei dem
    Versuch zu sterben. Eure Stunde ist gekommen. Um unser
    aller willen: Geht mutig mit { $deity }!

## ============================================================================
## Oberfläche — Hilfe
## ============================================================================

help-title = NetHack Babel Hilfe
help-move = Bewegen Sie sich mit hjklyubn oder Pfeiltasten.
help-attack = Gehen Sie auf ein Monster zu, um es anzugreifen.
help-wait = Drücken Sie . oder s, um eine Runde zu warten.
help-search = Drücken Sie s, um nach Verborgenem zu suchen.
help-inventory = Drücken Sie i, um das Inventar anzusehen.
help-pickup = Drücken Sie , um Gegenstände aufzuheben.
help-drop = Drücken Sie d, um Gegenstände abzulegen.
help-stairs-up = Drücken Sie < um die Treppe hochzugehen.
help-stairs-down = Drücken Sie > um die Treppe runterzugehen.
help-eat = Drücken Sie e zum Essen.
help-quaff = Drücken Sie q, um einen Trank zu trinken.
help-read = Drücken Sie r, um eine Schriftrolle oder ein Zauberbuch zu lesen.
help-wield = Drücken Sie w, um eine Waffe zu führen.
help-wear = Drücken Sie W, um Rüstung anzulegen.
help-remove = Drücken Sie T, um Rüstung abzulegen.
help-zap = Drücken Sie z, um einen Zauberstab zu benutzen.

# Hilfe — Bewegungsdiagramm
help-move-diagram =
    {"  "}y k u     NW  N  NO
    {"  "}h . l      W  .   O
    {"  "}b j n     SW  S  SO

# Hilfe — Symbole
help-symbols-title = Symbole:
help-symbol-player = @  = Sie (der Spieler)
help-symbol-floor = .  = Boden
help-symbol-corridor = #  = Korridor
help-symbol-door-closed = +  = geschlossene Tür
help-symbol-door-open = |  = offene Tür
help-symbol-stairs-up = <  = Treppe hoch
help-symbol-stairs-down = >  = Treppe runter
help-symbol-water = {"}"}  = Wasser/Lava
help-symbol-fountain = {"{"} = Brunnen

# Hilfe — zusätzliche Befehle
help-options = Drücken Sie O für Optionen.
help-look = Drücken Sie : um den Boden zu sehen.
help-history = Drücken Sie Strg+P für Nachrichtenverlauf.
help-shift-run = Shift+Richtung = Laufen.
help-arrows = Pfeiltasten funktionieren auch.

## ============================================================================
## Oberfläche — Inventar
## ============================================================================

ui-inventory-title = Inventar
ui-inventory-empty = Ihr tragt nichts bei Euch.

inv-class-weapon = Waffen
inv-class-armor = Rüstungen
inv-class-ring = Ringe
inv-class-amulet = Amulette
inv-class-tool = Werkzeuge
inv-class-food = Nahrung
inv-class-potion = Tränke
inv-class-scroll = Schriftrollen
inv-class-spellbook = Zauberbücher
inv-class-wand = Zauberstäbe
inv-class-coin = Münzen
inv-class-gem = Edelsteine
inv-class-rock = Steine
inv-class-ball = Eisenkugeln
inv-class-chain = Ketten
inv-class-venom = Gift
inv-class-other = Sonstiges

# Inventar-BUC-Markierungen
inv-buc-marker-blessed = [G]
inv-buc-marker-cursed = [V]
inv-buc-tag-blessed = (gesegnet)
inv-buc-tag-cursed = (verflucht)
inv-buc-tag-uncursed = (nicht verflucht)
ui-pickup-title = Was aufheben?

# Allgemeine TUI-Meldungen
ui-never-mind = Macht nichts.
ui-no-such-item = Diesen Gegenstand haben Sie nicht.
ui-not-implemented = Noch nicht implementiert.
ui-empty-handed = Sie sind unbewaffnet.

## ============================================================================
## Aktionsdispatch
## ============================================================================

eat-generic = Du isst das Essen.
eat-what = Was essen?
quaff-generic = Du trinkst den Trank.
quaff-what = Was trinken?
read-generic = Du liest die Schriftrolle.
read-what = Was lesen?
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
zap-generic = Du benutzt den Zauberstab.

## Türen
door-open-success = Die Tür öffnet sich.
door-locked = Diese Tür ist verschlossen.
door-already-open = Diese Tür ist bereits offen.
door-not-here = Dort ist keine Tür.
door-close-success = Die Tür schließt sich.
door-already-closed = Diese Tür ist bereits geschlossen.

## Schlösser
lock-nothing-to-force = Hier gibt es nichts aufzubrechen.

## Gebet
pray-begin = Du beginnst zu deiner Gottheit zu beten...

## Opfergaben
offer-generic = Du bringst ein Opfer am Altar dar.
offer-amulet-rejected = Das Amulett wird zurückgewiesen und landet in deiner Nähe!
offer-what = Was opfern?

## Gespräch
npc-chat-no-response = Die Kreatur scheint nicht reden zu wollen.
npc-chat-sleeping = Die Kreatur scheint Euch nicht zu bemerken.
chat-nobody-there = Dort ist niemand zum Reden.

## Bewegung / Reise
peaceful-monster-blocks = Du hältst an. { $monster } steht im Weg.
ride-not-available = Hier gibt es nichts zum Reiten.
enhance-not-available = Du kannst gerade keine Fähigkeiten verbessern.
enhance-success = Ihr verbessert { $skill } auf { $level }.
travel-not-implemented = Reisen ist noch nicht verfügbar.
two-weapon-not-implemented = Zweiwaffenkampf ist noch nicht verfügbar.
two-weapon-enabled = Ihr kämpft jetzt mit zwei Waffen.
two-weapon-disabled = Ihr kämpft nicht mehr mit zwei Waffen.
name-not-implemented = Benennen ist noch nicht verfügbar.
adjust-not-implemented = Inventaranpassung ist noch nicht verfügbar.

## Wizard-Modus (Debug)
wizard-identify-all = Du weißt plötzlich alles über deine Ausrüstung.
wizard-map-revealed = Ein Bild deiner Umgebung formt sich in deinem Geist!
wizard-vague-nervous = Dir wird plötzlich ganz mulmig.
wizard-black-glow = Du bemerkst einen schwarzen Schimmer um dich herum.
wizard-aggravate = Aus der Ferne hallen Geräusche wider, als der Dungeon plötzlich erwacht.
wizard-respawned = Der Zauberer von Yendor erhebt sich erneut!
wizard-respawned-boom = Eine Stimme dröhnt...
wizard-respawned-taunt = Du dachtest also, du könntest mich {$verb}, Narr.
wizard-steal-amulet = Der Zauberer von Yendor stiehlt das Amulett!
wizard-summon-nasties = Neue Scheusale erscheinen aus dem Nichts!
wizard-taunt-laughs = { $wizard } lacht teuflisch.
wizard-taunt-relinquish = Gib das Amulett her, { $insult }!
wizard-taunt-panic = Selbst jetzt versiegt deine Lebenskraft, { $insult }!
wizard-taunt-last-breath = Koste deinen Atem, { $insult }, es sei dein letzter!
wizard-taunt-return = Ich werde zurückkehren.
wizard-taunt-back = Ich komme wieder.
wizard-taunt-general = { $malediction }, { $insult }!
amulet-feels-hot = Das Amulett fühlt sich heiß an!
amulet-feels-very-warm = Das Amulett fühlt sich sehr warm an.
amulet-feels-warm = Das Amulett fühlt sich warm an.
wizard-detect-all = Du spürst alles um dich herum.
wizard-genesis = Ein { $monster } erscheint neben dir.
wizard-genesis-failed = Nichts reagiert auf deine Bitte um { $monster }.
wizard-wish = Dein Wunsch wird erfüllt: { $item }.
wizard-wish-adjusted = Dein Wunsch wird angepasst zu: { $item }.
wizard-wish-floor = Dein Wunsch wird erfüllt: { $item } landet vor deinen Füßen.
wizard-wish-adjusted-floor = Dein Wunsch wird angepasst: { $item } landet vor deinen Füßen.
wizard-wish-failed = Nichts reagiert auf deinen Wunsch nach "{ $wish }".
wizard-kill = Du löschst { $count } Monster auf dieser Ebene aus.
wizard-kill-none = Hier gibt es keine Monster zum Auslöschen.
wizard-where-current = Du bist auf { $location } (absolute Tiefe { $absolute }) bei { $x },{ $y }.
wizard-where-special = { $level } befindet sich auf { $location }.

## ============================================================================
## Quest / NPC-Dialog
## ============================================================================

quest-leader-greeting = Willkommen, { $role }. Ich habe Euch erwartet.
quest-assignment =
    Hört gut zu, { $role }. Der { $nemesis } hat { $artifact } gestohlen.
    Ihr müsst in die Tiefe hinabsteigen und es zurückholen.
    Unser Schicksal hängt von Euch ab.

## Händler
shop-welcome = Willkommen in { $shopkeeper }s { $shoptype }!
shop-buy-prompt = { $shopkeeper } sagt: „Bar oder auf Kredit?"
shop-unpaid-warning = { $shopkeeper } sagt: „Ihr habt unbezahlte Waren!"
shop-theft-warning = { $shopkeeper } ruft: „Dieb! Bezahlt sofort!"

## Priester
priest-welcome = Der Priester spricht einen Segen über Euch.
priest-protection-offer = Der Priester bietet göttlichen Schutz für { $cost } Goldstücke an.
priest-donation-thanks = Der Priester dankt Euch für Eure großzügige Spende.
priest-cranky-1 = Der Priester faucht: „Ihr wollt Worte? Ich gebe Euch Worte!“
priest-cranky-2 = Der Priester schnaubt: „Reden? Das hier habe ich zu sagen!“
priest-cranky-3 = Der Priester sagt: „Pilger, ich spreche nicht länger mit Euch.“

## ============================================================================
## Inhalte (Gerüchte, Orakel)
## ============================================================================

rumor-fortune-cookie = Du öffnest den Glückskeks. Er sagt: „{ $rumor }"
oracle-consultation = { $text }
oracle-no-mood = The Oracle is in no mood for consultations.
oracle-no-gold = You have no gold.
oracle-not-enough-gold = You don't even have enough gold for that!

## ============================================================================
## Symbolidentifikation
## ============================================================================

whatis-prompt = Was möchtest du identifizieren? (Wähle eine Position)
whatis-terrain = { $description } (Gelände)
whatis-monster = { $description } (Monster)
whatis-object = { $description } (Gegenstand)
whatis-nothing = Du siehst dort nichts Besonderes.

## Entdeckungen
discoveries-title = Entdeckungen
discoveries-empty = Du hast noch nichts entdeckt.
guardian-angel-appears = Ein Schutzengel erscheint neben dir!
temple-enter = Ihr betretet einen Tempel von { $god }.
temple-forbidding = Eine abweisende Heiligkeit liegt in der Luft.
temple-peace = Ein tiefer Friede senkt sich über den Tempel.
temple-unusual-peace = Der Tempel wirkt ungewöhnlich friedlich.
temple-eerie = Ihr habt ein unheimliches Gefühl...
temple-ghost-appears = Ein Geist erscheint vor Euch!
temple-shiver = Ihr schaudert.
temple-watched = Ihr habt das Gefühl, beobachtet zu werden.
sanctum-infidel = „Ihr wagt es, das Sanktuarium zu betreten, Ungläubiger!“
sanctum-be-gone = „Fort mit Euch, Sterblicher!“
sanctum-desecrate = Ihr entweiht den Hochaltar!
priest-ale-gift = Der Priester gibt Euch { $amount } Goldstücke für ein Bier.
priest-cheapskate = Der Priester mustert Eure dürftige Spende skeptisch.
priest-small-thanks = Der Priester dankt Euch für das Wenige, das Ihr entbehren könnt.
priest-pious = Der Priester sagt, Ihr seid wahrlich fromm.
priest-clairvoyance = Der Priester gewährt Euch einen Moment der Einsicht.
status-clairvoyance-end = Eure hellsichtige Einsicht schwindet.
priest-selfless-generosity = Der Priester würdigt Eure selbstlose Großzügigkeit zutiefst.
priest-cleansing = Der Segen des Priesters erleichtert Eure geistige Last.
priest-not-enough-gold = Der Priester verlangt { $cost } Goldstücke.
priest-protection-granted = Der Priester gewährt Euch göttlichen Schutz für { $cost } Goldstücke.
shk-welcome = { $shopkeeper } sagt: „Willkommen in meinem Laden, { $honorific }.“
shk-angry-greeting = { $shopkeeper } starrt Euch wütend an.
shk-follow-reminder = { $shopkeeper } sagt: „Hallo, { $honorific }! Habt Ihr nicht vergessen zu bezahlen?“
shk-bill-total = { $shopkeeper } sagt, dass Eure Rechnung { $amount } Goldstücke beträgt.
shk-debit-reminder = { $shopkeeper } erinnert Euch daran, dass Ihr { $amount } Goldstücke schuldet.
shk-credit-reminder = { $shopkeeper } ermuntert Euch, Eure { $amount } Goldstücke Guthaben zu nutzen.
shk-robbed-greeting = { $shopkeeper } sagt: „Den Raub habe ich nicht vergessen, { $honorific }.“
shk-surcharge-greeting = { $shopkeeper } sagt: „Für Euch sind die Preise jetzt höher, { $honorific }.“
shk-business-bad = { $shopkeeper } beklagt, dass das Geschäft schlecht läuft.
shk-business-good = { $shopkeeper } sagt, dass das Geschäft gut läuft.
shk-shoplifters = { $shopkeeper } spricht über das Problem mit Ladendieben.
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
npc-cuss-ancestry = { $monster } casts aspersions on your ancestry.
npc-cuss-angel-repent = { $monster } says: "Repent, and thou shalt be saved!"
npc-cuss-angel-insolence = { $monster } says: "Thou shalt pay for thine insolence!"
npc-cuss-angel-maker = { $monster } says: "Very soon, my child, thou shalt meet thy maker."
npc-cuss-angel-wrath = { $monster } says: "The wrath of heaven is now upon you!"
npc-cuss-angel-not-worthy = { $monster } says: "Thou art not worthy to seek the Amulet."
npc-cuss-demon-slime = { $monster } says: "Eat slime and die!"
npc-cuss-demon-clumsy = { $monster } says: "Hast thou been drinking, or art thou always so clumsy?"
npc-cuss-demon-laughter = { $monster } says: "Mercy! Dost thou wish me to die of laughter?"
npc-cuss-demon-amulet = { $monster } says: "Why search for the Amulet? Thou wouldst but lose it, cretin."
npc-cuss-demon-comedian = { $monster } says: "Thou ought to be a comedian, thy skills are so laughable!"
npc-cuss-demon-odor = { $monster } says: "Hast thou considered masking thine odour?"
npc-spell-cantrip = { $monster } seems to mutter a cantrip.
npc-vampire-tame-craving = { $monster } says: "I can stand this craving no longer!"
npc-vampire-tame-weary = { $monster } says: "I find myself growing a little weary."
npc-vampire-peaceful = { $monster } says: "I only drink... potions."
npc-vampire-hostile-blood = { $monster } says: "I vant to suck your blood!"
npc-vampire-hostile-hunt = { $monster } says: "I vill come after you without regret!"
npc-imitate-imitates = { $monster } imitates you.
npc-rider-sandman = { $monster } is busy reading a copy of Sandman #8.
npc-rider-war = { $monster } says: "Who do you think you are, War?"
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
shop-enter-digging-tool = Eine Stimme aus dem Laden warnt Euch, Euer Grabwerkzeug draußen zu lassen.
shop-enter-steed = Eine Stimme aus dem Laden besteht darauf, dass Ihr { $steed } draußen lasst.
shop-enter-invisible = Eine misstrauische Stimme warnt, dass unsichtbare Kunden nicht willkommen sind.
shop-leave-warning = { $shopkeeper } ruft: „Bezahlt, bevor Ihr geht!“
shop-repair = { $shopkeeper } repariert den Schaden.
shop-keeper-dead = { $shopkeeper } ist tot. Der Laden ist verlassen.
shop-restock = { $shopkeeper } scheint für die Wiedergutmachung dankbar zu sein.
god-roars-suffer = Eine dröhnende Stimme donnert: „Leide für deine Lästerung!“
god-how-dare-harm-servant = Eine dröhnende Stimme donnert: „Wie wagst du es, meinem Diener zu schaden?“
god-profane-shrine = Eine dröhnende Stimme donnert: „Du hast meinen Schrein entweiht!“
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
