
export interface Entry {
    readonly prerequisite: bigint;
    readonly dependent: bigint;
}

export interface Report {
    readonly layers: bigint;
    readonly last_layer_count: bigint;
    readonly layer_sum: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let levels: Map<bigint, bigint> = new Map<bigint, bigint>();
    let maximum: bigint = (-1n);
    let last_count: bigint = 0n;
    let total: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        let level: bigint = 0n;
        for (const row of rows) {
            if ((row.dependent == vertex)) {
                let prior: bigint = ((levels.get(row.prerequisite) ?? 0n) + 1n);
                if ((prior > level)) {
                    level = prior;
                }
            }
        }
        let updated_value_0: bigint = level;
        levels = levels.set(vertex, updated_value_0);
        total = (total + level);
        if ((level > maximum)) {
            maximum = level;
            last_count = 0n;
        }
        if ((level == maximum)) {
            last_count = (last_count + 1n);
        }
        vertex = (vertex + 1n);
    }
    return { layers: (maximum + 1n), last_layer_count: last_count, layer_sum: total };
}
