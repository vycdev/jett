
export interface Entry {
    readonly stock: bigint;
    readonly daily_demand: bigint;
}

export interface Report {
    readonly restock_units: bigint;
    readonly at_risk: bigint;
    readonly worst_shortfall: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let units: bigint = 0n;
    let risk: bigint = 0n;
    let worst: bigint = 0n;
    for (const row of rows) {
        let shortage: bigint = ((row.daily_demand * limit) - row.stock);
        if ((shortage > 0n)) {
            units = (units + shortage);
            risk = (risk + 1n);
            if ((shortage > worst)) {
                worst = shortage;
            }
        }
    }
    return { restock_units: units, at_risk: risk, worst_shortfall: worst };
}
