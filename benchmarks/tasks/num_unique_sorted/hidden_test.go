package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := UniqueSorted(input_0_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{}) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{0}
    if got := UniqueSorted(input_1_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0}) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{0}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{-7}
    if got := UniqueSorted(input_2_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-7}) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{-7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{1, 2, 3, 4}
    if got := UniqueSorted(input_3_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{1, 2, 3, 4}) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{1, 2, 3, 4}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{2, 2, 2}
    if got := UniqueSorted(input_4_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{2}) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{2, 2, 2}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{-3, -2, -1}
    if got := UniqueSorted(input_5_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-3, -2, -1}) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{-3, -2, -1}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{1, 1, 2, 3, 4}
    if got := UniqueSorted(input_6_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{1, 2, 3, 4}) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{1, 1, 2, 3, 4}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{-1, -1, 5, 5, 5}
    if got := UniqueSorted(input_7_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-1, 5}) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{-1, -1, 5, 5, 5}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{-5, -3, -1, 0, 2, 4}
    if got := UniqueSorted(input_8_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-5, -3, -1, 0, 2, 4}) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{-5, -3, -1, 0, 2, 4}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{1, 2, 3, 4, 5, 6, 7, 8, 9}
    if got := UniqueSorted(input_9_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{1, 2, 3, 4, 5, 6, 7, 8, 9}) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{1, 2, 3, 4, 5, 6, 7, 8, 9}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{-100, 0, 100, 100}
    if got := UniqueSorted(input_10_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-100, 0, 100}) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{-100, 0, 100, 100}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    if got := UniqueSorted(input_11_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0}) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := UniqueSorted(input_12_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 12: input mutation") }
    input_13_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20}
    if got := UniqueSorted(input_13_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20}) { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20}) { t.Fatalf("case 13: input mutation") }
    input_14_0 := []int64{-1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000}
    if got := UniqueSorted(input_14_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-1000, 1000}) { t.Fatalf("case 14: got %#v", got) }
    if !reflect.DeepEqual(input_14_0, []int64{-1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000, 1000}) { t.Fatalf("case 14: input mutation") }
    input_15_0 := []int64{1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100}
    if got := UniqueSorted(input_15_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100}) { t.Fatalf("case 15: got %#v", got) }
    if !reflect.DeepEqual(input_15_0, []int64{1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100}) { t.Fatalf("case 15: input mutation") }
    input_16_0 := []int64{-1000, -1000, 0, 1000, 1000}
    if got := UniqueSorted(input_16_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{-1000, 0, 1000}) { t.Fatalf("case 16: got %#v", got) }
    if !reflect.DeepEqual(input_16_0, []int64{-1000, -1000, 0, 1000, 1000}) { t.Fatalf("case 16: input mutation") }
    input_17_0 := []int64{1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}
    if got := UniqueSorted(input_17_0); !reflect.DeepEqual(append([]int64{}, got...), []int64{1}) { t.Fatalf("case 17: got %#v", got) }
    if !reflect.DeepEqual(input_17_0, []int64{1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1}) { t.Fatalf("case 17: input mutation") }
}
