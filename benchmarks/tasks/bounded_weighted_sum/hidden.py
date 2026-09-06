from solution import bounded_weighted_sum

assert bounded_weighted_sum([], 5) == 0
assert bounded_weighted_sum([1, 2, 3], 10) == 14
assert bounded_weighted_sum([20, -20, 3], 10) == -1
assert bounded_weighted_sum([-1, 2, -3, 4], 2) == 5
assert bounded_weighted_sum([9, -4], 0) == 0
