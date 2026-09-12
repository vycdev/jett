
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


def quote_mode(mode: int, ch: str) -> int:
    if (mode == 2):
        return 1
    if ((mode == 1) and (ch == "\\")):
        return 2
    if (ch == "\""):
        return (1 - mode)
    return mode

def solve(text: str) -> str:
    out: str = ""
    mode: int = 0
    comment: bool = False
    pos: int = 0
    while (pos < length(text)):
        ch: str = at(text, pos)
        if comment:
            if (ch == "\n"):
                comment = False
                out = (out + ch)
        else:
            if ((mode == 0) and (ch == "#")):
                comment = True
            else:
                out = (out + ch)
                mode = quote_mode(mode, ch)
        pos = (pos + 1)
    return out
