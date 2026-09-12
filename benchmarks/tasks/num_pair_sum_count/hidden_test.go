package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := PairSumCount(input_0_0, 0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{0}
    if got := PairSumCount(input_1_0, 0); got != (0) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{0}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{0, 0}
    if got := PairSumCount(input_2_0, 0); got != (1) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{0, 0}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{0, 0, 0, 0}
    if got := PairSumCount(input_3_0, 0); got != (6) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{0, 0, 0, 0}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{1, 2, 3, 4}
    if got := PairSumCount(input_4_0, 5); got != (2) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{1, 2, 3, 4}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{1, 1, 1, 2, 2}
    if got := PairSumCount(input_5_0, 3); got != (6) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{1, 1, 1, 2, 2}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{-2, -1, 0, 1, 2}
    if got := PairSumCount(input_6_0, 0); got != (2) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{-2, -1, 0, 1, 2}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{5, -5, 5, -5}
    if got := PairSumCount(input_7_0, 0); got != (4) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{5, -5, 5, -5}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{1, 2, 3}
    if got := PairSumCount(input_8_0, 10); got != (0) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{1, 2, 3}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{1000, 1000}
    if got := PairSumCount(input_9_0, 2000); got != (1) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{1000, 1000}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}
    if got := PairSumCount(input_10_0, 2); got != (4950) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := PairSumCount(input_11_0, 19); got != (10) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{2, 2, 2}
    if got := PairSumCount(input_12_0, 4); got != (3) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{2, 2, 2}) { t.Fatalf("case 12: input mutation") }
}
