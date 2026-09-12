
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


def closing(ch: str) -> str:
    if (ch == ")"):
        return "("
    if (ch == "]"):
        return "["
    return "{"

def solve(text: str) -> bool:
    stack: str = ""
    pos: int = 0
    while (pos < length(text)):
        ch: str = at(text, pos)
        if contains("([{", ch):
            stack = (stack + ch)
        else:
            if contains(")]}", ch):
                if (length(stack) == 0):
                    return False
                if (at(stack, (length(stack) - 1)) != closing(ch)):
                    return False
                stack = part(stack, 0, (length(stack) - 1))
        pos = (pos + 1)
    return (stack == "")
