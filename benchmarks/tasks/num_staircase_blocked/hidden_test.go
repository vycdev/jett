package benchmark

import ("testing"; "reflect")

func TestHidden(t *testing.T) {
    _ = reflect.DeepEqual

    input_0_1 := []int64{}
    if got := StaircaseBlocked(0, input_0_1); got != (1) { t.Fatalf("case 0: got %#v", got) }
    if !reflect.DeepEqual(input_0_1, []int64{}) { t.Fatalf("case 0: input mutation") }
    input_1_1 := []int64{}
    if got := StaircaseBlocked(1, input_1_1); got != (1) { t.Fatalf("case 1: got %#v", got) }
    if !reflect.DeepEqual(input_1_1, []int64{}) { t.Fatalf("case 1: input mutation") }
    input_2_1 := []int64{1}
    if got := StaircaseBlocked(1, input_2_1); got != (0) { t.Fatalf("case 2: got %#v", got) }
    if !reflect.DeepEqual(input_2_1, []int64{1}) { t.Fatalf("case 2: input mutation") }
    input_3_1 := []int64{1}
    if got := StaircaseBlocked(2, input_3_1); got != (1) { t.Fatalf("case 3: got %#v", got) }
    if !reflect.DeepEqual(input_3_1, []int64{1}) { t.Fatalf("case 3: input mutation") }
    input_4_1 := []int64{2}
    if got := StaircaseBlocked(2, input_4_1); got != (0) { t.Fatalf("case 4: got %#v", got) }
    if !reflect.DeepEqual(input_4_1, []int64{2}) { t.Fatalf("case 4: input mutation") }
    input_5_1 := []int64{}
    if got := StaircaseBlocked(3, input_5_1); got != (3) { t.Fatalf("case 5: got %#v", got) }
    if !reflect.DeepEqual(input_5_1, []int64{}) { t.Fatalf("case 5: input mutation") }
    input_6_1 := []int64{2}
    if got := StaircaseBlocked(4, input_6_1); got != (1) { t.Fatalf("case 6: got %#v", got) }
    if !reflect.DeepEqual(input_6_1, []int64{2}) { t.Fatalf("case 6: input mutation") }
    input_7_1 := []int64{2, 3}
    if got := StaircaseBlocked(5, input_7_1); got != (0) { t.Fatalf("case 7: got %#v", got) }
    if !reflect.DeepEqual(input_7_1, []int64{2, 3}) { t.Fatalf("case 7: input mutation") }
    input_8_1 := []int64{5, 1}
    if got := StaircaseBlocked(6, input_8_1); got != (2) { t.Fatalf("case 8: got %#v", got) }
    if !reflect.DeepEqual(input_8_1, []int64{5, 1}) { t.Fatalf("case 8: input mutation") }
    input_9_1 := []int64{4, 4, 7}
    if got := StaircaseBlocked(10, input_9_1); got != (6) { t.Fatalf("case 9: got %#v", got) }
    if !reflect.DeepEqual(input_9_1, []int64{4, 4, 7}) { t.Fatalf("case 9: input mutation") }
    input_10_1 := []int64{}
    if got := StaircaseBlocked(20, input_10_1); got != (10946) { t.Fatalf("case 10: got %#v", got) }
    if !reflect.DeepEqual(input_10_1, []int64{}) { t.Fatalf("case 10: input mutation") }
    input_11_1 := []int64{}
    if got := StaircaseBlocked(40, input_11_1); got != (165580141) { t.Fatalf("case 11: got %#v", got) }
    if !reflect.DeepEqual(input_11_1, []int64{}) { t.Fatalf("case 11: input mutation") }
    input_12_1 := []int64{39}
    if got := StaircaseBlocked(40, input_12_1); got != (63245986) { t.Fatalf("case 12: got %#v", got) }
    if !reflect.DeepEqual(input_12_1, []int64{39}) { t.Fatalf("case 12: input mutation") }
    input_13_1 := []int64{1, 2}
    if got := StaircaseBlocked(8, input_13_1); got != (0) { t.Fatalf("case 13: got %#v", got) }
    if !reflect.DeepEqual(input_13_1, []int64{1, 2}) { t.Fatalf("case 13: input mutation") }
}
