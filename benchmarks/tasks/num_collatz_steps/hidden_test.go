package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := CollatzSteps(1, 0); !got.Found || got.Value != 0 { t.Fatalf("case 0: got %#v", got) }
    if got := CollatzSteps(2, 0); got.Found { t.Fatalf("case 1: got %#v", got) }
    if got := CollatzSteps(2, 1); !got.Found || got.Value != 1 { t.Fatalf("case 2: got %#v", got) }
    if got := CollatzSteps(3, 6); got.Found { t.Fatalf("case 3: got %#v", got) }
    if got := CollatzSteps(3, 7); !got.Found || got.Value != 7 { t.Fatalf("case 4: got %#v", got) }
    if got := CollatzSteps(6, 8); !got.Found || got.Value != 8 { t.Fatalf("case 5: got %#v", got) }
    if got := CollatzSteps(7, 15); got.Found { t.Fatalf("case 6: got %#v", got) }
    if got := CollatzSteps(7, 16); !got.Found || got.Value != 16 { t.Fatalf("case 7: got %#v", got) }
    if got := CollatzSteps(27, 110); got.Found { t.Fatalf("case 8: got %#v", got) }
    if got := CollatzSteps(27, 111); !got.Found || got.Value != 111 { t.Fatalf("case 9: got %#v", got) }
    if got := CollatzSteps(1000000, 200); !got.Found || got.Value != 152 { t.Fatalf("case 10: got %#v", got) }
    if got := CollatzSteps(999999, 200); got.Found { t.Fatalf("case 11: got %#v", got) }
    if got := CollatzSteps(1024, 9); got.Found { t.Fatalf("case 12: got %#v", got) }
    if got := CollatzSteps(1024, 10); !got.Found || got.Value != 10 { t.Fatalf("case 13: got %#v", got) }
}
