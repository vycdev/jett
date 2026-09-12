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

def octet(text: str) -> int:
    if ((length(text) > 1) and (at(text, 0) == "0")):
        return (-1)
    return uint_value(text, 255)

def solve(text: str) -> TextOutcome:
    pos: int = 0
    start: int = 0
    count: int = 0
    value: int = 0
    while (pos <= length(text)):
        if ((pos == length(text)) or (at(text, pos) == ".")):
            num: int = octet(part(text, start, pos))
            if ((num < 0) or (count == 4)):
                return TextRejected(TextError.MALFORMED)
            value = ((value * 256) + num)
            count = (count + 1)
            start = (pos + 1)
        pos = (pos + 1)
    if (count != 4):
        return TextRejected(TextError.MALFORMED)
    return TextAccepted(value)
