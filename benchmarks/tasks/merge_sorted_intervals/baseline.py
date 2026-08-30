from dataclasses import dataclass


@dataclass(frozen=True)
class Interval:
    start: int
    end: int


def merge_sorted_intervals(intervals: list[Interval]) -> list[Interval]:
    merged: list[Interval] = []
    for interval in intervals:
        if not merged or interval.start > merged[-1].end:
            merged.append(Interval(interval.start, interval.end))
        elif interval.end > merged[-1].end:
            current = merged[-1]
            merged[-1] = Interval(current.start, interval.end)
    return merged
