package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    input_0_1 := []int64{}
    if got := KnapsackValue(input_0_0, input_0_1, 0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    if !reflect.DeepEqual(input_0_1, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{}
    input_1_1 := []int64{}
    if got := KnapsackValue(input_1_0, input_1_1, 8); got != (0) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{}) { t.Fatalf("case 1: input mutation") }
    if !reflect.DeepEqual(input_1_1, []int64{}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{1}
    input_2_1 := []int64{7}
    if got := KnapsackValue(input_2_0, input_2_1, 0); got != (0) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{1}) { t.Fatalf("case 2: input mutation") }
    if !reflect.DeepEqual(input_2_1, []int64{7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{5}
    input_3_1 := []int64{10}
    if got := KnapsackValue(input_3_0, input_3_1, 4); got != (0) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{5}) { t.Fatalf("case 3: input mutation") }
    if !reflect.DeepEqual(input_3_1, []int64{10}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{5}
    input_4_1 := []int64{10}
    if got := KnapsackValue(input_4_0, input_4_1, 5); got != (10) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{5}) { t.Fatalf("case 4: input mutation") }
    if !reflect.DeepEqual(input_4_1, []int64{10}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{2, 3, 4}
    input_5_1 := []int64{4, 5, 7}
    if got := KnapsackValue(input_5_0, input_5_1, 5); got != (9) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{2, 3, 4}) { t.Fatalf("case 5: input mutation") }
    if !reflect.DeepEqual(input_5_1, []int64{4, 5, 7}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{2, 2, 2}
    input_6_1 := []int64{3, 3, 3}
    if got := KnapsackValue(input_6_0, input_6_1, 4); got != (6) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{2, 2, 2}) { t.Fatalf("case 6: input mutation") }
    if !reflect.DeepEqual(input_6_1, []int64{3, 3, 3}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{1, 1, 1}
    input_7_1 := []int64{0, 5, 7}
    if got := KnapsackValue(input_7_0, input_7_1, 2); got != (12) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{1, 1, 1}) { t.Fatalf("case 7: input mutation") }
    if !reflect.DeepEqual(input_7_1, []int64{0, 5, 7}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{6, 3, 4, 2}
    input_8_1 := []int64{30, 14, 16, 9}
    if got := KnapsackValue(input_8_0, input_8_1, 10); got != (46) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{6, 3, 4, 2}) { t.Fatalf("case 8: input mutation") }
    if !reflect.DeepEqual(input_8_1, []int64{30, 14, 16, 9}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{10, 20, 30}
    input_9_1 := []int64{60, 100, 100}
    if got := KnapsackValue(input_9_0, input_9_1, 40); got != (160) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{10, 20, 30}) { t.Fatalf("case 9: input mutation") }
    if !reflect.DeepEqual(input_9_1, []int64{60, 100, 100}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{1, 3, 4}
    input_10_1 := []int64{1, 4, 5}
    if got := KnapsackValue(input_10_0, input_10_1, 7); got != (9) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{1, 3, 4}) { t.Fatalf("case 10: input mutation") }
    if !reflect.DeepEqual(input_10_1, []int64{1, 4, 5}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{40}
    input_11_1 := []int64{100}
    if got := KnapsackValue(input_11_0, input_11_1, 40); got != (100) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{40}) { t.Fatalf("case 11: input mutation") }
    if !reflect.DeepEqual(input_11_1, []int64{100}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{7, 6, 5, 4, 3, 2}
    input_12_1 := []int64{5, 6, 7, 8, 9, 10}
    if got := KnapsackValue(input_12_0, input_12_1, 12); got != (27) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{7, 6, 5, 4, 3, 2}) { t.Fatalf("case 12: input mutation") }
    if !reflect.DeepEqual(input_12_1, []int64{5, 6, 7, 8, 9, 10}) { t.Fatalf("case 12: input mutation") }
}
