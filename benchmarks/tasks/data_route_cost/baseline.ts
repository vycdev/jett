
export interface Entry {
    readonly source: bigint;
    readonly target: bigint;
    readonly cost: bigint;
}

export interface Report {
    readonly reachable: bigint;
    readonly cost_sum: bigint;
    readonly most_expensive: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let costs: Map<bigint, bigint> = new Map<bigint, bigint>();
    let updated_value_0: bigint = 0n;
    costs = costs.set(0n, updated_value_0);
    let reachable: bigint = 0n;
    let total: bigint = 0n;
    let maximum: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        let best: bigint = (costs.get(vertex) ?? 1000000n);
        for (const row of rows) {
            if ((row.target == vertex)) {
                let candidate: bigint = ((costs.get(row.source) ?? 1000000n) + row.cost);
                if ((candidate < best)) {
                    best = candidate;
                }
            }
        }
        let updated_value_1: bigint = best;
        costs = costs.set(vertex, updated_value_1);
        if ((best < 1000000n)) {
            reachable = (reachable + 1n);
            total = (total + best);
            if ((best > maximum)) {
                maximum = best;
            }
        }
        vertex = (vertex + 1n);
    }
    return { reachable: reachable, cost_sum: total, most_expensive: maximum };
}
