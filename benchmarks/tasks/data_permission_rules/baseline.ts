
export interface Entry {
    readonly principal: bigint;
    readonly allowed: bigint;
    readonly specificity: bigint;
}

export interface Report {
    readonly allowed: bigint;
    readonly denied: bigint;
    readonly defaulted: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let allowed: bigint = 0n;
    let denied: bigint = 0n;
    let defaulted: bigint = 0n;
    let principal: bigint = 0n;
    while ((principal < limit)) {
        let priority: bigint = (-1n);
        let decision: bigint = 0n;
        for (const row of rows) {
            if (((row.principal == principal) && (row.specificity >= priority))) {
                priority = row.specificity;
                decision = row.allowed;
            }
        }
        if ((priority == (-1n))) {
            defaulted = (defaulted + 1n);
        }
        allowed = (allowed + decision);
        denied = ((denied + 1n) - decision);
        principal = (principal + 1n);
    }
    return { allowed: allowed, denied: denied, defaulted: defaulted };
}
