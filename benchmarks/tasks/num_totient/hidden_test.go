package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := Totient(1); got != (1) { t.Fatalf("case 0: got %#v", got) }
    if got := Totient(2); got != (1) { t.Fatalf("case 1: got %#v", got) }
    if got := Totient(3); got != (2) { t.Fatalf("case 2: got %#v", got) }
    if got := Totient(4); got != (2) { t.Fatalf("case 3: got %#v", got) }
    if got := Totient(5); got != (4) { t.Fatalf("case 4: got %#v", got) }
    if got := Totient(6); got != (2) { t.Fatalf("case 5: got %#v", got) }
    if got := Totient(8); got != (4) { t.Fatalf("case 6: got %#v", got) }
    if got := Totient(9); got != (6) { t.Fatalf("case 7: got %#v", got) }
    if got := Totient(10); got != (4) { t.Fatalf("case 8: got %#v", got) }
    if got := Totient(12); got != (4) { t.Fatalf("case 9: got %#v", got) }
    if got := Totient(30); got != (8) { t.Fatalf("case 10: got %#v", got) }
    if got := Totient(36); got != (12) { t.Fatalf("case 11: got %#v", got) }
    if got := Totient(49); got != (42) { t.Fatalf("case 12: got %#v", got) }
    if got := Totient(97); got != (96) { t.Fatalf("case 13: got %#v", got) }
    if got := Totient(210); got != (48) { t.Fatalf("case 14: got %#v", got) }
    if got := Totient(1024); got != (512) { t.Fatalf("case 15: got %#v", got) }
    if got := Totient(9999); got != (6000) { t.Fatalf("case 16: got %#v", got) }
    if got := Totient(10000); got != (4000) { t.Fatalf("case 17: got %#v", got) }
}
