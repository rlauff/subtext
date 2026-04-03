// Hilfsfunktion: Wandelt eine Reihe von Einsen in Nullen um
def to_zeros {
        1(.*) => 0~to_zeros(^#1)
    ||        => 
}

// Hauptfunktion: Zählt eine Binärzahl um +1 hoch
// Logik: Wir suchen die LETZTE '0' im String und alle '1'en, die danach kommen.
// Die '0' wird zur '1' und alle folgenden '1'en werden durch die Hilfsfunktion zu Nullen.
def inc_bin {
        (.*)0(1*) => #1~1~to_zeros(^#2)
    ||  (1+)      => 1~to_zeros(^#1)    // Sonderfall: String besteht nur aus Einsen (z.B. 111 -> 1000)
    ||            => 1                 // Sonderfall: Leerer Input
}

// Test
print_output(inc_bin(1011))
