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

def trim_space(text: str) -> str:
    start: int = 0
    end: int = length(text)
    while ((start < end) and (at(text, start) == " ")):
        start = (start + 1)
    while ((end > start) and (at(text, (end - 1)) == " ")):
        end = (end - 1)
    return part(text, start, end)

def solve(text: str) -> TextPair:
    pos: int = 0
    while (at(text, pos) != "="):
        pos = (pos + 1)
    return TextPair(trim_space(part(text, 0, pos)), trim_space(part(text, (pos + 1), length(text))))
