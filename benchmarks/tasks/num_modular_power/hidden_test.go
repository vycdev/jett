package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := ModularPower(0, 0, 7); got != (1) { t.Fatalf("case 0: got %#v", got) }
    if got := ModularPower(0, 1, 7); got != (0) { t.Fatalf("case 1: got %#v", got) }
    if got := ModularPower(7, 0, 1); got != (0) { t.Fatalf("case 2: got %#v", got) }
    if got := ModularPower(2, 10, 1000); got != (24) { t.Fatalf("case 3: got %#v", got) }
    if got := ModularPower(3, 4, 5); got != (1) { t.Fatalf("case 4: got %#v", got) }
    if got := ModularPower(999999, 60, 1000000); got != (1) { t.Fatalf("case 5: got %#v", got) }
    if got := ModularPower(1000000, 60, 999983); got != (535869) { t.Fatalf("case 6: got %#v", got) }
    if got := ModularPower(1, 60, 2); got != (1) { t.Fatalf("case 7: got %#v", got) }
    if got := ModularPower(6, 5, 8); got != (0) { t.Fatalf("case 8: got %#v", got) }
    if got := ModularPower(12, 13, 17); got != (14) { t.Fatalf("case 9: got %#v", got) }
    if got := ModularPower(5, 8, 25); got != (0) { t.Fatalf("case 10: got %#v", got) }
    if got := ModularPower(17, 11, 97); got != (38) { t.Fatalf("case 11: got %#v", got) }
    if got := ModularPower(2, 60, 99991); got != (66329) { t.Fatalf("case 12: got %#v", got) }
    if got := ModularPower(999, 3, 1000); got != (999) { t.Fatalf("case 13: got %#v", got) }
}
