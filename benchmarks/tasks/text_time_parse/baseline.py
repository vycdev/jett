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

def uint_value(text: str, bound: int) -> int:
    if (length(text) == 0):
        return (-1)
    value: int = 0
    pos: int = 0
    while (pos < length(text)):
        num: int = digit(at(text, pos))
        if (num < 0):
            return (-1)
        value = ((value * 10) + num)
        if (value > bound):
            return (-1)
        pos = (pos + 1)
    return value

def solve(text: str) -> TextOutcome:
    if (length(text) != 8):
        return TextRejected(TextError.MALFORMED)
    if ((at(text, 2) != ":") or (at(text, 5) != ":")):
        return TextRejected(TextError.MALFORMED)
    hour: int = uint_value(part(text, 0, 2), 99)
    minute: int = uint_value(part(text, 3, 5), 99)
    second: int = uint_value(part(text, 6, 8), 99)
    if ((hour < 0) or (minute < 0) or (second < 0)):
        return TextRejected(TextError.MALFORMED)
    if ((hour > 23) or (minute > 59) or (second > 59)):
        return TextRejected(TextError.RANGE)
    return TextAccepted((((hour * 3600) + (minute * 60)) + second))
