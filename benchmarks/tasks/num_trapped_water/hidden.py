from solution import trapped_water

input_0_0: list[int] = []
assert trapped_water(input_0_0) == 0, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [0]
assert trapped_water(input_1_0) == 0, 'case 1'
assert input_1_0 == [0], 'case 1: input mutation'
input_2_0: list[int] = [5]
assert trapped_water(input_2_0) == 0, 'case 2'
assert input_2_0 == [5], 'case 2: input mutation'
input_3_0: list[int] = [5, 0]
assert trapped_water(input_3_0) == 0, 'case 3'
assert input_3_0 == [5, 0], 'case 3: input mutation'
input_4_0: list[int] = [0, 5]
assert trapped_water(input_4_0) == 0, 'case 4'
assert input_4_0 == [0, 5], 'case 4: input mutation'
input_5_0: list[int] = [3, 3, 3]
assert trapped_water(input_5_0) == 0, 'case 5'
assert input_5_0 == [3, 3, 3], 'case 5: input mutation'
input_6_0: list[int] = [3, 0, 3]
assert trapped_water(input_6_0) == 3, 'case 6'
assert input_6_0 == [3, 0, 3], 'case 6: input mutation'
input_7_0: list[int] = [5, 0, 2]
assert trapped_water(input_7_0) == 2, 'case 7'
assert input_7_0 == [5, 0, 2], 'case 7: input mutation'
input_8_0: list[int] = [2, 0, 5]
assert trapped_water(input_8_0) == 2, 'case 8'
assert input_8_0 == [2, 0, 5], 'case 8: input mutation'
input_9_0: list[int] = [3, 0, 2, 0, 4]
assert trapped_water(input_9_0) == 7, 'case 9'
assert input_9_0 == [3, 0, 2, 0, 4], 'case 9: input mutation'
input_10_0: list[int] = [0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1]
assert trapped_water(input_10_0) == 6, 'case 10'
assert input_10_0 == [0, 1, 0, 2, 1, 0, 1, 3, 2, 1, 2, 1], 'case 10: input mutation'
input_11_0: list[int] = [5, 4, 3, 2, 1]
assert trapped_water(input_11_0) == 0, 'case 11'
assert input_11_0 == [5, 4, 3, 2, 1], 'case 11: input mutation'
input_12_0: list[int] = [1, 2, 3, 4, 5]
assert trapped_water(input_12_0) == 0, 'case 12'
assert input_12_0 == [1, 2, 3, 4, 5], 'case 12: input mutation'
input_13_0: list[int] = [4, 2, 0, 3, 2, 5]
assert trapped_water(input_13_0) == 9, 'case 13'
assert input_13_0 == [4, 2, 0, 3, 2, 5], 'case 13: input mutation'
input_14_0: list[int] = [3, 0, 0, 3]
assert trapped_water(input_14_0) == 6, 'case 14'
assert input_14_0 == [3, 0, 0, 3], 'case 14: input mutation'
input_15_0: list[int] = [1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1000]
assert trapped_water(input_15_0) == 98000, 'case 15'
assert input_15_0 == [1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1000], 'case 15: input mutation'
input_16_0: list[int] = [1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0]
assert trapped_water(input_16_0) == 49000, 'case 16'
assert input_16_0 == [1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0, 1000, 0], 'case 16: input mutation'
input_17_0: list[int] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
assert trapped_water(input_17_0) == 0, 'case 17'
assert input_17_0 == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 'case 17: input mutation'
