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
def new_words() -> list[str]:
    return []
def push(values: list[str], value: str) -> list[str]:
    return values + [value]

class TextError(Enum):
    EMPTY = "empty"
    MALFORMED = "malformed"
    RANGE = "range"
@dataclass(frozen=True)
class TextAccepted:
    value: list[str]
@dataclass(frozen=True)
class TextRejected:
    error: TextError
type TextOutcome = TextAccepted | TextRejected

def quoted_end(text: str, start: int) -> int:
    pos: int = (start + 1)
    while (pos < length(text)):
        if (at(text, pos) == "\""):
            if (at(text, (pos + 1)) == "\""):
                pos = (pos + 2)
            else:
                return (pos + 1)
        else:
            pos = (pos + 1)
    return (-1)

def field_end(text: str, start: int) -> int:
    if (at(text, start) == "\""):
        return quoted_end(text, start)
    pos: int = start
    while ((pos < length(text)) and (at(text, pos) != ",")):
        if (at(text, pos) == "\""):
            return (-1)
        pos = (pos + 1)
    return pos

def decoded_field(text: str, start: int, end: int) -> str:
    if (at(text, start) != "\""):
        return part(text, start, end)
    out: str = ""
    pos: int = (start + 1)
    while (pos < (end - 1)):
        ch: str = at(text, pos)
        out = (out + ch)
        if (ch == "\""):
            pos = (pos + 1)
        pos = (pos + 1)
    return out

def solve(text: str) -> TextOutcome:
    pos: int = 0
    fields: list[str] = new_words()
    while (pos <= length(text)):
        end: int = field_end(text, pos)
        if (end < 0):
            return TextRejected(TextError.MALFORMED)
        fields = push(fields, decoded_field(text, pos, end))
        if (end == length(text)):
            return TextAccepted(fields)
        if (at(text, end) != ","):
            return TextRejected(TextError.MALFORMED)
        pos = (end + 1)
    return TextRejected(TextError.MALFORMED)
