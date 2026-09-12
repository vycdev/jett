package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := CoinChange(input_0_0, 0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{}
    if got := CoinChange(input_1_0, 1); got != (-1) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{1}
    if got := CoinChange(input_2_0, 0); got != (0) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{1}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{1}
    if got := CoinChange(input_3_0, 7); got != (7) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{1}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{2}
    if got := CoinChange(input_4_0, 3); got != (-1) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{2}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{2}
    if got := CoinChange(input_5_0, 8); got != (4) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{2}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{1, 3, 4}
    if got := CoinChange(input_6_0, 6); got != (2) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{1, 3, 4}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{5, 2}
    if got := CoinChange(input_7_0, 11); got != (4) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{5, 2}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{3, 7}
    if got := CoinChange(input_8_0, 10); got != (2) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{3, 7}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{3, 7}
    if got := CoinChange(input_9_0, 5); got != (-1) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{3, 7}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{2, 2, 4}
    if got := CoinChange(input_10_0, 8); got != (2) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{2, 2, 4}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{50}
    if got := CoinChange(input_11_0, 100); got != (2) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{50}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{7, 10, 25}
    if got := CoinChange(input_12_0, 99); got != (6) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{7, 10, 25}) { t.Fatalf("case 12: input mutation") }
    input_13_0 := []int64{9, 6, 5, 1}
    if got := CoinChange(input_13_0, 11); got != (2) { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_0, []int64{9, 6, 5, 1}) { t.Fatalf("case 13: input mutation") }
}
