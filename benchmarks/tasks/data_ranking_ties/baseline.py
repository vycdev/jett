from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    score: int
    penalty: int

@dataclass(frozen=True)
class Report:
    selected: int
    rank_sum: int
    tie_groups: int

def ahead(score: int, penalty: int, other_score: int, other_penalty: int) -> bool:
    return ((other_score > score) or ((other_score == score) and (other_penalty < penalty)))

def solve(rows: list[Entry], limit: int) -> Report:
    selected: int = 0
    rank_sum: int = 0
    groups: int = 0
    index: int = 0
    for row in rows:
        rank: int = 1
        ties: int = 0
        first: int = index
        other_index: int = 0
        for other in rows:
            if ahead(row.score, row.penalty, other.score, other.penalty):
                rank = (rank + 1)
            if ((row.score == other.score) and (row.penalty == other.penalty)):
                ties = (ties + 1)
                if (other_index < first):
                    first = other_index
            other_index = (other_index + 1)
        if (rank <= limit):
            selected = (selected + 1)
            rank_sum = (rank_sum + rank)
        if ((ties > 1) and (first == index)):
            groups = (groups + 1)
        index = (index + 1)
    return Report(selected, rank_sum, groups)
