package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := Binomial(0, 0); got != (1) { t.Fatalf("case 0: got %#v", got) }
    if got := Binomial(0, 1); got != (0) { t.Fatalf("case 1: got %#v", got) }
    if got := Binomial(1, 0); got != (1) { t.Fatalf("case 2: got %#v", got) }
    if got := Binomial(1, 1); got != (1) { t.Fatalf("case 3: got %#v", got) }
    if got := Binomial(1, 2); got != (0) { t.Fatalf("case 4: got %#v", got) }
    if got := Binomial(5, 2); got != (10) { t.Fatalf("case 5: got %#v", got) }
    if got := Binomial(5, 3); got != (10) { t.Fatalf("case 6: got %#v", got) }
    if got := Binomial(10, 1); got != (10) { t.Fatalf("case 7: got %#v", got) }
    if got := Binomial(10, 9); got != (10) { t.Fatalf("case 8: got %#v", got) }
    if got := Binomial(20, 10); got != (184756) { t.Fatalf("case 9: got %#v", got) }
    if got := Binomial(30, 15); got != (155117520) { t.Fatalf("case 10: got %#v", got) }
    if got := Binomial(30, 0); got != (1) { t.Fatalf("case 11: got %#v", got) }
    if got := Binomial(30, 30); got != (1) { t.Fatalf("case 12: got %#v", got) }
    if got := Binomial(30, 35); got != (0) { t.Fatalf("case 13: got %#v", got) }
}
