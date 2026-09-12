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

def hex_digit(ch: str) -> int:
    pos: int = 0
    while (pos < 16):
        if (at("0123456789abcdef", pos) == lower(ch)):
            return pos
        pos = (pos + 1)
    return (-1)

def ascii_print(code: int) -> str:
    return at(" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~", (code - 32))

def solve(text: str) -> TextOutcome:
    if ((length(text) % 2) != 0):
        return TextRejected(TextError.MALFORMED)
    out: str = ""
    pos: int = 0
    while (pos < length(text)):
        high: int = hex_digit(at(text, pos))
        low: int = hex_digit(at(text, (pos + 1)))
        if ((high < 0) or (low < 0)):
            return TextRejected(TextError.MALFORMED)
        code: int = ((high * 16) + low)
        if ((code < 32) or (code > 126)):
            return TextRejected(TextError.RANGE)
        out = (out + ascii_print(code))
        pos = (pos + 2)
    return TextAccepted(out)
