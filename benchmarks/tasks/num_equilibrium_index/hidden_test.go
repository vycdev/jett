package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := EquilibriumIndex(input_0_0); got.Found { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{0}
    if got := EquilibriumIndex(input_1_0); !got.Found || got.Value != 0 { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{0}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{-7}
    if got := EquilibriumIndex(input_2_0); !got.Found || got.Value != 0 { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{-7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{1, 2, 3, 4}
    if got := EquilibriumIndex(input_3_0); got.Found { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{1, 2, 3, 4}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{4, 3, 2, 1}
    if got := EquilibriumIndex(input_4_0); got.Found { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{4, 3, 2, 1}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{2, 2, 2}
    if got := EquilibriumIndex(input_5_0); !got.Found || got.Value != 1 { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{2, 2, 2}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{-3, -2, -1}
    if got := EquilibriumIndex(input_6_0); got.Found { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{-3, -2, -1}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{3, 1, 2, 1, 4}
    if got := EquilibriumIndex(input_7_0); got.Found { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{3, 1, 2, 1, 4}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{5, -1, 5, -1, 5}
    if got := EquilibriumIndex(input_8_0); !got.Found || got.Value != 2 { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{5, -1, 5, -1, 5}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{0, -1, 2, -3, 4, -5}
    if got := EquilibriumIndex(input_9_0); got.Found { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{0, -1, 2, -3, 4, -5}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{9, 3, 7, 1, 8, 2, 6, 4, 5}
    if got := EquilibriumIndex(input_10_0); got.Found { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{9, 3, 7, 1, 8, 2, 6, 4, 5}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{100, -100, 100, 0}
    if got := EquilibriumIndex(input_11_0); !got.Found || got.Value != 0 { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{100, -100, 100, 0}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    if got := EquilibriumIndex(input_12_0); !got.Found || got.Value != 0 { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 12: input mutation") }
    input_13_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := EquilibriumIndex(input_13_0); got.Found { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 13: input mutation") }
    input_14_0 := []int64{20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0}
    if got := EquilibriumIndex(input_14_0); got.Found { t.Fatalf("case 14: got %#v", got) }
    if !reflect.DeepEqual(input_14_0, []int64{20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0}) { t.Fatalf("case 14: input mutation") }
    input_15_0 := []int64{1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000}
    if got := EquilibriumIndex(input_15_0); got.Found { t.Fatalf("case 15: got %#v", got) }
    if !reflect.DeepEqual(input_15_0, []int64{1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000}) { t.Fatalf("case 15: input mutation") }
    input_16_0 := []int64{100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1}
    if got := EquilibriumIndex(input_16_0); got.Found { t.Fatalf("case 16: got %#v", got) }
    if !reflect.DeepEqual(input_16_0, []int64{100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1}) { t.Fatalf("case 16: input mutation") }
    input_17_0 := []int64{1, 3, 5, 2, 2}
    if got := EquilibriumIndex(input_17_0); !got.Found || got.Value != 2 { t.Fatalf("case 17: got %#v", got) }
    if !reflect.DeepEqual(input_17_0, []int64{1, 3, 5, 2, 2}) { t.Fatalf("case 17: input mutation") }
    input_18_0 := []int64{-7, 1, 5, 2, -4, 3, 0}
    if got := EquilibriumIndex(input_18_0); !got.Found || got.Value != 3 { t.Fatalf("case 18: got %#v", got) }
    if !reflect.DeepEqual(input_18_0, []int64{-7, 1, 5, 2, -4, 3, 0}) { t.Fatalf("case 18: input mutation") }
    input_19_0 := []int64{0, 0, 0}
    if got := EquilibriumIndex(input_19_0); !got.Found || got.Value != 0 { t.Fatalf("case 19: got %#v", got) }
    if !reflect.DeepEqual(input_19_0, []int64{0, 0, 0}) { t.Fatalf("case 19: input mutation") }
    input_20_0 := []int64{2, -2, 7}
    if got := EquilibriumIndex(input_20_0); !got.Found || got.Value != 2 { t.Fatalf("case 20: got %#v", got) }
    if !reflect.DeepEqual(input_20_0, []int64{2, -2, 7}) { t.Fatalf("case 20: input mutation") }
    input_21_0 := []int64{7, 2, -2}
    if got := EquilibriumIndex(input_21_0); !got.Found || got.Value != 0 { t.Fatalf("case 21: got %#v", got) }
    if !reflect.DeepEqual(input_21_0, []int64{7, 2, -2}) { t.Fatalf("case 21: input mutation") }
}
