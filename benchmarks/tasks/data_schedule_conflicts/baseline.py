from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    start: int
    end: int
    resource_id: int

@dataclass(frozen=True)
class Report:
    conflict_pairs: int
    affected_bookings: int
    longest_overlap: int

def overlap(a_start: int, a_end: int, b_start: int, b_end: int) -> int:
    start: int = a_start
    end: int = a_end
    if (b_start > start):
        start = b_start
    if (b_end < end):
        end = b_end
    if (end > start):
        return (end - start)
    return 0

def solve(rows: list[Entry]) -> Report:
    pairs: int = 0
    affected: int = 0
    longest: int = 0
    left_index: int = 0
    for left in rows:
        hit: bool = False
        right_index: int = 0
        for right in rows:
            duration: int = 0
            if ((left_index != right_index) and (left.resource_id == right.resource_id)):
                duration = overlap(left.start, left.end, right.start, right.end)
            if (duration > 0):
                hit = True
                if (left_index < right_index):
                    pairs = (pairs + 1)
                if (duration > longest):
                    longest = duration
            right_index = (right_index + 1)
        if hit:
            affected = (affected + 1)
        left_index = (left_index + 1)
    return Report(pairs, affected, longest)
