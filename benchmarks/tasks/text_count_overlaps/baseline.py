
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


def solve(text: str, needle: str) -> int:
    count: int = 0
    pos: int = 0
    while ((pos + length(needle)) <= length(text)):
        if (part(text, pos, (pos + length(needle))) == needle):
            count = (count + 1)
        pos = (pos + 1)
    return count
