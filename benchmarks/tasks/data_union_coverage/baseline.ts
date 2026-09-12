
export interface Entry {
    readonly start: bigint;
    readonly end: bigint;
}

export interface Report {
    readonly covered: bigint;
    readonly gaps: bigint;
    readonly longest_gap: bigint;
}

export function covered_at(rows: readonly Entry[], moment: bigint): boolean {
    for (const row of rows) {
        if (((row.start <= moment) && (row.end > moment))) {
            return true;
        }
    }
    return false;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let covered: bigint = 0n;
    let gaps: bigint = 0n;
    let longest: bigint = 0n;
    let current: bigint = 0n;
    let moment: bigint = 0n;
    while ((moment < limit)) {
        if (covered_at(rows, moment)) {
            covered = (covered + 1n);
            current = 0n;
        }
        else {
            if ((current == 0n)) {
                gaps = (gaps + 1n);
            }
            current = (current + 1n);
            if ((current > longest)) {
                longest = current;
            }
        }
        moment = (moment + 1n);
    }
    return { covered: covered, gaps: gaps, longest_gap: longest };
}
