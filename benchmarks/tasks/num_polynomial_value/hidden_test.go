package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := PolynomialValue(input_0_0, 3); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{5}
    if got := PolynomialValue(input_1_0, 0); got != (5) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{5}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{5}
    if got := PolynomialValue(input_2_0, -10); got != (5) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{5}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{1, 2, 3}
    if got := PolynomialValue(input_3_0, 0); got != (1) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{1, 2, 3}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{1, 2, 3}
    if got := PolynomialValue(input_4_0, 1); got != (6) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{1, 2, 3}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{1, 2, 3}
    if got := PolynomialValue(input_5_0, 2); got != (17) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{1, 2, 3}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{1, 2, 3}
    if got := PolynomialValue(input_6_0, -2); got != (9) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{1, 2, 3}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{0, 0, 1}
    if got := PolynomialValue(input_7_0, 3); got != (9) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{0, 0, 1}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{1, -1, 1, -1}
    if got := PolynomialValue(input_8_0, -1); got != (4) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{1, -1, 1, -1}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100}
    if got := PolynomialValue(input_9_0, 10); got != (11111111111100) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{-100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100}
    if got := PolynomialValue(input_10_0, -10); got != (9090909090900) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{-100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    if got := PolynomialValue(input_11_0, 10); got != (0) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{3, 0, -4, 0, 5}
    if got := PolynomialValue(input_12_0, 2); got != (67) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{3, 0, -4, 0, 5}) { t.Fatalf("case 12: input mutation") }
}
