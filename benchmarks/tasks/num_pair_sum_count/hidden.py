from solution import pair_sum_count

input_0_0: list[int] = []
assert pair_sum_count(input_0_0, 0) == 0, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [0]
assert pair_sum_count(input_1_0, 0) == 0, 'case 1'
assert input_1_0 == [0], 'case 1: input mutation'
input_2_0: list[int] = [0, 0]
assert pair_sum_count(input_2_0, 0) == 1, 'case 2'
assert input_2_0 == [0, 0], 'case 2: input mutation'
input_3_0: list[int] = [0, 0, 0, 0]
assert pair_sum_count(input_3_0, 0) == 6, 'case 3'
assert input_3_0 == [0, 0, 0, 0], 'case 3: input mutation'
input_4_0: list[int] = [1, 2, 3, 4]
assert pair_sum_count(input_4_0, 5) == 2, 'case 4'
assert input_4_0 == [1, 2, 3, 4], 'case 4: input mutation'
input_5_0: list[int] = [1, 1, 1, 2, 2]
assert pair_sum_count(input_5_0, 3) == 6, 'case 5'
assert input_5_0 == [1, 1, 1, 2, 2], 'case 5: input mutation'
input_6_0: list[int] = [-2, -1, 0, 1, 2]
assert pair_sum_count(input_6_0, 0) == 2, 'case 6'
assert input_6_0 == [-2, -1, 0, 1, 2], 'case 6: input mutation'
input_7_0: list[int] = [5, -5, 5, -5]
assert pair_sum_count(input_7_0, 0) == 4, 'case 7'
assert input_7_0 == [5, -5, 5, -5], 'case 7: input mutation'
input_8_0: list[int] = [1, 2, 3]
assert pair_sum_count(input_8_0, 10) == 0, 'case 8'
assert input_8_0 == [1, 2, 3], 'case 8: input mutation'
input_9_0: list[int] = [1000, 1000]
assert pair_sum_count(input_9_0, 2000) == 1, 'case 9'
assert input_9_0 == [1000, 1000], 'case 9: input mutation'
input_10_0: list[int] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1]
assert pair_sum_count(input_10_0, 2) == 4950, 'case 10'
assert input_10_0 == [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 'case 10: input mutation'
input_11_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
assert pair_sum_count(input_11_0, 19) == 10, 'case 11'
assert input_11_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 11: input mutation'
input_12_0: list[int] = [2, 2, 2]
assert pair_sum_count(input_12_0, 4) == 3, 'case 12'
assert input_12_0 == [2, 2, 2], 'case 12: input mutation'
