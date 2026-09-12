from solution import window_peak

input_0_0: list[int] = []
assert window_peak(input_0_0, 0) == None, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = []
assert window_peak(input_1_0, 1) == None, 'case 1'
assert input_1_0 == [], 'case 1: input mutation'
input_2_0: list[int] = [7]
assert window_peak(input_2_0, 0) == None, 'case 2'
assert input_2_0 == [7], 'case 2: input mutation'
input_3_0: list[int] = [7]
assert window_peak(input_3_0, 1) == 7, 'case 3'
assert input_3_0 == [7], 'case 3: input mutation'
input_4_0: list[int] = [7]
assert window_peak(input_4_0, 2) == None, 'case 4'
assert input_4_0 == [7], 'case 4: input mutation'
input_5_0: list[int] = [1, 2, 3, 4]
assert window_peak(input_5_0, 2) == 7, 'case 5'
assert input_5_0 == [1, 2, 3, 4], 'case 5: input mutation'
input_6_0: list[int] = [-5, -2, -7]
assert window_peak(input_6_0, 2) == -7, 'case 6'
assert input_6_0 == [-5, -2, -7], 'case 6: input mutation'
input_7_0: list[int] = [2, -1, 2, -1, 2]
assert window_peak(input_7_0, 3) == 3, 'case 7'
assert input_7_0 == [2, -1, 2, -1, 2], 'case 7: input mutation'
input_8_0: list[int] = [5, -9, 5]
assert window_peak(input_8_0, 1) == 5, 'case 8'
assert input_8_0 == [5, -9, 5], 'case 8: input mutation'
input_9_0: list[int] = [5, -9, 5]
assert window_peak(input_9_0, 3) == 1, 'case 9'
assert input_9_0 == [5, -9, 5], 'case 9: input mutation'
input_10_0: list[int] = [0, 0, 0]
assert window_peak(input_10_0, 2) == 0, 'case 10'
assert input_10_0 == [0, 0, 0], 'case 10: input mutation'
input_11_0: list[int] = [10, -5, -5, 10]
assert window_peak(input_11_0, 2) == 5, 'case 11'
assert input_11_0 == [10, -5, -5, 10], 'case 11: input mutation'
input_12_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
assert window_peak(input_12_0, 5) == 85, 'case 12'
assert input_12_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 12: input mutation'
input_13_0: list[int] = [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0]
assert window_peak(input_13_0, 7) == 119, 'case 13'
assert input_13_0 == [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0], 'case 13: input mutation'
