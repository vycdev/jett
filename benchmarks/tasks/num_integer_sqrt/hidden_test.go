package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := IntegerSqrt(0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if got := IntegerSqrt(1); got != (1) { t.Fatalf("case 1: got %#v", got) }
    if got := IntegerSqrt(2); got != (1) { t.Fatalf("case 2: got %#v", got) }
    if got := IntegerSqrt(3); got != (1) { t.Fatalf("case 3: got %#v", got) }
    if got := IntegerSqrt(4); got != (2) { t.Fatalf("case 4: got %#v", got) }
    if got := IntegerSqrt(8); got != (2) { t.Fatalf("case 5: got %#v", got) }
    if got := IntegerSqrt(9); got != (3) { t.Fatalf("case 6: got %#v", got) }
    if got := IntegerSqrt(10); got != (3) { t.Fatalf("case 7: got %#v", got) }
    if got := IntegerSqrt(15); got != (3) { t.Fatalf("case 8: got %#v", got) }
    if got := IntegerSqrt(16); got != (4) { t.Fatalf("case 9: got %#v", got) }
    if got := IntegerSqrt(17); got != (4) { t.Fatalf("case 10: got %#v", got) }
    if got := IntegerSqrt(99980000); got != (9998) { t.Fatalf("case 11: got %#v", got) }
    if got := IntegerSqrt(99980001); got != (9999) { t.Fatalf("case 12: got %#v", got) }
    if got := IntegerSqrt(99980002); got != (9999) { t.Fatalf("case 13: got %#v", got) }
    if got := IntegerSqrt(999999999999); got != (999999) { t.Fatalf("case 14: got %#v", got) }
    if got := IntegerSqrt(1000000000000); got != (1000000) { t.Fatalf("case 15: got %#v", got) }
}
