
export interface Entry {
    readonly job: bigint;
    readonly prerequisite: bigint;
    readonly cost: bigint;
}

export interface Report {
    readonly ready: bigint;
    readonly blocked: bigint;
    readonly total_cost: bigint;
}

export function ready_job(rows: readonly Entry[], job: bigint): boolean {
    let current: bigint = job;
    while ((current != 0n)) {
        let parent: bigint = (-1n);
        for (const row of rows) {
            if ((row.job == current)) {
                parent = row.prerequisite;
            }
        }
        if ((parent == (-1n))) {
            return false;
        }
        current = parent;
    }
    return true;
}

export function solve(rows: readonly Entry[]): Report {
    let ready: bigint = 0n;
    let blocked: bigint = 0n;
    let total: bigint = 0n;
    for (const row of rows) {
        if (ready_job(rows, row.job)) {
            ready = (ready + 1n);
            total = (total + row.cost);
        }
        else {
            blocked = (blocked + 1n);
        }
    }
    return { ready: ready, blocked: blocked, total_cost: total };
}
