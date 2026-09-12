from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    source: int
    target: int

@dataclass(frozen=True)
class Report:
    sources: int
    sinks: int
    balanced: int

def solve(rows: list[Entry], limit: int) -> Report:
    sources: int = 0
    sinks: int = 0
    balanced: int = 0
    vertex: int = 0
    while (vertex < limit):
        incoming: int = 0
        outgoing: int = 0
        for row in rows:
            if (row.source == vertex):
                outgoing = (outgoing + 1)
            if (row.target == vertex):
                incoming = (incoming + 1)
        if ((incoming == 0) and (outgoing > 0)):
            sources = (sources + 1)
        if ((outgoing == 0) and (incoming > 0)):
            sinks = (sinks + 1)
        if (incoming == outgoing):
            balanced = (balanced + 1)
        vertex = (vertex + 1)
    return Report(sources, sinks, balanced)
