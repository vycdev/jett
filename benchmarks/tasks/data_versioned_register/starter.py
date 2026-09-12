from dataclasses import dataclass

@dataclass(frozen=True)
class Entry:
    key: int
    version: int
    value: int

@dataclass(frozen=True)
class Report:
    accepted: int
    stale: int
    checksum: int

def put(table: dict[int, int], key: int, value: int) -> dict[int, int]:
    table[key] = value
    return table

def solve(rows: list[Entry]) -> Report:
    versions: dict[int, int] = {}
    values: dict[int, int] = {}
    accepted: int = 0
    stale: int = 0
    checksum: int = 0
    for row in rows:
        if (row.version >= versions.get(row.key, (-1))):
            checksum = (checksum + ((row.key + 1) * (row.value - values.get(row.key, 0))))
            updated_value_0: int = row.value
            values = put(values, row.key, updated_value_0)
            updated_value_1: int = row.version
            versions = put(versions, row.key, updated_value_1)
            accepted = (accepted + 1)
        else:
            stale = (stale + 1)
    return Report(accepted, stale, checksum)
