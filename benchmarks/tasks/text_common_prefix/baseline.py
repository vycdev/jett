
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


def solve(left: str, right: str) -> str:
    pos: int = 0
    while ((pos < length(left)) and (pos < length(right))):
        if (at(left, pos) != at(right, pos)):
            return part(left, 0, pos)
        pos = (pos + 1)
    return part(left, 0, pos)
