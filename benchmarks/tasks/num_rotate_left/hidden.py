from solution import rotate_left

input_0_0: list[int] = []
assert rotate_left(input_0_0, 0) == [], 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = []
assert rotate_left(input_1_0, 100) == [], 'case 1'
assert input_1_0 == [], 'case 1: input mutation'
input_2_0: list[int] = [7]
assert rotate_left(input_2_0, 0) == [7], 'case 2'
assert input_2_0 == [7], 'case 2: input mutation'
input_3_0: list[int] = [7]
assert rotate_left(input_3_0, 999) == [7], 'case 3'
assert input_3_0 == [7], 'case 3: input mutation'
input_4_0: list[int] = [1, 2, 3]
assert rotate_left(input_4_0, 0) == [1, 2, 3], 'case 4'
assert input_4_0 == [1, 2, 3], 'case 4: input mutation'
input_5_0: list[int] = [1, 2, 3]
assert rotate_left(input_5_0, 1) == [2, 3, 1], 'case 5'
assert input_5_0 == [1, 2, 3], 'case 5: input mutation'
input_6_0: list[int] = [1, 2, 3]
assert rotate_left(input_6_0, 2) == [3, 1, 2], 'case 6'
assert input_6_0 == [1, 2, 3], 'case 6: input mutation'
input_7_0: list[int] = [1, 2, 3]
assert rotate_left(input_7_0, 3) == [1, 2, 3], 'case 7'
assert input_7_0 == [1, 2, 3], 'case 7: input mutation'
input_8_0: list[int] = [1, 2, 3]
assert rotate_left(input_8_0, 4) == [2, 3, 1], 'case 8'
assert input_8_0 == [1, 2, 3], 'case 8: input mutation'
input_9_0: list[int] = [-1, 0, 1, 0]
assert rotate_left(input_9_0, 7) == [0, -1, 0, 1], 'case 9'
assert input_9_0 == [-1, 0, 1, 0], 'case 9: input mutation'
input_10_0: list[int] = [2, 2, 3, 2]
assert rotate_left(input_10_0, 2) == [3, 2, 2, 2], 'case 10'
assert input_10_0 == [2, 2, 3, 2], 'case 10: input mutation'
input_11_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
assert rotate_left(input_11_0, 1000000) == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 11'
assert input_11_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 11: input mutation'
input_12_0: list[int] = [0, 1, 2, 3, 4, 5, 6]
assert rotate_left(input_12_0, 999999) == [0, 1, 2, 3, 4, 5, 6], 'case 12'
assert input_12_0 == [0, 1, 2, 3, 4, 5, 6], 'case 12: input mutation'
