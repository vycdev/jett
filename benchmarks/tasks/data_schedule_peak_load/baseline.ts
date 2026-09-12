
export interface Entry {
    readonly start: bigint;
    readonly end: bigint;
    readonly demand: bigint;
}

export interface Report {
    readonly peak: bigint;
    readonly earliest: bigint;
    readonly overloaded_starts: bigint;
}

export function load_at(rows: readonly Entry[], moment: bigint): bigint {
    let total: bigint = 0n;
    for (const row of rows) {
        if (((row.start <= moment) && (row.end > moment))) {
            total = (total + row.demand);
        }
    }
    return total;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    let peak: bigint = 0n;
    let earliest: bigint = (-1n);
    let overloaded: bigint = 0n;
    for (const row of rows) {
        let load: bigint = load_at(rows, row.start);
        if (((load > peak) || ((load == peak) && (row.start < earliest)))) {
            peak = load;
            earliest = row.start;
        }
        if (((seen.get(row.start) ?? 0n) == 0n)) {
            if ((load > limit)) {
                overloaded = (overloaded + 1n);
            }
            let updated_value_0: bigint = 1n;
            seen = seen.set(row.start, updated_value_0);
        }
    }
    return { peak: peak, earliest: earliest, overloaded_starts: overloaded };
}
