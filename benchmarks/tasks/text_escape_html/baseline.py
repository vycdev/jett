
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


def escape_char(ch: str) -> str:
    if (ch == "&"):
        return "&amp;"
    if (ch == "<"):
        return "&lt;"
    if (ch == ">"):
        return "&gt;"
    if (ch == "\""):
        return "&quot;"
    if (ch == "'"):
        return "&#39;"
    return ch

def solve(text: str) -> str:
    out: str = ""
    pos: int = 0
    while (pos < length(text)):
        out = (out + escape_char(at(text, pos)))
        pos = (pos + 1)
    return out
