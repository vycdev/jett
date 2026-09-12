
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


def compact(text: str) -> str:
    out: str = ""
    pending: bool = False
    pos: int = 0
    while (pos < length(text)):
        ch: str = at(text, pos)
        if (ch == " "):
            pending = (length(out) > 0)
        else:
            if pending:
                out = (out + " ")
            out = (out + ch)
            pending = False
        pos = (pos + 1)
    return out

def solve(text: str) -> str:
    return compact(text)
