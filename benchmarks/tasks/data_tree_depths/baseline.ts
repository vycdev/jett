
export interface Entry {
    readonly node: bigint;
    readonly parent: bigint;
    readonly weight: bigint;
}

export interface Report {
    readonly roots: bigint;
    readonly max_depth: bigint;
    readonly weighted_depth: bigint;
}

export function depth_of(rows: readonly Entry[], node: bigint): bigint {
    let current: bigint = node;
    let depth: bigint = (-1n);
    while ((current != 0n)) {
        let parent: bigint = 0n;
        for (const row of rows) {
            if ((row.node == current)) {
                parent = row.parent;
            }
        }
        current = parent;
        depth = (depth + 1n);
    }
    return depth;
}

export function solve(rows: readonly Entry[]): Report {
    let roots: bigint = 0n;
    let maximum: bigint = 0n;
    let total: bigint = 0n;
    for (const row of rows) {
        let depth: bigint = depth_of(rows, row.node);
        if ((depth == 0n)) {
            roots = (roots + 1n);
        }
        if ((depth > maximum)) {
            maximum = depth;
        }
        total = (total + (row.weight * depth));
    }
    return { roots: roots, max_depth: maximum, weighted_depth: total };
}
