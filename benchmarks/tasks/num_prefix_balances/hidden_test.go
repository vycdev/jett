package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := PrefixBalances(input_0_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0}) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{0}
    if got := PrefixBalances(input_1_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{7, 7}) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{0}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{-7}
    if got := PrefixBalances(input_2_0, -3); !reflect.DeepEqual(append([]int64{}, got...), []int64{-3, -10}) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{-7}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{1, 2, 3, 4}
    if got := PrefixBalances(input_3_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 1, 3, 6, 10}) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{1, 2, 3, 4}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{4, 3, 2, 1}
    if got := PrefixBalances(input_4_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{7, 11, 14, 16, 17}) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{4, 3, 2, 1}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{2, 2, 2}
    if got := PrefixBalances(input_5_0, -3); !reflect.DeepEqual(append([]int64{}, got...), []int64{-3, -1, 1, 3}) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{2, 2, 2}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{-3, -2, -1}
    if got := PrefixBalances(input_6_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, -3, -5, -6}) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{-3, -2, -1}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{3, 1, 2, 1, 4}
    if got := PrefixBalances(input_7_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{7, 10, 11, 13, 14, 18}) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{3, 1, 2, 1, 4}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{5, -1, 5, -1, 5}
    if got := PrefixBalances(input_8_0, -3); !reflect.DeepEqual(append([]int64{}, got...), []int64{-3, 2, 1, 6, 5, 10}) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{5, -1, 5, -1, 5}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{0, -1, 2, -3, 4, -5}
    if got := PrefixBalances(input_9_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 0, -1, 1, -2, 2, -3}) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{0, -1, 2, -3, 4, -5}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{9, 3, 7, 1, 8, 2, 6, 4, 5}
    if got := PrefixBalances(input_10_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{7, 16, 19, 26, 27, 35, 37, 43, 47, 52}) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{9, 3, 7, 1, 8, 2, 6, 4, 5}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{100, -100, 100, 0}
    if got := PrefixBalances(input_11_0, -3); !reflect.DeepEqual(append([]int64{}, got...), []int64{-3, 97, -3, 97, 97}) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{100, -100, 100, 0}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}
    if got := PrefixBalances(input_12_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0}) { t.Fatalf("case 12: input mutation") }
    input_13_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := PrefixBalances(input_13_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{7, 7, 8, 10, 13, 17, 22, 28, 35, 43, 52, 62, 73, 85, 98, 112, 127, 143, 160, 178, 197}) { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 13: input mutation") }
    input_14_0 := []int64{20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0}
    if got := PrefixBalances(input_14_0, -3); !reflect.DeepEqual(append([]int64{}, got...), []int64{-3, 17, 36, 54, 71, 87, 102, 116, 129, 141, 152, 162, 171, 179, 186, 192, 197, 201, 204, 206, 207, 207}) { t.Fatalf("case 14: got %#v", got) }
    if !reflect.DeepEqual(input_14_0, []int64{20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0}) { t.Fatalf("case 14: input mutation") }
    input_15_0 := []int64{1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000}
    if got := PrefixBalances(input_15_0, 0); !reflect.DeepEqual(append([]int64{}, got...), []int64{0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0}) { t.Fatalf("case 15: got %#v", got) }
    if !reflect.DeepEqual(input_15_0, []int64{1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000}) { t.Fatalf("case 15: input mutation") }
    input_16_0 := []int64{100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1}
    if got := PrefixBalances(input_16_0, 7); !reflect.DeepEqual(append([]int64{}, got...), []int64{7, 107, 206, 304, 401, 497, 592, 686, 779, 871, 962, 1052, 1141, 1229, 1316, 1402, 1487, 1571, 1654, 1736, 1817, 1897, 1976, 2054, 2131, 2207, 2282, 2356, 2429, 2501, 2572, 2642, 2711, 2779, 2846, 2912, 2977, 3041, 3104, 3166, 3227, 3287, 3346, 3404, 3461, 3517, 3572, 3626, 3679, 3731, 3782, 3832, 3881, 3929, 3976, 4022, 4067, 4111, 4154, 4196, 4237, 4277, 4316, 4354, 4391, 4427, 4462, 4496, 4529, 4561, 4592, 4622, 4651, 4679, 4706, 4732, 4757, 4781, 4804, 4826, 4847, 4867, 4886, 4904, 4921, 4937, 4952, 4966, 4979, 4991, 5002, 5012, 5021, 5029, 5036, 5042, 5047, 5051, 5054, 5056, 5057}) { t.Fatalf("case 16: got %#v", got) }
    if !reflect.DeepEqual(input_16_0, []int64{100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1}) { t.Fatalf("case 16: input mutation") }
}
