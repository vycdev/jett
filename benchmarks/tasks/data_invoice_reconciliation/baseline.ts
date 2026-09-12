
export interface Entry {
    readonly invoice: bigint;
    readonly amount: bigint;
}

export interface Report {
    readonly settled: bigint;
    readonly outstanding: bigint;
    readonly credit: bigint;
}

export function solve(rows: readonly Entry[]): Report {
    let sums: Map<bigint, bigint> = new Map<bigint, bigint>();
    for (const row of rows) {
        let updated_value_0: bigint = ((sums.get(row.invoice) ?? 0n) + row.amount);
        sums = sums.set(row.invoice, updated_value_0);
    }
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    let settled: bigint = 0n;
    let outstanding: bigint = 0n;
    let credit: bigint = 0n;
    for (const row of rows) {
        if (((seen.get(row.invoice) ?? 0n) == 0n)) {
            let updated_value_1: bigint = 1n;
            seen = seen.set(row.invoice, updated_value_1);
            let balance: bigint = (sums.get(row.invoice) ?? 0n);
            if ((balance == 0n)) {
                settled = (settled + 1n);
            }
            if ((balance > 0n)) {
                outstanding = (outstanding + balance);
            }
            if ((balance < 0n)) {
                credit = (credit - balance);
            }
        }
    }
    return { settled: settled, outstanding: outstanding, credit: credit };
}
