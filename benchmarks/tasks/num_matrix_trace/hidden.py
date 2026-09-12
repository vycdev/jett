from solution import matrix_trace

input_0_0: list[int] = []
assert matrix_trace(input_0_0, 0) == 0, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [0]
assert matrix_trace(input_1_0, 1) == 0, 'case 1'
assert input_1_0 == [0], 'case 1: input mutation'
input_2_0: list[int] = [7]
assert matrix_trace(input_2_0, 1) == 7, 'case 2'
assert input_2_0 == [7], 'case 2: input mutation'
input_3_0: list[int] = [-7]
assert matrix_trace(input_3_0, 1) == -7, 'case 3'
assert input_3_0 == [-7], 'case 3: input mutation'
input_4_0: list[int] = [1, 2, 3, 4]
assert matrix_trace(input_4_0, 2) == 5, 'case 4'
assert input_4_0 == [1, 2, 3, 4], 'case 4: input mutation'
input_5_0: list[int] = [0, 9, 9, 0]
assert matrix_trace(input_5_0, 2) == 0, 'case 5'
assert input_5_0 == [0, 9, 9, 0], 'case 5: input mutation'
input_6_0: list[int] = [-1, 8, 9, -2]
assert matrix_trace(input_6_0, 2) == -3, 'case 6'
assert input_6_0 == [-1, 8, 9, -2], 'case 6: input mutation'
input_7_0: list[int] = [1, 2, 3, 4, 5, 6, 7, 8, 9]
assert matrix_trace(input_7_0, 3) == 15, 'case 7'
assert input_7_0 == [1, 2, 3, 4, 5, 6, 7, 8, 9], 'case 7: input mutation'
input_8_0: list[int] = [0, 0, 0, 0, 1, 0, 0, 0, 0]
assert matrix_trace(input_8_0, 3) == 1, 'case 8'
assert input_8_0 == [0, 0, 0, 0, 1, 0, 0, 0, 0], 'case 8: input mutation'
input_9_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
assert matrix_trace(input_9_0, 4) == 30, 'case 9'
assert input_9_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], 'case 9: input mutation'
input_10_0: list[int] = [1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000]
assert matrix_trace(input_10_0, 8) == 8000, 'case 10'
assert input_10_0 == [1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, -1000, 1000], 'case 10: input mutation'
input_11_0: list[int] = [0, -1, 2, -3, 4, -5, 6, -7, 8, -9, 10, -11, 12, -13, 14, -15, 16, -17, 18, -19, 20, -21, 22, -23, 24]
assert matrix_trace(input_11_0, 5) == 60, 'case 11'
assert input_11_0 == [0, -1, 2, -3, 4, -5, 6, -7, 8, -9, 10, -11, 12, -13, 14, -15, 16, -17, 18, -19, 20, -21, 22, -23, 24], 'case 11: input mutation'
input_12_0: list[int] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
assert matrix_trace(input_12_0, 7) == 0, 'case 12'
assert input_12_0 == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'case 12: input mutation'
