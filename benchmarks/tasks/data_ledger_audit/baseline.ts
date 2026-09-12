
export interface Entry {
    readonly account: bigint;
    readonly delta: bigint;
}

export interface Report {
    readonly total: bigint;
    readonly lowest_balance: bigint;
    readonly first_overdraw: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let balances: Map<bigint, bigint> = new Map<bigint, bigint>();
    let seen: Map<bigint, bigint> = new Map<bigint, bigint>();
    let total: bigint = 0n;
    let lowest: bigint = limit;
    let first: bigint = (-1n);
    let index: bigint = 0n;
    for (const row of rows) {
        if (((seen.get(row.account) ?? 0n) == 0n)) {
            total = (total + limit);
            let updated_value_0: bigint = 1n;
            seen = seen.set(row.account, updated_value_0);
        }
        let balance: bigint = ((balances.get(row.account) ?? limit) + row.delta);
        let updated_value_1: bigint = balance;
        balances = balances.set(row.account, updated_value_1);
        total = (total + row.delta);
        if (((index == 0n) || (balance < lowest))) {
            lowest = balance;
        }
        if (((balance < 0n) && (first == (-1n)))) {
            first = index;
        }
        index = (index + 1n);
    }
    return { total: total, lowest_balance: lowest, first_overdraw: first };
}
