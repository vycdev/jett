
export interface Entry {
    readonly duration: bigint;
    readonly deadline: bigint;
    readonly penalty: bigint;
}

export interface Report {
    readonly late: bigint;
    readonly weighted_tardiness: bigint;
    readonly finish: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let late: bigint = 0n;
    let weighted: bigint = 0n;
    let finish: bigint = limit;
    for (const row of rows) {
        finish = (finish + row.duration);
        if ((finish > row.deadline)) {
            late = (late + 1n);
            weighted = (weighted + ((finish - row.deadline) * row.penalty));
        }
    }
    return { late: late, weighted_tardiness: weighted, finish: finish };
}
