package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    if got := Fibonacci(0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if got := Fibonacci(1); got != (1) { t.Fatalf("case 1: got %#v", got) }
    if got := Fibonacci(2); got != (1) { t.Fatalf("case 2: got %#v", got) }
    if got := Fibonacci(3); got != (2) { t.Fatalf("case 3: got %#v", got) }
    if got := Fibonacci(4); got != (3) { t.Fatalf("case 4: got %#v", got) }
    if got := Fibonacci(5); got != (5) { t.Fatalf("case 5: got %#v", got) }
    if got := Fibonacci(8); got != (21) { t.Fatalf("case 6: got %#v", got) }
    if got := Fibonacci(10); got != (55) { t.Fatalf("case 7: got %#v", got) }
    if got := Fibonacci(20); got != (6765) { t.Fatalf("case 8: got %#v", got) }
    if got := Fibonacci(30); got != (832040) { t.Fatalf("case 9: got %#v", got) }
    if got := Fibonacci(40); got != (102334155) { t.Fatalf("case 10: got %#v", got) }
    if got := Fibonacci(50); got != (12586269025) { t.Fatalf("case 11: got %#v", got) }
    if got := Fibonacci(60); got != (1548008755920) { t.Fatalf("case 12: got %#v", got) }
    if got := Fibonacci(70); got != (190392490709135) { t.Fatalf("case 13: got %#v", got) }
}
