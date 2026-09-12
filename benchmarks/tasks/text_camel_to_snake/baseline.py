
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


def boundary(text: str, pos: int) -> bool:
    if (pos == 0):
        return False
    ch: str = at(text, pos)
    if (not contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ", ch)):
        return False
    prev: str = at(text, (pos - 1))
    if contains("abcdefghijklmnopqrstuvwxyz0123456789", prev):
        return True
    return (contains("abcdefghijklmnopqrstuvwxyz", at(text, (pos + 1))) and ((pos + 1) < length(text)))

def solve(text: str) -> str:
    out: str = ""
    pos: int = 0
    while (pos < length(text)):
        if boundary(text, pos):
            out = (out + "_")
        out = (out + lower(at(text, pos)))
        pos = (pos + 1)
    return out
