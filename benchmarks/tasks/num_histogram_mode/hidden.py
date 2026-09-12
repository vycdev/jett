from solution import histogram_mode

input_0_0: list[int] = []
case_0 = histogram_mode(input_0_0)
assert case_0.value == 0, 'case 0: value'
assert case_0.count == 0, 'case 0: count'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [0]
case_1 = histogram_mode(input_1_0)
assert case_1.value == 0, 'case 1: value'
assert case_1.count == 1, 'case 1: count'
assert input_1_0 == [0], 'case 1: input mutation'
input_2_0: list[int] = [-7]
case_2 = histogram_mode(input_2_0)
assert case_2.value == -7, 'case 2: value'
assert case_2.count == 1, 'case 2: count'
assert input_2_0 == [-7], 'case 2: input mutation'
input_3_0: list[int] = [1, 2, 3, 4]
case_3 = histogram_mode(input_3_0)
assert case_3.value == 1, 'case 3: value'
assert case_3.count == 1, 'case 3: count'
assert input_3_0 == [1, 2, 3, 4], 'case 3: input mutation'
input_4_0: list[int] = [4, 3, 2, 1]
case_4 = histogram_mode(input_4_0)
assert case_4.value == 1, 'case 4: value'
assert case_4.count == 1, 'case 4: count'
assert input_4_0 == [4, 3, 2, 1], 'case 4: input mutation'
input_5_0: list[int] = [2, 2, 2]
case_5 = histogram_mode(input_5_0)
assert case_5.value == 2, 'case 5: value'
assert case_5.count == 3, 'case 5: count'
assert input_5_0 == [2, 2, 2], 'case 5: input mutation'
input_6_0: list[int] = [-3, -2, -1]
case_6 = histogram_mode(input_6_0)
assert case_6.value == -3, 'case 6: value'
assert case_6.count == 1, 'case 6: count'
assert input_6_0 == [-3, -2, -1], 'case 6: input mutation'
input_7_0: list[int] = [3, 1, 2, 1, 4]
case_7 = histogram_mode(input_7_0)
assert case_7.value == 1, 'case 7: value'
assert case_7.count == 2, 'case 7: count'
assert input_7_0 == [3, 1, 2, 1, 4], 'case 7: input mutation'
input_8_0: list[int] = [5, -1, 5, -1, 5]
case_8 = histogram_mode(input_8_0)
assert case_8.value == 5, 'case 8: value'
assert case_8.count == 3, 'case 8: count'
assert input_8_0 == [5, -1, 5, -1, 5], 'case 8: input mutation'
input_9_0: list[int] = [0, -1, 2, -3, 4, -5]
case_9 = histogram_mode(input_9_0)
assert case_9.value == -5, 'case 9: value'
assert case_9.count == 1, 'case 9: count'
assert input_9_0 == [0, -1, 2, -3, 4, -5], 'case 9: input mutation'
input_10_0: list[int] = [9, 3, 7, 1, 8, 2, 6, 4, 5]
case_10 = histogram_mode(input_10_0)
assert case_10.value == 1, 'case 10: value'
assert case_10.count == 1, 'case 10: count'
assert input_10_0 == [9, 3, 7, 1, 8, 2, 6, 4, 5], 'case 10: input mutation'
input_11_0: list[int] = [100, -100, 100, 0]
case_11 = histogram_mode(input_11_0)
assert case_11.value == 100, 'case 11: value'
assert case_11.count == 2, 'case 11: count'
assert input_11_0 == [100, -100, 100, 0], 'case 11: input mutation'
input_12_0: list[int] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
case_12 = histogram_mode(input_12_0)
assert case_12.value == 0, 'case 12: value'
assert case_12.count == 20, 'case 12: count'
assert input_12_0 == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'case 12: input mutation'
input_13_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
case_13 = histogram_mode(input_13_0)
assert case_13.value == 0, 'case 13: value'
assert case_13.count == 1, 'case 13: count'
assert input_13_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 13: input mutation'
input_14_0: list[int] = [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
case_14 = histogram_mode(input_14_0)
assert case_14.value == 0, 'case 14: value'
assert case_14.count == 1, 'case 14: count'
assert input_14_0 == [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0], 'case 14: input mutation'
input_15_0: list[int] = [1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000]
case_15 = histogram_mode(input_15_0)
assert case_15.value == -1000, 'case 15: value'
assert case_15.count == 50, 'case 15: count'
assert input_15_0 == [1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000, 1000, -1000], 'case 15: input mutation'
input_16_0: list[int] = [100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
case_16 = histogram_mode(input_16_0)
assert case_16.value == 1, 'case 16: value'
assert case_16.count == 1, 'case 16: count'
assert input_16_0 == [100, 99, 98, 97, 96, 95, 94, 93, 92, 91, 90, 89, 88, 87, 86, 85, 84, 83, 82, 81, 80, 79, 78, 77, 76, 75, 74, 73, 72, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1], 'case 16: input mutation'
input_17_0: list[int] = [3, 3, 1, 1]
case_17 = histogram_mode(input_17_0)
assert case_17.value == 1, 'case 17: value'
assert case_17.count == 2, 'case 17: count'
assert input_17_0 == [3, 3, 1, 1], 'case 17: input mutation'
input_18_0: list[int] = [-2, -2, -3, -3]
case_18 = histogram_mode(input_18_0)
assert case_18.value == -3, 'case 18: value'
assert case_18.count == 2, 'case 18: count'
assert input_18_0 == [-2, -2, -3, -3], 'case 18: input mutation'
input_19_0: list[int] = [5, 1, 5, 1, 5]
case_19 = histogram_mode(input_19_0)
assert case_19.value == 5, 'case 19: value'
assert case_19.count == 3, 'case 19: count'
assert input_19_0 == [5, 1, 5, 1, 5], 'case 19: input mutation'
input_20_0: list[int] = [1000, -1000]
case_20 = histogram_mode(input_20_0)
assert case_20.value == -1000, 'case 20: value'
assert case_20.count == 1, 'case 20: count'
assert input_20_0 == [1000, -1000], 'case 20: input mutation'
input_21_0: list[int] = [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7]
case_21 = histogram_mode(input_21_0)
assert case_21.value == 7, 'case 21: value'
assert case_21.count == 100, 'case 21: count'
assert input_21_0 == [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], 'case 21: input mutation'
