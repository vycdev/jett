from solution import knapsack_value

input_0_0: list[int] = []
input_0_1: list[int] = []
assert knapsack_value(input_0_0, input_0_1, 0) == 0, 'case 0'
assert input_0_0 == [], 'case 0: input mutation'
assert input_0_1 == [], 'case 0: input mutation'
input_1_0: list[int] = []
input_1_1: list[int] = []
assert knapsack_value(input_1_0, input_1_1, 8) == 0, 'case 1'
assert input_1_0 == [], 'case 1: input mutation'
assert input_1_1 == [], 'case 1: input mutation'
input_2_0: list[int] = [1]
input_2_1: list[int] = [7]
assert knapsack_value(input_2_0, input_2_1, 0) == 0, 'case 2'
assert input_2_0 == [1], 'case 2: input mutation'
assert input_2_1 == [7], 'case 2: input mutation'
input_3_0: list[int] = [5]
input_3_1: list[int] = [10]
assert knapsack_value(input_3_0, input_3_1, 4) == 0, 'case 3'
assert input_3_0 == [5], 'case 3: input mutation'
assert input_3_1 == [10], 'case 3: input mutation'
input_4_0: list[int] = [5]
input_4_1: list[int] = [10]
assert knapsack_value(input_4_0, input_4_1, 5) == 10, 'case 4'
assert input_4_0 == [5], 'case 4: input mutation'
assert input_4_1 == [10], 'case 4: input mutation'
input_5_0: list[int] = [2, 3, 4]
input_5_1: list[int] = [4, 5, 7]
assert knapsack_value(input_5_0, input_5_1, 5) == 9, 'case 5'
assert input_5_0 == [2, 3, 4], 'case 5: input mutation'
assert input_5_1 == [4, 5, 7], 'case 5: input mutation'
input_6_0: list[int] = [2, 2, 2]
input_6_1: list[int] = [3, 3, 3]
assert knapsack_value(input_6_0, input_6_1, 4) == 6, 'case 6'
assert input_6_0 == [2, 2, 2], 'case 6: input mutation'
assert input_6_1 == [3, 3, 3], 'case 6: input mutation'
input_7_0: list[int] = [1, 1, 1]
input_7_1: list[int] = [0, 5, 7]
assert knapsack_value(input_7_0, input_7_1, 2) == 12, 'case 7'
assert input_7_0 == [1, 1, 1], 'case 7: input mutation'
assert input_7_1 == [0, 5, 7], 'case 7: input mutation'
input_8_0: list[int] = [6, 3, 4, 2]
input_8_1: list[int] = [30, 14, 16, 9]
assert knapsack_value(input_8_0, input_8_1, 10) == 46, 'case 8'
assert input_8_0 == [6, 3, 4, 2], 'case 8: input mutation'
assert input_8_1 == [30, 14, 16, 9], 'case 8: input mutation'
input_9_0: list[int] = [10, 20, 30]
input_9_1: list[int] = [60, 100, 100]
assert knapsack_value(input_9_0, input_9_1, 40) == 160, 'case 9'
assert input_9_0 == [10, 20, 30], 'case 9: input mutation'
assert input_9_1 == [60, 100, 100], 'case 9: input mutation'
input_10_0: list[int] = [1, 3, 4]
input_10_1: list[int] = [1, 4, 5]
assert knapsack_value(input_10_0, input_10_1, 7) == 9, 'case 10'
assert input_10_0 == [1, 3, 4], 'case 10: input mutation'
assert input_10_1 == [1, 4, 5], 'case 10: input mutation'
input_11_0: list[int] = [40]
input_11_1: list[int] = [100]
assert knapsack_value(input_11_0, input_11_1, 40) == 100, 'case 11'
assert input_11_0 == [40], 'case 11: input mutation'
assert input_11_1 == [100], 'case 11: input mutation'
input_12_0: list[int] = [7, 6, 5, 4, 3, 2]
input_12_1: list[int] = [5, 6, 7, 8, 9, 10]
assert knapsack_value(input_12_0, input_12_1, 12) == 27, 'case 12'
assert input_12_0 == [7, 6, 5, 4, 3, 2], 'case 12: input mutation'
assert input_12_1 == [5, 6, 7, 8, 9, 10], 'case 12: input mutation'
