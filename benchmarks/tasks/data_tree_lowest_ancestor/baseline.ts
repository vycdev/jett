
export interface Entry {
    readonly node: bigint;
    readonly parent: bigint;
}

export interface Report {
    readonly ancestor: bigint;
    readonly selected_distance: bigint;
    readonly largest_distance: bigint;
}

export function parent_of(rows: readonly Entry[], node: bigint): bigint {
    for (const row of rows) {
        if ((row.node == node)) {
            return row.parent;
        }
    }
    return (-1n);
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let largest: bigint = 0n;
    for (const row of rows) {
        if ((row.node > largest)) {
            largest = row.node;
        }
    }
    if ((parent_of(rows, limit) == (-1n))) {
        return { ancestor: 0n, selected_distance: (-1n), largest_distance: (-1n) };
    }
    let selected: bigint = limit;
    let selected_distance: bigint = 0n;
    while ((selected > 0n)) {
        let other: bigint = largest;
        let other_distance: bigint = 0n;
        while ((other > 0n)) {
            if ((other == selected)) {
                return { ancestor: selected, selected_distance: selected_distance, largest_distance: other_distance };
            }
            other = parent_of(rows, other);
            other_distance = (other_distance + 1n);
        }
        selected = parent_of(rows, selected);
        selected_distance = (selected_distance + 1n);
    }
    return { ancestor: 0n, selected_distance: (-1n), largest_distance: (-1n) };
}
