from solution import max_subarray

input_0_0: list[int] = []
assert max_subarray(input_0_0) == None, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [0]
assert max_subarray(input_1_0) == 0, 'case 1'
assert input_1_0 == [0], 'case 1: input mutation'
input_2_0: list[int] = [-7]
assert max_subarray(input_2_0) == -7, 'case 2'
assert input_2_0 == [-7], 'case 2: input mutation'
input_3_0: list[int] = [1, 2, 3, 4]
assert max_subarray(input_3_0) == 10, 'case 3'
assert input_3_0 == [1, 2, 3, 4], 'case 3: input mutation'
input_4_0: list[int] = [4, 3, 2, 1]
assert max_subarray(input_4_0) == 10, 'case 4'
assert input_4_0 == [4, 3, 2, 1], 'case 4: input mutation'
input_5_0: list[int] = [2, 2, 2]
assert max_subarray(input_5_0) == 6, 'case 5'
assert input_5_0 == [2, 2, 2], 'case 5: input mutation'
input_6_0: list[int] = [-3, -2, -1]
assert max_subarray(input_6_0) == -1, 'case 6'
assert input_6_0 == [-3, -2, -1], 'case 6: input mutation'
input_7_0: list[int] = [3, 1, 2, 1, 4]
assert max_subarray(input_7_0) == 11, 'case 7'
assert input_7_0 == [3, 1, 2, 1, 4], 'case 7: input mutation'
input_8_0: list[int] = [5, -1, 5, -1, 5]
assert max_subarray(input_8_0) == 13, 'case 8'
assert input_8_0 == [5, -1, 5, -1, 5], 'case 8: input mutation'
input_9_0: list[int] = [0, -1, 2, -3, 4, -5]
assert max_subarray(input_9_0) == 4, 'case 9'
assert input_9_0 == [0, -1, 2, -3, 4, -5], 'case 9: input mutation'
input_10_0: list[int] = [9, 3, 7, 1, 8, 2, 6, 4, 5]
assert max_subarray(input_10_0) == 45, 'case 10'
assert input_10_0 == [9, 3, 7, 1, 8, 2, 6, 4, 5], 'case 10: input mutation'
input_11_0: list[int] = [100, -100, 100, 0]
assert max_subarray(input_11_0) == 100, 'case 11'
assert input_11_0 == [100, -100, 100, 0], 'case 11: input mutation'
input_12_0: list[int] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
assert max_subarray(input_12_0) == 0, 'case 12'
assert input_12_0 == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'case 12: input mutation'
input_13_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
assert max_subarray(input_13_0) == 190, 'case 13'
assert input_13_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 13: input mutation'
input_14_0: list[int] = [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
assert max_subarray(input_14_0) == 210, 'case 14'
assert input_14_0 == [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0], 'case 14: input mutation'
input_15_0: list[int] = [1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000]
assert max_subarray(input_15_0) == 1000, 'case 15'
assert input_15_0 == [1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000], 'case 15: input mutation'
input_16_0: list[int] = [100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
assert max_subarray(input_16_0) == 5050, 'case 16'
assert input_16_0 == [100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1], 'case 16: input mutation'
input_17_0: list[int] = [-8, -2, -3]
assert max_subarray(input_17_0) == -2, 'case 17'
assert input_17_0 == [-8, -2, -3], 'case 17: input mutation'
input_18_0: list[int] = [-2, 1, -3, 4, -1, 2, 1, -5, 4]
assert max_subarray(input_18_0) == 6, 'case 18'
assert input_18_0 == [-2, 1, -3, 4, -1, 2, 1, -5, 4], 'case 18: input mutation'
input_19_0: list[int] = [5, -10, 6]
assert max_subarray(input_19_0) == 6, 'case 19'
assert input_19_0 == [5, -10, 6], 'case 19: input mutation'
