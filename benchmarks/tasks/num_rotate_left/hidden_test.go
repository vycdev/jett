package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := RotateLeft(input_0_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{}) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{}
    if got := RotateLeft(input_1_0, 100); !reflect.DeepEqual(append([]int64{}, got...), []int64{}) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{7}
    if got := RotateLeft(input_2_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{7}) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{7}
    if got := RotateLeft(input_3_0, 999); !reflect.DeepEqual(append([]int64{}, got...), []int64{7}) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{7}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{1, 2, 3}
    if got := RotateLeft(input_4_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{1, 2, 3}) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{1, 2, 3}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{1, 2, 3}
    if got := RotateLeft(input_5_0, 1); !reflect.DeepEqual(append([]int64{}, got...), []int64{2, 3, 1}) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{1, 2, 3}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{1, 2, 3}
    if got := RotateLeft(input_6_0, 2); !reflect.DeepEqual(append([]int64{}, got...), []int64{3, 1, 2}) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{1, 2, 3}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{1, 2, 3}
    if got := RotateLeft(input_7_0, 3); !reflect.DeepEqual(append([]int64{}, got...), []int64{1, 2, 3}) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{1, 2, 3}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{1, 2, 3}
    if got := RotateLeft(input_8_0, 4); !reflect.DeepEqual(append([]int64{}, got...), []int64{2, 3, 1}) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{1, 2, 3}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{-1, 0, 1, 0}
    if got := RotateLeft(input_9_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, -1, 0, 1}) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{-1, 0, 1, 0}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{2, 2, 3, 2}
    if got := RotateLeft(input_10_0, 2); !reflect.DeepEqual(append([]int64{}, got...), []int64{3, 2, 2, 2}) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{2, 2, 3, 2}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := RotateLeft(input_11_0, 1000000); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{0, 1, 2, 3, 4, 5, 6}
    if got := RotateLeft(input_12_0, 999999); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 1, 2, 3, 4, 5, 6}) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{0, 1, 2, 3, 4, 5, 6}) { t.Fatalf("case 12: input mutation") }
}
