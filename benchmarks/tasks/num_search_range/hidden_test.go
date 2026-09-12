package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_0 := []int64{}
    if got := FindSearchRange(input_0_0, 0); got != (SearchRange{first: -1, last: -1}) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_0, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_0 := []int64{1}
    if got := FindSearchRange(input_1_0, 1); got != (SearchRange{first: 0, last: 0}) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_0, []int64{1}) { t.Fatalf("case 1: input mutation") }
    input_2_0 := []int64{1}
    if got := FindSearchRange(input_2_0, 0); got != (SearchRange{first: -1, last: -1}) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_0, []int64{1}) { t.Fatalf("case 2: input mutation") }
    input_3_0 := []int64{1}
    if got := FindSearchRange(input_3_0, 2); got != (SearchRange{first: -1, last: -1}) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_0, []int64{1}) { t.Fatalf("case 3: input mutation") }
    input_4_0 := []int64{1, 2, 2, 2, 3}
    if got := FindSearchRange(input_4_0, 2); got != (SearchRange{first: 1, last: 3}) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_0, []int64{1, 2, 2, 2, 3}) { t.Fatalf("case 4: input mutation") }
    input_5_0 := []int64{1, 1, 2}
    if got := FindSearchRange(input_5_0, 1); got != (SearchRange{first: 0, last: 1}) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_0, []int64{1, 1, 2}) { t.Fatalf("case 5: input mutation") }
    input_6_0 := []int64{1, 2, 2}
    if got := FindSearchRange(input_6_0, 2); got != (SearchRange{first: 1, last: 2}) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_0, []int64{1, 2, 2}) { t.Fatalf("case 6: input mutation") }
    input_7_0 := []int64{0, 0, 0}
    if got := FindSearchRange(input_7_0, 0); got != (SearchRange{first: 0, last: 2}) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_0, []int64{0, 0, 0}) { t.Fatalf("case 7: input mutation") }
    input_8_0 := []int64{-5, -2, -2, 0, 3}
    if got := FindSearchRange(input_8_0, -2); got != (SearchRange{first: 1, last: 2}) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_0, []int64{-5, -2, -2, 0, 3}) { t.Fatalf("case 8: input mutation") }
    input_9_0 := []int64{-5, -2, 0, 3}
    if got := FindSearchRange(input_9_0, 1); got != (SearchRange{first: -1, last: -1}) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_0, []int64{-5, -2, 0, 3}) { t.Fatalf("case 9: input mutation") }
    input_10_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := FindSearchRange(input_10_0, 10); got != (SearchRange{first: 10, last: 10}) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 10: input mutation") }
    input_11_0 := []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}
    if got := FindSearchRange(input_11_0, 20); got != (SearchRange{first: -1, last: -1}) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_0, []int64{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19}) { t.Fatalf("case 11: input mutation") }
    input_12_0 := []int64{7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7}
    if got := FindSearchRange(input_12_0, 7); got != (SearchRange{first: 0, last: 99}) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_0, []int64{7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7}) { t.Fatalf("case 12: input mutation") }
}
