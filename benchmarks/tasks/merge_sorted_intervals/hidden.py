from solution import Interval, merge_sorted_intervals


assert merge_sorted_intervals([]) == []
assert merge_sorted_intervals([Interval(1, 3)]) == [Interval(1, 3)]
assert merge_sorted_intervals([Interval(1, 3), Interval(2, 6), Interval(8, 10)]) == [Interval(1, 6), Interval(8, 10)]
assert merge_sorted_intervals([Interval(1, 10), Interval(2, 3), Interval(10, 12)]) == [Interval(1, 12)]
assert merge_sorted_intervals([Interval(-5, -2), Interval(-2, 0), Interval(4, 4)]) == [Interval(-5, 0), Interval(4, 4)]
