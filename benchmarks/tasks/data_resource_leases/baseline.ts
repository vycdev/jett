
export interface Entry {
    readonly resource_id: bigint;
    readonly timestamp: bigint;
    readonly duration: bigint;
}

export interface Report {
    readonly accepted: bigint;
    readonly rejected: bigint;
    readonly expires_sum: bigint;
}

export function solve(rows: readonly Entry[]): Report {
    let expiries: Map<bigint, bigint> = new Map<bigint, bigint>();
    let accepted: bigint = 0n;
    let rejected: bigint = 0n;
    let total: bigint = 0n;
    for (const row of rows) {
        let prior: bigint = (expiries.get(row.resource_id) ?? 0n);
        if (((row.timestamp >= prior) && (row.duration > 0n))) {
            let next_expiry: bigint = (row.timestamp + row.duration);
            total = ((total + next_expiry) - prior);
            let updated_value_0: bigint = next_expiry;
            expiries = expiries.set(row.resource_id, updated_value_0);
            accepted = (accepted + 1n);
        }
        else {
            rejected = (rejected + 1n);
        }
    }
    return { accepted: accepted, rejected: rejected, expires_sum: total };
}
