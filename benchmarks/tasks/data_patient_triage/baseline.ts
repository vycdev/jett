
export interface Entry {
    readonly patient: bigint;
    readonly severity: bigint;
    readonly arrival: bigint;
}

export interface Report {
    readonly selected: bigint;
    readonly patient_checksum: bigint;
    readonly severity_sum: bigint;
}

export function precedes(severity: bigint, arrival: bigint, index: bigint, other_severity: bigint, other_arrival: bigint, other_index: bigint): boolean {
    if ((other_severity != severity)) {
        return (other_severity > severity);
    }
    if ((other_arrival != arrival)) {
        return (other_arrival < arrival);
    }
    return (other_index < index);
}

export function rank_of(rows: readonly Entry[], severity: bigint, arrival: bigint, index: bigint): bigint {
    let rank: bigint = 1n;
    let other_index: bigint = 0n;
    for (const other of rows) {
        if (precedes(severity, arrival, index, other.severity, other.arrival, other_index)) {
            rank = (rank + 1n);
        }
        other_index = (other_index + 1n);
    }
    return rank;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let selected: bigint = 0n;
    let checksum: bigint = 0n;
    let total: bigint = 0n;
    let index: bigint = 0n;
    for (const row of rows) {
        let rank: bigint = rank_of(rows, row.severity, row.arrival, index);
        if ((rank <= limit)) {
            selected = (selected + 1n);
            checksum = (checksum + (rank * (row.patient + 1n)));
            total = (total + row.severity);
        }
        index = (index + 1n);
    }
    return { selected: selected, patient_checksum: checksum, severity_sum: total };
}
