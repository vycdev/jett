
export interface Entry {
    readonly start: bigint;
    readonly end: bigint;
    readonly resource_id: bigint;
}

export interface Report {
    readonly conflict_pairs: bigint;
    readonly affected_bookings: bigint;
    readonly longest_overlap: bigint;
}

export function overlap(a_start: bigint, a_end: bigint, b_start: bigint, b_end: bigint): bigint {
    let start: bigint = a_start;
    let end: bigint = a_end;
    if ((b_start > start)) {
        start = b_start;
    }
    if ((b_end < end)) {
        end = b_end;
    }
    if ((end > start)) {
        return (end - start);
    }
    return 0n;
}

export function solve(rows: readonly Entry[]): Report {
    let pairs: bigint = 0n;
    let affected: bigint = 0n;
    let longest: bigint = 0n;
    let left_index: bigint = 0n;
    for (const left of rows) {
        let hit: boolean = false;
        let right_index: bigint = 0n;
        for (const right of rows) {
            let duration: bigint = 0n;
            if (((left_index != right_index) && (left.resource_id == right.resource_id))) {
                duration = overlap(left.start, left.end, right.start, right.end);
            }
            if ((duration > 0n)) {
                hit = true;
                if ((left_index < right_index)) {
                    pairs = (pairs + 1n);
                }
                if ((duration > longest)) {
                    longest = duration;
                }
            }
            right_index = (right_index + 1n);
        }
        if (hit) {
            affected = (affected + 1n);
        }
        left_index = (left_index + 1n);
    }
    return { conflict_pairs: pairs, affected_bookings: affected, longest_overlap: longest };
}
