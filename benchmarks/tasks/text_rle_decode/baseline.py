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

def repeat_letter(ch: str, count: int) -> str:
    out: str = ""
    pos: int = 0
    while (pos < count):
        out = (out + ch)
        pos = (pos + 1)
    return out

def solve(text: str) -> TextOutcome:
    out: str = ""
    pos: int = 0
    while (pos < length(text)):
        start: int = pos
        while ((pos < length(text)) and (digit(at(text, pos)) >= 0)):
            pos = (pos + 1)
        count: int = uint_value(part(text, start, pos), 100)
        if ((count <= 0) or (at(text, start) == "0")):
            return TextRejected(TextError.MALFORMED)
        ch: str = at(text, pos)
        if ((pos == length(text)) or (not contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ", ch))):
            return TextRejected(TextError.MALFORMED)
        if ((length(out) + count) > 100):
            return TextRejected(TextError.RANGE)
        out = (out + repeat_letter(ch, count))
        pos = (pos + 1)
    return TextAccepted(out)
