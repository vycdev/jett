
export interface Entry {
    readonly key: bigint;
    readonly version: bigint;
    readonly value: bigint;
}

export interface Report {
    readonly accepted: bigint;
    readonly stale: bigint;
    readonly checksum: bigint;
}

export function solve(rows: readonly Entry[]): Report {
    let versions: Map<bigint, bigint> = new Map<bigint, bigint>();
    let values: Map<bigint, bigint> = new Map<bigint, bigint>();
    let accepted: bigint = 0n;
    let stale: bigint = 0n;
    let checksum: bigint = 0n;
    for (const row of rows) {
        if ((row.version > (versions.get(row.key) ?? (-1n)))) {
            checksum = (checksum + ((row.key + 1n) * (row.value - (values.get(row.key) ?? 0n))));
            let updated_value_0: bigint = row.value;
            values = values.set(row.key, updated_value_0);
            let updated_value_1: bigint = row.version;
            versions = versions.set(row.key, updated_value_1);
            accepted = (accepted + 1n);
        }
        else {
            stale = (stale + 1n);
        }
    }
    return { accepted: accepted, stale: stale, checksum: checksum };
}
