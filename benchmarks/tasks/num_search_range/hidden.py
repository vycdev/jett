from solution import search_range

input_0_0: list[int] = []
case_0 = search_range(input_0_0, 0)
assert case_0.first == -1, 'case 0: first'
assert case_0.last == -1, 'case 0: last'
assert input_0_0 == [], 'case 0: input mutation'
input_1_0: list[int] = [1]
case_1 = search_range(input_1_0, 1)
assert case_1.first == 0, 'case 1: first'
assert case_1.last == 0, 'case 1: last'
assert input_1_0 == [1], 'case 1: input mutation'
input_2_0: list[int] = [1]
case_2 = search_range(input_2_0, 0)
assert case_2.first == -1, 'case 2: first'
assert case_2.last == -1, 'case 2: last'
assert input_2_0 == [1], 'case 2: input mutation'
input_3_0: list[int] = [1]
case_3 = search_range(input_3_0, 2)
assert case_3.first == -1, 'case 3: first'
assert case_3.last == -1, 'case 3: last'
assert input_3_0 == [1], 'case 3: input mutation'
input_4_0: list[int] = [1, 2, 2, 2, 3]
case_4 = search_range(input_4_0, 2)
assert case_4.first == 1, 'case 4: first'
assert case_4.last == 3, 'case 4: last'
assert input_4_0 == [1, 2, 2, 2, 3], 'case 4: input mutation'
input_5_0: list[int] = [1, 1, 2]
case_5 = search_range(input_5_0, 1)
assert case_5.first == 0, 'case 5: first'
assert case_5.last == 1, 'case 5: last'
assert input_5_0 == [1, 1, 2], 'case 5: input mutation'
input_6_0: list[int] = [1, 2, 2]
case_6 = search_range(input_6_0, 2)
assert case_6.first == 1, 'case 6: first'
assert case_6.last == 2, 'case 6: last'
assert input_6_0 == [1, 2, 2], 'case 6: input mutation'
input_7_0: list[int] = [0, 0, 0]
case_7 = search_range(input_7_0, 0)
assert case_7.first == 0, 'case 7: first'
assert case_7.last == 2, 'case 7: last'
assert input_7_0 == [0, 0, 0], 'case 7: input mutation'
input_8_0: list[int] = [-5, -2, -2, 0, 3]
case_8 = search_range(input_8_0, -2)
assert case_8.first == 1, 'case 8: first'
assert case_8.last == 2, 'case 8: last'
assert input_8_0 == [-5, -2, -2, 0, 3], 'case 8: input mutation'
input_9_0: list[int] = [-5, -2, 0, 3]
case_9 = search_range(input_9_0, 1)
assert case_9.first == -1, 'case 9: first'
assert case_9.last == -1, 'case 9: last'
assert input_9_0 == [-5, -2, 0, 3], 'case 9: input mutation'
input_10_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
case_10 = search_range(input_10_0, 10)
assert case_10.first == 10, 'case 10: first'
assert case_10.last == 10, 'case 10: last'
assert input_10_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 10: input mutation'
input_11_0: list[int] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19]
case_11 = search_range(input_11_0, 20)
assert case_11.first == -1, 'case 11: first'
assert case_11.last == -1, 'case 11: last'
assert input_11_0 == [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 'case 11: input mutation'
input_12_0: list[int] = [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7]
case_12 = search_range(input_12_0, 7)
assert case_12.first == 0, 'case 12: first'
assert case_12.last == 99, 'case 12: last'
assert input_12_0 == [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], 'case 12: input mutation'
