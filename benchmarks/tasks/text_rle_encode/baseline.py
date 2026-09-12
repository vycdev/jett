
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


def solve(text: str) -> str:
    out: str = ""
    pos: int = 0
    while (pos < length(text)):
        end: int = (pos + 1)
        while ((end < length(text)) and (at(text, end) == at(text, pos))):
            end = (end + 1)
        out = ((out + render((end - pos))) + at(text, pos))
        pos = end
    return out
