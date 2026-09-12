
export interface Entry {
    readonly key: bigint;
    readonly side: bigint;
    readonly quantity: bigint;
}

export interface Report {
    readonly matched: bigint;
    readonly left_only: bigint;
    readonly right_only: bigint;
}

export function solve(rows: readonly Entry[]): Report {
    let left: Map<bigint, bigint> = new Map<bigint, bigint>();
    let right: Map<bigint, bigint> = new Map<bigint, bigint>();
    for (const row of rows) {
        if ((row.side == 0n)) {
            let updated_value_0: bigint = ((left.get(row.key) ?? 0n) + row.quantity);
            left = left.set(row.key, updated_value_0);
        }
        if ((row.side == 1n)) {
            let updated_value_1: bigint = ((right.get(row.key) ?? 0n) + row.quantity);
            right = right.set(row.key, updated_value_1);
        }
    }
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    let matched: bigint = 0n;
    let left_only: bigint = 0n;
    let right_only: bigint = 0n;
    for (const row of rows) {
        if (((seen.get(row.key) ?? 0n) == 0n)) {
            let updated_value_2: bigint = 1n;
            seen = seen.set(row.key, updated_value_2);
            let a: bigint = (left.get(row.key) ?? 0n);
            let b: bigint = (right.get(row.key) ?? 0n);
            let pairs: bigint = a;
            if ((b < pairs)) {
                pairs = b;
            }
            matched = (matched + pairs);
            left_only = ((left_only + a) - pairs);
            right_only = ((right_only + b) - pairs);
        }
    }
    return { matched: matched, left_only: left_only, right_only: right_only };
}
