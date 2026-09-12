from dataclasses import dataclass
from enum import Enum

def length(text: str) -> int:
    return len(text)
def at(text: str, pos: int) -> str:
    return text[pos] if 0 <= pos < len(text) else ""
def part(text: str, start: int, end: int) -> str:
    return text[start:end]
def contains(text: str, needle: str) -> bool:
    return needle in text
def lower(text: str) -> str:
    return text.lower()
def upper(text: str) -> str:
    return text.upper()
def render(value: int) -> str:
    return str(value)

class TextError(Enum):
    EMPTY = "empty"
    MALFORMED = "malformed"
    RANGE = "range"
@dataclass(frozen=True)
class TextAccepted:
    value: str
@dataclass(frozen=True)
class TextRejected:
    error: TextError
type TextOutcome = TextAccepted | TextRejected

def solve(text: str) -> TextOutcome:
    best: str = ""
    word: str = ""
    pos: int = 0
    while (pos <= length(text)):
        ch: str = at(text, pos)
        if ((ch == " ") or (pos == length(text))):
            if (length(word) > length(best)):
                best = word
            word = ""
        else:
            word = (word + ch)
        pos = (pos + 1)
    if (best == ""):
        return TextRejected(TextError.EMPTY)
    return TextAccepted(best)
