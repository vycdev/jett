from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    voter: int
    choice: int
    weight: int

@dataclass(frozen=True)
class Report:
    yes_weight: int
    no_weight: int
    quorum_met: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def contribution(choice: int, weight: int, wanted: int) -> int:
    if ((choice == wanted) and (weight > 0)):
        return weight
    return 0

def solve(rows: list[Entry], limit: int) -> Report:
    yes_votes: dict[int, int] = {}
    no_votes: dict[int, int] = {}
    yes: int = 0
    no: int = 0
    for row in rows:
        next_yes: int = contribution(row.choice, row.weight, 1)
        next_no: int = contribution(row.choice, row.weight, 0)
        yes = ((yes + next_yes) - yes_votes.get(row.voter, 0))
        no = ((no + next_no) - no_votes.get(row.voter, 0))
        updated_value_0: int = next_yes
        yes_votes = put(yes_votes, row.voter, updated_value_0)
        updated_value_1: int = next_no
        no_votes = put(no_votes, row.voter, updated_value_1)
    met: int = 0
    if ((yes >= limit) and (yes > no)):
        met = 1
    return Report(yes, no, met)
