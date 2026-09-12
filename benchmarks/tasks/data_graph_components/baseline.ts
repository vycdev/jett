
export interface Entry {
    readonly source: bigint;
    readonly target: bigint;
}

export interface Report {
    readonly components: bigint;
    readonly largest: bigint;
    readonly isolated: bigint;
}

export function spread(rows: readonly Entry[], seen: Map<bigint, bigint>, vertex: bigint): bigint {
    let found: bigint = (seen.get(vertex) ?? 0n);
    for (const row of rows) {
        if ((row.source == vertex)) {
            if (((seen.get(row.target) ?? 0n) == 1n)) {
                found = 1n;
            }
        }
        if ((row.target == vertex)) {
            if (((seen.get(row.source) ?? 0n) == 1n)) {
                found = 1n;
            }
        }
    }
    return found;
}

export function component_size(rows: readonly Entry[], start: bigint, limit: bigint): bigint {
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    let updated_value_0: bigint = 1n;
    seen = seen.set(start, updated_value_0);
    let turn: bigint = 0n;
    while ((turn < limit)) {
        let vertex: bigint = 0n;
        while ((vertex < limit)) {
            let updated_value_1: bigint = spread(rows, seen, vertex);
            seen = seen.set(vertex, updated_value_1);
            vertex = (vertex + 1n);
        }
        turn = (turn + 1n);
    }
    let count: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        if (((seen.get(vertex) ?? 0n) == 1n)) {
            if ((vertex < start)) {
                return 0n;
            }
            count = (count + 1n);
        }
        vertex = (vertex + 1n);
    }
    return count;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let components: bigint = 0n;
    let largest: bigint = 0n;
    let isolated: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        let count: bigint = component_size(rows, vertex, limit);
        if ((count > 0n)) {
            components = (components + 1n);
        }
        if ((count > largest)) {
            largest = count;
        }
        let incident: bigint = 0n;
        for (const row of rows) {
            if (((row.source == vertex) || (row.target == vertex))) {
                incident = (incident + 1n);
            }
        }
        if ((incident == 0n)) {
            isolated = (isolated + 1n);
        }
        vertex = (vertex + 1n);
    }
    return { components: components, largest: largest, isolated: isolated };
}
