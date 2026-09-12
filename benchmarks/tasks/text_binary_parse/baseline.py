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
    value: int
@dataclass(frozen=True)
class TextRejected:
    error: TextError
type TextOutcome = TextAccepted | TextRejected

def digit(ch: str) -> int:
    pos: int = 0
    while (pos < 10):
        if (at("0123456789", pos) == ch):
            return pos
        pos = (pos + 1)
    return (-1)

def solve(text: str) -> TextOutcome:
    if (length(text) == 0):
        return TextRejected(TextError.EMPTY)
    if (length(text) > 16):
        return TextRejected(TextError.RANGE)
    value: int = 0
    pos: int = 0
    while (pos < length(text)):
        num: int = digit(at(text, pos))
        if ((num < 0) or (num > 1)):
            return TextRejected(TextError.MALFORMED)
        value = ((value * 2) + num)
        pos = (pos + 1)
    return TextAccepted(value)
