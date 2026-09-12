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

def decode_escape(ch: str) -> str:
    if (ch == "n"):
        return "\n"
    if (ch == "t"):
        return "\t"
    if (ch == "r"):
        return "\r"
    return ch

def solve(text: str) -> TextOutcome:
    out: str = ""
    pos: int = 0
    while (pos < length(text)):
        ch: str = at(text, pos)
        if (ch == "\\"):
            pos = (pos + 1)
            if (pos == length(text)):
                return TextRejected(TextError.MALFORMED)
            ch = at(text, pos)
            if (not contains("ntr\\\"", ch)):
                return TextRejected(TextError.MALFORMED)
            ch = decode_escape(ch)
        out = (out + ch)
        pos = (pos + 1)
    return TextAccepted(out)
