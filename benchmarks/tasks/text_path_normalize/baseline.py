
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


def parent_path(text: str) -> str:
    pos: int = (length(text) - 1)
    while (pos >= 0):
        if (at(text, pos) == "/"):
            return part(text, 0, pos)
        pos = (pos - 1)
    return ""

def add_segment(path: str, segment: str) -> str:
    if ((segment == "") or (segment == ".")):
        return path
    if (segment == ".."):
        return parent_path(path)
    return ((path + "/") + segment)

def solve(text: str) -> str:
    path: str = ""
    start: int = 1
    pos: int = 1
    while (pos <= length(text)):
        if ((pos == length(text)) or (at(text, pos) == "/")):
            path = add_segment(path, part(text, start, pos))
            start = (pos + 1)
        pos = (pos + 1)
    if (path == ""):
        return "/"
    return path
