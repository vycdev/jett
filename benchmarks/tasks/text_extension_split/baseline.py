from dataclasses import dataclass

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

@dataclass(frozen=True)
class TextPair:
    first: str
    second: str

def solve(text: str) -> TextPair:
    dot: int = (-1)
    pos: int = 1
    while (pos < length(text)):
        if (at(text, pos) == "."):
            dot = pos
        pos = (pos + 1)
    if (dot < 0):
        return TextPair(text, "")
    return TextPair(part(text, 0, dot), part(text, (dot + 1), length(text)))
