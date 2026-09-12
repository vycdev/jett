
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


def solve(text: str, pattern: str) -> bool:
    ti: int = 0
    pi: int = 0
    star: int = (-1)
    retry: int = 0
    while (ti < length(text)):
        ch: str = at(pattern, pi)
        if ((pi < length(pattern)) and ((ch == "?") or (ch == at(text, ti)))):
            ti = (ti + 1)
            pi = (pi + 1)
        else:
            if (ch == "*"):
                star = pi
                retry = ti
                pi = (pi + 1)
            else:
                if (star < 0):
                    return False
                retry = (retry + 1)
                ti = retry
                pi = (star + 1)
    while (at(pattern, pi) == "*"):
        pi = (pi + 1)
    return (pi == length(pattern))
