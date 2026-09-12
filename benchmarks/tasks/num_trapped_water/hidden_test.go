package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := TrappedWater(input_0_0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{0}
    if got := TrappedWater(input_1_0); got != (0) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{0}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{5}
    if got := TrappedWater(input_2_0); got != (0) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{5}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{5, 0}
    if got := TrappedWater(input_3_0); got != (0) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{5, 0}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{0, 5}
    if got := TrappedWater(input_4_0); got != (0) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{0, 5}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{3, 3, 3}
    if got := TrappedWater(input_5_0); got != (0) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{3, 3, 3}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{3, 0, 3}
    if got := TrappedWater(input_6_0); got != (3) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{3, 0, 3}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{5, 0, 2}
    if got := TrappedWater(input_7_0); got != (2) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{5, 0, 2}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{2, 0, 5}
    if got := TrappedWater(input_8_0); got != (2) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{2, 0, 5}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{3, 0, 2, 0, 4}
    if got := TrappedWater(input_9_0); got != (7) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{3, 0, 2, 0, 4}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1}
    if got := TrappedWater(input_10_0); got != (6) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{5, 4, 3, 2, 1}
    if got := TrappedWater(input_11_0); got != (0) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{5, 4, 3, 2, 1}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{1, 2, 3, 4, 5}
    if got := TrappedWater(input_12_0); got != (0) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{1, 2, 3, 4, 5}) { t.Fatalf("case 12: input mutation") }
    input_13_0 := []int64{4, 2, 0, 3, 2, 5}
    if got := TrappedWater(input_13_0); got != (9) { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_0, []int64{4, 2, 0, 3, 2, 5}) { t.Fatalf("case 13: input mutation") }
    input_14_0 := []int64{3, 0, 0, 3}
    if got := TrappedWater(input_14_0); got != (6) { t.Fatalf("case 14: got %#v", got) }
    if !reflect.DeepEqual(input_14_0, []int64{3, 0, 0, 3}) { t.Fatalf("case 14: input mutation") }
    input_15_0 := []int64{1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1000}
    if got := TrappedWater(input_15_0); got != (98000) { t.Fatalf("case 15: got %#v", got) }
    if !reflect.DeepEqual(input_15_0, []int64{1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1000}) { t.Fatalf("case 15: input mutation") }
    input_16_0 := []int64{1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0}
    if got := TrappedWater(input_16_0); got != (49000) { t.Fatalf("case 16: got %#v", got) }
    if !reflect.DeepEqual(input_16_0, []int64{1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0}) { t.Fatalf("case 16: input mutation") }
    input_17_0 := []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    if got := TrappedWater(input_17_0); got != (0) { t.Fatalf("case 17: got %#v", got) }
    if !reflect.DeepEqual(input_17_0, []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 17: input mutation") }
}
