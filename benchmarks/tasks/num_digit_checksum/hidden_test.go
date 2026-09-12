package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := DigitChecksum(0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if got := DigitChecksum(1); got != (1) { t.Fatalf("case 1: got %#v", got) }
    if got := DigitChecksum(9); got != (9) { t.Fatalf("case 2: got %#v", got) }
    if got := DigitChecksum(10); got != (-1) { t.Fatalf("case 3: got %#v", got) }
    if got := DigitChecksum(11); got != (0) { t.Fatalf("case 4: got %#v", got) }
    if got := DigitChecksum(12); got != (1) { t.Fatalf("case 5: got %#v", got) }
    if got := DigitChecksum(123); got != (2) { t.Fatalf("case 6: got %#v", got) }
    if got := DigitChecksum(1234); got != (2) { t.Fatalf("case 7: got %#v", got) }
    if got := DigitChecksum(90909); got != (27) { t.Fatalf("case 8: got %#v", got) }
    if got := DigitChecksum(100001); got != (0) { t.Fatalf("case 9: got %#v", got) }
    if got := DigitChecksum(987654321); got != (5) { t.Fatalf("case 10: got %#v", got) }
    if got := DigitChecksum(1000000000000); got != (1) { t.Fatalf("case 11: got %#v", got) }
    if got := DigitChecksum(999999999999); got != (0) { t.Fatalf("case 12: got %#v", got) }
    if got := DigitChecksum(10101010101); got != (6) { t.Fatalf("case 13: got %#v", got) }
}
