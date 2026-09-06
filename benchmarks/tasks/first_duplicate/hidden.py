from solution import first_duplicate


assert first_duplicate([]) is None
assert first_duplicate([7]) is None
assert first_duplicate([2, 1, 3, 1, 2]) == 1
assert first_duplicate([4, 4, 5, 5]) == 4
assert first_duplicate([-2, 0, -2]) == -2
assert first_duplicate([0, 1, 0, 1]) == 0
