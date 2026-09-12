package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := WindowPeak(input_0_0, 0); got.Found { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{}
    if got := WindowPeak(input_1_0, 1); got.Found { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{7}
    if got := WindowPeak(input_2_0, 0); got.Found { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{7}
    if got := WindowPeak(input_3_0, 1); !got.Found || got.Value != 7 { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{7}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{7}
    if got := WindowPeak(input_4_0, 2); got.Found { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{7}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{1, 2, 3, 4}
    if got := WindowPeak(input_5_0, 2); !got.Found || got.Value != 7 { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{1, 2, 3, 4}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{-5, -2, -7}
    if got := WindowPeak(input_6_0, 2); !got.Found || got.Value != -7 { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{-5, -2, -7}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{2, -1, 2, -1, 2}
    if got := WindowPeak(input_7_0, 3); !got.Found || got.Value != 3 { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{2, -1, 2, -1, 2}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{5, -9, 5}
    if got := WindowPeak(input_8_0, 1); !got.Found || got.Value != 5 { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{5, -9, 5}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{5, -9, 5}
    if got := WindowPeak(input_9_0, 3); !got.Found || got.Value != 1 { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{5, -9, 5}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{0, 0, 0}
    if got := WindowPeak(input_10_0, 2); !got.Found || got.Value != 0 { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{0, 0, 0}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{10, -5, -5, 10}
    if got := WindowPeak(input_11_0, 2); !got.Found || got.Value != 5 { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{10, -5, -5, 10}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := WindowPeak(input_12_0, 5); !got.Found || got.Value != 85 { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 12: input mutation") }
    input_13_0 := []int64{20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0}
    if got := WindowPeak(input_13_0, 7); !got.Found || got.Value != 119 { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_0, []int64{20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0}) { t.Fatalf("case 13: input mutation") }
}
