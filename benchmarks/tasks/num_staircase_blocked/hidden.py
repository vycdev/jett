from solution import staircase_blocked

input_0_1: list[int] = []
assert staircase_blocked(0, input_0_1) == 1, 'case 0'
assert input_0_1 == [], 'case 0: input mutation'
input_1_1: list[int] = []
assert staircase_blocked(1, input_1_1) == 1, 'case 1'
assert input_1_1 == [], 'case 1: input mutation'
input_2_1: list[int] = [1]
assert staircase_blocked(1, input_2_1) == 0, 'case 2'
assert input_2_1 == [1], 'case 2: input mutation'
input_3_1: list[int] = [1]
assert staircase_blocked(2, input_3_1) == 1, 'case 3'
assert input_3_1 == [1], 'case 3: input mutation'
input_4_1: list[int] = [2]
assert staircase_blocked(2, input_4_1) == 0, 'case 4'
assert input_4_1 == [2], 'case 4: input mutation'
input_5_1: list[int] = []
assert staircase_blocked(3, input_5_1) == 3, 'case 5'
assert input_5_1 == [], 'case 5: input mutation'
input_6_1: list[int] = [2]
assert staircase_blocked(4, input_6_1) == 1, 'case 6'
assert input_6_1 == [2], 'case 6: input mutation'
input_7_1: list[int] = [2, 3]
assert staircase_blocked(5, input_7_1) == 0, 'case 7'
assert input_7_1 == [2, 3], 'case 7: input mutation'
input_8_1: list[int] = [5, 1]
assert staircase_blocked(6, input_8_1) == 2, 'case 8'
assert input_8_1 == [5, 1], 'case 8: input mutation'
input_9_1: list[int] = [4, 4, 7]
assert staircase_blocked(10, input_9_1) == 6, 'case 9'
assert input_9_1 == [4, 4, 7], 'case 9: input mutation'
input_10_1: list[int] = []
assert staircase_blocked(20, input_10_1) == 10946, 'case 10'
assert input_10_1 == [], 'case 10: input mutation'
input_11_1: list[int] = []
assert staircase_blocked(40, input_11_1) == 165580141, 'case 11'
assert input_11_1 == [], 'case 11: input mutation'
input_12_1: list[int] = [39]
assert staircase_blocked(40, input_12_1) == 63245986, 'case 12'
assert input_12_1 == [39], 'case 12: input mutation'
input_13_1: list[int] = [1, 2]
assert staircase_blocked(8, input_13_1) == 0, 'case 13'
assert input_13_1 == [1, 2], 'case 13: input mutation'
