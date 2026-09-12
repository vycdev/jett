package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := PrimeStatus(-100); got != (false) { t.Fatalf("case 0: got %#v", got) }
    if got := PrimeStatus(-1); got != (false) { t.Fatalf("case 1: got %#v", got) }
    if got := PrimeStatus(0); got != (false) { t.Fatalf("case 2: got %#v", got) }
    if got := PrimeStatus(1); got != (false) { t.Fatalf("case 3: got %#v", got) }
    if got := PrimeStatus(2); got != (true) { t.Fatalf("case 4: got %#v", got) }
    if got := PrimeStatus(3); got != (true) { t.Fatalf("case 5: got %#v", got) }
    if got := PrimeStatus(4); got != (false) { t.Fatalf("case 6: got %#v", got) }
    if got := PrimeStatus(9); got != (false) { t.Fatalf("case 7: got %#v", got) }
    if got := PrimeStatus(25); got != (false) { t.Fatalf("case 8: got %#v", got) }
    if got := PrimeStatus(49); got != (false) { t.Fatalf("case 9: got %#v", got) }
    if got := PrimeStatus(97); got != (true) { t.Fatalf("case 10: got %#v", got) }
    if got := PrimeStatus(121); got != (false) { t.Fatalf("case 11: got %#v", got) }
    if got := PrimeStatus(997); got != (true) { t.Fatalf("case 12: got %#v", got) }
    if got := PrimeStatus(1024); got != (false) { t.Fatalf("case 13: got %#v", got) }
    if got := PrimeStatus(65521); got != (true) { t.Fatalf("case 14: got %#v", got) }
    if got := PrimeStatus(999983); got != (true) { t.Fatalf("case 15: got %#v", got) }
    if got := PrimeStatus(1000000); got != (false) { t.Fatalf("case 16: got %#v", got) }
}
