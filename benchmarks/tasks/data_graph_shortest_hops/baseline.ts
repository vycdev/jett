
export interface Entry {
    readonly source: bigint;
    readonly target: bigint;
}

export interface Report {
    readonly distance_sum: bigint;
    readonly farthest: bigint;
    readonly unreachable: bigint;
}

export function relax(rows: readonly Entry[], distance: Map<bigint, bigint>): Map<bigint, bigint> {
    let updated: Map<bigint, bigint> = new Map<bigint, bigint>();
    for (const row of rows) {
        let prior: bigint = (distance.get(row.source) ?? 1000n);
        let old: bigint = (distance.get(row.target) ?? 1000n);
        let best: bigint = (updated.get(row.target) ?? old);
        if (((prior + 1n) < best)) {
            let updated_value_0: bigint = (prior + 1n);
            updated = updated.set(row.target, updated_value_0);
        }
    }
    return updated;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let distance: Map<bigint, bigint> = new Map<bigint, bigint>();
    let updated_value_1: bigint = 0n;
    distance = distance.set(0n, updated_value_1);
    let turn: bigint = 0n;
    while ((turn < limit)) {
        let updated: Map<bigint, bigint> = relax(rows, distance);
        let vertex: bigint = 0n;
        while ((vertex < limit)) {
            let updated_value_2: bigint = (updated.get(vertex) ?? (distance.get(vertex) ?? 1000n));
            distance = distance.set(vertex, updated_value_2);
            vertex = (vertex + 1n);
        }
        turn = (turn + 1n);
    }
    let total: bigint = 0n;
    let farthest: bigint = 0n;
    let missing: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        let hops: bigint = (distance.get(vertex) ?? 1000n);
        if ((hops == 1000n)) {
            missing = (missing + 1n);
        }
        else {
            total = (total + hops);
            if ((hops > farthest)) {
                farthest = hops;
            }
        }
        vertex = (vertex + 1n);
    }
    return { distance_sum: total, farthest: farthest, unreachable: missing };
}
