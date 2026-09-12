
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


def digit(ch: str) -> int:
    pos: int = 0
    while (pos < 10):
        if (at("0123456789", pos) == ch):
            return pos
        pos = (pos + 1)
    return (-1)

def uint_value(text: str, bound: int) -> int:
    if (length(text) == 0):
        return (-1)
    value: int = 0
    pos: int = 0
    while (pos < length(text)):
        num: int = digit(at(text, pos))
        if (num < 0):
            return (-1)
        value = ((value * 10) + num)
        if (value > bound):
            return (-1)
        pos = (pos + 1)
    return value

def component_end(text: str, start: int) -> int:
    pos: int = start
    while (pos < length(text)):
        if (at(text, pos) == "."):
            return pos
        pos = (pos + 1)
    return pos

def component_value(text: str, start: int, end: int) -> int:
    if (start >= length(text)):
        return 0
    return uint_value(part(text, start, end), 999)

def solve(left: str, right: str) -> int:
    lp: int = 0
    rp: int = 0
    while ((lp < length(left)) or (rp < length(right))):
        le: int = component_end(left, lp)
        re: int = component_end(right, rp)
        lv: int = component_value(left, lp, le)
        rv: int = component_value(right, rp, re)
        if (lv < rv):
            return (-1)
        if (lv > rv):
            return 1
        lp = (le + 1)
        rp = (re + 1)
    return 0
