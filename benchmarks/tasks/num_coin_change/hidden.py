from solution import coin_change

input_0_0: list[int] = []
assert coin_change(input_0_0, 0) == 0, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = []
assert coin_change(input_1_0, 1) == -1, 'case 1'
assert input_1_0 == [], 'case 1: input mutation'
input_2_0: list[int] = [1]
assert coin_change(input_2_0, 0) == 0, 'case 2'
assert input_2_0 == [1], 'case 2: input mutation'
input_3_0: list[int] = [1]
assert coin_change(input_3_0, 7) == 7, 'case 3'
assert input_3_0 == [1], 'case 3: input mutation'
input_4_0: list[int] = [2]
assert coin_change(input_4_0, 3) == -1, 'case 4'
assert input_4_0 == [2], 'case 4: input mutation'
input_5_0: list[int] = [2]
assert coin_change(input_5_0, 8) == 4, 'case 5'
assert input_5_0 == [2], 'case 5: input mutation'
input_6_0: list[int] = [1, 3, 4]
assert coin_change(input_6_0, 6) == 2, 'case 6'
assert input_6_0 == [1, 3, 4], 'case 6: input mutation'
input_7_0: list[int] = [5, 2]
assert coin_change(input_7_0, 11) == 4, 'case 7'
assert input_7_0 == [5, 2], 'case 7: input mutation'
input_8_0: list[int] = [3, 7]
assert coin_change(input_8_0, 10) == 2, 'case 8'
assert input_8_0 == [3, 7], 'case 8: input mutation'
input_9_0: list[int] = [3, 7]
assert coin_change(input_9_0, 5) == -1, 'case 9'
assert input_9_0 == [3, 7], 'case 9: input mutation'
input_10_0: list[int] = [2, 2, 4]
assert coin_change(input_10_0, 8) == 2, 'case 10'
assert input_10_0 == [2, 2, 4], 'case 10: input mutation'
input_11_0: list[int] = [50]
assert coin_change(input_11_0, 100) == 2, 'case 11'
assert input_11_0 == [50], 'case 11: input mutation'
input_12_0: list[int] = [7, 10, 25]
assert coin_change(input_12_0, 99) == 6, 'case 12'
assert input_12_0 == [7, 10, 25], 'case 12: input mutation'
input_13_0: list[int] = [9, 6, 5, 1]
assert coin_change(input_13_0, 11) == 2, 'case 13'
assert input_13_0 == [9, 6, 5, 1], 'case 13: input mutation'
