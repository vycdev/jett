
export interface Entry {
    readonly node: bigint;
    readonly parent: bigint;
    readonly weight: bigint;
}

export interface Report {
    readonly node_count: bigint;
    readonly total_weight: bigint;
    readonly leaf_count: bigint;
}

export function belongs(rows: readonly Entry[], node: bigint, selected: bigint): boolean {
    let current: bigint = node;
    while ((current != 0n)) {
        if ((current == selected)) {
            return true;
        }
        let parent: bigint = 0n;
        for (const row of rows) {
            if ((row.node == current)) {
                parent = row.parent;
            }
        }
        current = parent;
    }
    return false;
}

export function is_leaf(rows: readonly Entry[], node: bigint): boolean {
    for (const row of rows) {
        if ((row.parent == node)) {
            return false;
        }
    }
    return true;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let count: bigint = 0n;
    let total: bigint = 0n;
    let leaves: bigint = 0n;
    for (const row of rows) {
        if (belongs(rows, row.node, limit)) {
            count = (count + 1n);
            total = (total + row.weight);
            if (is_leaf(rows, row.node)) {
                leaves = (leaves + 1n);
            }
        }
    }
    return { node_count: count, total_weight: total, leaf_count: leaves };
}
