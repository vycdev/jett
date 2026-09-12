from solution import polynomial_value

input_0_0: list[int] = []
assert polynomial_value(input_0_0, 3) == 0, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [5]
assert polynomial_value(input_1_0, 0) == 5, 'case 1'
assert input_1_0 == [5], 'case 1: input mutation'
input_2_0: list[int] = [5]
assert polynomial_value(input_2_0, -10) == 5, 'case 2'
assert input_2_0 == [5], 'case 2: input mutation'
input_3_0: list[int] = [1, 2, 3]
assert polynomial_value(input_3_0, 0) == 1, 'case 3'
assert input_3_0 == [1, 2, 3], 'case 3: input mutation'
input_4_0: list[int] = [1, 2, 3]
assert polynomial_value(input_4_0, 1) == 6, 'case 4'
assert input_4_0 == [1, 2, 3], 'case 4: input mutation'
input_5_0: list[int] = [1, 2, 3]
assert polynomial_value(input_5_0, 2) == 17, 'case 5'
assert input_5_0 == [1, 2, 3], 'case 5: input mutation'
input_6_0: list[int] = [1, 2, 3]
assert polynomial_value(input_6_0, -2) == 9, 'case 6'
assert input_6_0 == [1, 2, 3], 'case 6: input mutation'
input_7_0: list[int] = [0, 0, 1]
assert polynomial_value(input_7_0, 3) == 9, 'case 7'
assert input_7_0 == [0, 0, 1], 'case 7: input mutation'
input_8_0: list[int] = [1, -1, 1, -1]
assert polynomial_value(input_8_0, -1) == 4, 'case 8'
assert input_8_0 == [1, -1, 1, -1], 'case 8: input mutation'
input_9_0: list[int] = [100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100]
assert polynomial_value(input_9_0, 10) == 11111111111100, 'case 9'
assert input_9_0 == [100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100], 'case 9: input mutation'
input_10_0: list[int] = [-100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100]
assert polynomial_value(input_10_0, -10) == 9090909090900, 'case 10'
assert input_10_0 == [-100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100], 'case 10: input mutation'
input_11_0: list[int] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
assert polynomial_value(input_11_0, 10) == 0, 'case 11'
assert input_11_0 == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'case 11: input mutation'
input_12_0: list[int] = [3, 0, -4, 0, 5]
assert polynomial_value(input_12_0, 2) == 67, 'case 12'
assert input_12_0 == [3, 0, -4, 0, 5], 'case 12: input mutation'
