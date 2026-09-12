
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

def alpha(ch: str) -> bool:
    return (contains("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ", ch) and (length(ch) == 1))

def alnum(ch: str) -> bool:
    return (alpha(ch) or (digit(ch) >= 0))

def solve(text: str) -> bool:
    clean: str = ""
    pos: int = 0
    while (pos < length(text)):
        ch: str = at(text, pos)
        if alnum(ch):
            clean = (clean + lower(ch))
        pos = (pos + 1)
    pos = 0
    while (pos < length(clean)):
        if (at(clean, pos) != at(clean, ((length(clean) - 1) - pos))):
            return False
        pos = (pos + 1)
    return True
