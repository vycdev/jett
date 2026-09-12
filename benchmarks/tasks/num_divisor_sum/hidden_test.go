package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := DivisorSum(1); got != (1) { t.Fatalf("case 0: got %#v", got) }
    if got := DivisorSum(2); got != (3) { t.Fatalf("case 1: got %#v", got) }
    if got := DivisorSum(3); got != (4) { t.Fatalf("case 2: got %#v", got) }
    if got := DivisorSum(4); got != (7) { t.Fatalf("case 3: got %#v", got) }
    if got := DivisorSum(6); got != (12) { t.Fatalf("case 4: got %#v", got) }
    if got := DivisorSum(12); got != (28) { t.Fatalf("case 5: got %#v", got) }
    if got := DivisorSum(16); got != (31) { t.Fatalf("case 6: got %#v", got) }
    if got := DivisorSum(25); got != (31) { t.Fatalf("case 7: got %#v", got) }
    if got := DivisorSum(36); got != (91) { t.Fatalf("case 8: got %#v", got) }
    if got := DivisorSum(49); got != (57) { t.Fatalf("case 9: got %#v", got) }
    if got := DivisorSum(64); got != (127) { t.Fatalf("case 10: got %#v", got) }
    if got := DivisorSum(97); got != (98) { t.Fatalf("case 11: got %#v", got) }
    if got := DivisorSum(120); got != (360) { t.Fatalf("case 12: got %#v", got) }
    if got := DivisorSum(360); got != (1170) { t.Fatalf("case 13: got %#v", got) }
    if got := DivisorSum(99991); got != (99992) { t.Fatalf("case 14: got %#v", got) }
    if got := DivisorSum(100000); got != (246078) { t.Fatalf("case 15: got %#v", got) }
}
