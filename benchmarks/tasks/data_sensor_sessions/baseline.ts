
export interface Entry {
    readonly sensor: bigint;
    readonly operation: bigint;
    readonly timestamp: bigint;
}

export interface Report {
    readonly completed: bigint;
    readonly active: bigint;
    readonly rejected: bigint;
}

export function step(operation: bigint, timestamp: bigint, opened: bigint): bigint {
    if ((timestamp < 0n)) {
        return (-2n);
    }
    if (((operation == 0n) && (opened == (-1n)))) {
        return timestamp;
    }
    if (((operation == 1n) && (opened >= 0n) && (timestamp >= opened))) {
        return (-1n);
    }
    return (-2n);
}

export function solve(rows: readonly Entry[]): Report {
    let opened: Map<bigint, bigint> = new Map<bigint, bigint>();
    let completed: bigint = 0n;
    let active: bigint = 0n;
    let rejected: bigint = 0n;
    for (const row of rows) {
        let prior: bigint = (opened.get(row.sensor) ?? (-1n));
        let next_time: bigint = step(row.operation, row.timestamp, prior);
        if ((next_time == (-2n))) {
            rejected = (rejected + 1n);
        }
        else {
            let updated_value_0: bigint = next_time;
            opened = opened.set(row.sensor, updated_value_0);
            if ((next_time == (-1n))) {
                completed = (completed + 1n);
                active = (active - 1n);
            }
            else {
                active = (active + 1n);
            }
        }
    }
    return { completed: completed, active: active, rejected: rejected };
}
