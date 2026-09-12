from solution import sorted_insert

input_0_0: list[int] = []
assert sorted_insert(input_0_0, 3) == [3], 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [1]
assert sorted_insert(input_1_0, 0) == [0, 1], 'case 1'
assert input_1_0 == [1], 'case 1: input mutation'
input_2_0: list[int] = [1]
assert sorted_insert(input_2_0, 1) == [1, 1], 'case 2'
assert input_2_0 == [1], 'case 2: input mutation'
input_3_0: list[int] = [1]
assert sorted_insert(input_3_0, 2) == [1, 2], 'case 3'
assert input_3_0 == [1], 'case 3: input mutation'
input_4_0: list[int] = [1, 2, 3]
assert sorted_insert(input_4_0, 0) == [0, 1, 2, 3], 'case 4'
assert input_4_0 == [1, 2, 3], 'case 4: input mutation'
input_5_0: list[int] = [1, 2, 3]
assert sorted_insert(input_5_0, 2) == [1, 2, 2, 3], 'case 5'
assert input_5_0 == [1, 2, 3], 'case 5: input mutation'
input_6_0: list[int] = [1, 2, 3]
assert sorted_insert(input_6_0, 4) == [1, 2, 3, 4], 'case 6'
assert input_6_0 == [1, 2, 3], 'case 6: input mutation'
input_7_0: list[int] = [0, 0, 0]
assert sorted_insert(input_7_0, 0) == [0, 0, 0, 0], 'case 7'
assert input_7_0 == [0, 0, 0], 'case 7: input mutation'
input_8_0: list[int] = [-5, -2, 0, 3]
assert sorted_insert(input_8_0, -3) == [-5, -3, -2, 0, 3], 'case 8'
assert input_8_0 == [-5, -2, 0, 3], 'case 8: input mutation'
input_9_0: list[int] = [-5, -2, 0, 3]
assert sorted_insert(input_9_0, 0) == [-5, -2, 0, 0, 3], 'case 9'
assert input_9_0 == [-5, -2, 0, 3], 'case 9: input mutation'
input_10_0: list[int] = [1, 1, 2, 2]
assert sorted_insert(input_10_0, 1) == [1, 1, 1, 2, 2], 'case 10'
assert input_10_0 == [1, 1, 2, 2], 'case 10: input mutation'
input_11_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
assert sorted_insert(input_11_0, 10) == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 11'
assert input_11_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 11: input mutation'
input_12_0: list[int] = [1000]
assert sorted_insert(input_12_0, -1000) == [-1000, 1000], 'case 12'
assert input_12_0 == [1000], 'case 12: input mutation'
