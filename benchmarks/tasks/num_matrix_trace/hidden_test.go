package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := MatrixTrace(input_0_0, 0); got != (0) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{0}
    if got := MatrixTrace(input_1_0, 1); got != (0) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{0}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{7}
    if got := MatrixTrace(input_2_0, 1); got != (7) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{-7}
    if got := MatrixTrace(input_3_0, 1); got != (-7) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{-7}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{1, 2, 3, 4}
    if got := MatrixTrace(input_4_0, 2); got != (5) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{1, 2, 3, 4}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{0, 9, 9, 0}
    if got := MatrixTrace(input_5_0, 2); got != (0) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{0, 9, 9, 0}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{-1, 8, 9, -2}
    if got := MatrixTrace(input_6_0, 2); got != (-3) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{-1, 8, 9, -2}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{1, 2, 3, 4, 5, 6, 7, 8, 9}
    if got := MatrixTrace(input_7_0, 3); got != (15) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{1, 2, 3, 4, 5, 6, 7, 8, 9}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{0, 0, 0, 0, 1, 0, 0, 0, 0}
    if got := MatrixTrace(input_8_0, 3); got != (1) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{0, 0, 0, 0, 1, 0, 0, 0, 0}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15}
    if got := MatrixTrace(input_9_0, 4); got != (30) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000}
    if got := MatrixTrace(input_10_0, 8); got != (8000) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{0, -1, 2, -3, 4, -5, 6, -7, 8, -9, 10, -11, 12, -13, 14, -15, 16, -17, 18, -19, 20, -21, 22, -23, 24}
    if got := MatrixTrace(input_11_0, 5); got != (60) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{0, -1, 2, -3, 4, -5, 6, -7, 8, -9, 10, -11, 12, -13, 14, -15, 16, -17, 18, -19, 20, -21, 22, -23, 24}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    if got := MatrixTrace(input_12_0, 7); got != (0) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 12: input mutation") }
}
