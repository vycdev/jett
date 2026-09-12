
export interface Entry {
    readonly event: bigint;
    readonly key: bigint;
    readonly delta: bigint;
}

export interface Report {
    readonly applied: bigint;
    readonly duplicates: bigint;
    readonly checksum: bigint;
}

export function solve(rows: readonly Entry[]): Report {
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    let applied: bigint = 0n;
    let duplicates: bigint = 0n;
    let checksum: bigint = 0n;
    for (const row of rows) {
        if (((seen.get(row.event) ?? 0n) == 0n)) {
            let updated_value_0: bigint = 1n;
            seen = seen.set(row.event, updated_value_0);
            checksum = (checksum + ((row.key + 1n) * row.delta));
            applied = (applied + 1n);
        }
        else {
            duplicates = (duplicates + 1n);
        }
    }
    return { applied: applied, duplicates: duplicates, checksum: checksum };
}
