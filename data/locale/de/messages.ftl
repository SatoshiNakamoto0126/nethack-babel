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
offer-what = Was opfern?

## Gespräch
npc-chat-no-response = Die Kreatur scheint nicht reden zu wollen.
chat-nobody-there = Dort ist niemand zum Reden.

## Bewegung / Reise
ride-not-available = Hier gibt es nichts zum Reiten.
enhance-not-available = Du kannst gerade keine Fähigkeiten verbessern.
enhance-success = Ihr verbessert { $skill } auf { $level }.
travel-not-implemented = Reisen ist noch nicht verfügbar.
two-weapon-not-implemented = Zweiwaffenkampf ist noch nicht verfügbar.
two-weapon-enabled = Ihr kämpft jetzt mit zwei Waffen.
two-weapon-disabled = Ihr kämpft nicht mehr mit zwei Waffen.
name-not-implemented = Benennen ist noch nicht verfügbar.
adjust-not-implemented = Inventaranpassung ist noch nicht verfügbar.

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

## ============================================================================
## Inhalte (Gerüchte, Orakel)
## ============================================================================

rumor-fortune-cookie = Du öffnest den Glückskeks. Er sagt: „{ $rumor }"
oracle-consultation = { $text }

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
