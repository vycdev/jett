from dataclasses import dataclass
from enum import Enum


class ScoreError(Enum):
    MALFORMED = "malformed"
    INVALID_SCORE = "invalid_score"
    DUPLICATE_NAME = "duplicate_name"


@dataclass(frozen=True)
class ParsedScores:
    scores: dict[str, int]


@dataclass(frozen=True)
class ScoreFailure:
    line: int
    error: ScoreError


type ScoreResult = ParsedScores | ScoreFailure


def parse_scores(lines: list[str]) -> ScoreResult:
    scores: dict[str, int] = {}
    for line_index, line in enumerate(lines):
        parts = line.split("=")
        if len(parts) != 2 or parts[0] == "":
            return ScoreFailure(line_index, ScoreError.MALFORMED)
        name, raw_score = parts
        try:
            score = int(raw_score)
        except ValueError:
            return ScoreFailure(line_index, ScoreError.INVALID_SCORE)
        if score < -(2**63) or score > 2**63 - 1 or str(score) != raw_score:
            return ScoreFailure(line_index, ScoreError.INVALID_SCORE)
        if name in scores:
            return ScoreFailure(line_index, ScoreError.DUPLICATE_NAME)
        scores[name] = score
    return ParsedScores(scores)
