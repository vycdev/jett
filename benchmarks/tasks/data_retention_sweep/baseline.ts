
export interface Entry {
    readonly key: bigint;
    readonly modified: bigint;
    readonly size: bigint;
}

export interface Report {
    readonly removed: bigint;
    readonly reclaimed: bigint;
    readonly retained: bigint;
}

export function newest_index(rows: readonly Entry[], key: bigint): bigint {
    let best_index: bigint = (-1n);
    let best_time: bigint = (-1000000n);
    let index: bigint = 0n;
    for (const row of rows) {
        if (((row.key == key) && (row.modified >= best_time))) {
            best_index = index;
            best_time = row.modified;
        }
        index = (index + 1n);
    }
    return best_index;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let removed: bigint = 0n;
    let reclaimed: bigint = 0n;
    let retained: bigint = 0n;
    let index: bigint = 0n;
    for (const row of rows) {
        if (((row.modified < limit) && (index != newest_index(rows, row.key)))) {
            removed = (removed + 1n);
            reclaimed = (reclaimed + row.size);
        }
        else {
            retained = (retained + 1n);
        }
        index = (index + 1n);
    }
    return { removed: removed, reclaimed: reclaimed, retained: retained };
}
