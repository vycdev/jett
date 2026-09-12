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

def signed_digits(negative: bool, digits: str) -> TextOutcome:
    if (digits == ""):
        return TextAccepted("0")
    if negative:
        return TextAccepted(("-" + digits))
    return TextAccepted(digits)

def solve(text: str) -> TextOutcome:
    if (length(text) == 0):
        return TextRejected(TextError.EMPTY)
    pos: int = 0
    negative: bool = (at(text, 0) == "-")
    if (negative or (at(text, 0) == "+")):
        pos = 1
    if (pos == length(text)):
        return TextRejected(TextError.MALFORMED)
    out: str = ""
    while (pos < length(text)):
        ch: str = at(text, pos)
        if (digit(ch) < 0):
            return TextRejected(TextError.MALFORMED)
        if ((out != "") or (ch != "0")):
            out = (out + ch)
        pos = (pos + 1)
    return signed_digits(negative, out)
