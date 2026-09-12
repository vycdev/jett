
export interface Entry {
    readonly source: bigint;
    readonly target: bigint;
}

export interface Report {
    readonly reachable: bigint;
    readonly unreachable: bigint;
    readonly reachable_edge_count: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    if ((limit > 0n)) {
        let updated_value_0: bigint = 1n;
        seen = seen.set(0n, updated_value_0);
    }
    let turn: bigint = 0n;
    while ((turn < limit)) {
        for (const row of rows) {
            if (((seen.get(row.source) ?? 0n) == 1n)) {
                let updated_value_1: bigint = 1n;
                seen = seen.set(row.target, updated_value_1);
            }
        }
        turn = (turn + 1n);
    }
    let reachable: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        reachable = (reachable + (seen.get(vertex) ?? 0n));
        vertex = (vertex + 1n);
    }
    let edges: bigint = 0n;
    for (const row of rows) {
        edges = (edges + (seen.get(row.source) ?? 0n));
    }
    return { reachable: reachable, unreachable: (limit - reachable), reachable_edge_count: edges };
}
