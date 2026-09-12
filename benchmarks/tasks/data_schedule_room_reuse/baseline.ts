
export interface Entry {
    readonly start: bigint;
    readonly end: bigint;
}

export interface Report {
    readonly rooms: bigint;
    readonly assignment_checksum: bigint;
    readonly reuses: bigint;
}

export function solve(rows: readonly Entry[]): Report {
    let ends: Map<bigint, bigint> = new Map<bigint, bigint>();
    let rooms: bigint = 0n;
    let checksum: bigint = 0n;
    let reuses: bigint = 0n;
    let index: bigint = 1n;
    for (const row of rows) {
        let selected: bigint = 0n;
        let room: bigint = 1n;
        while ((room <= rooms)) {
            if (((selected == 0n) && ((ends.get(room) ?? 0n) <= row.start))) {
                selected = room;
            }
            room = (room + 1n);
        }
        if ((selected == 0n)) {
            rooms = (rooms + 1n);
            selected = rooms;
        }
        else {
            reuses = (reuses + 1n);
        }
        let updated_value_0: bigint = row.end;
        ends = ends.set(selected, updated_value_0);
        checksum = (checksum + (index * selected));
        index = (index + 1n);
    }
    return { rooms: rooms, assignment_checksum: checksum, reuses: reuses };
}
