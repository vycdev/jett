
export interface Entry {
    readonly score: bigint;
    readonly penalty: bigint;
}

export interface Report {
    readonly selected: bigint;
    readonly rank_sum: bigint;
    readonly tie_groups: bigint;
}

export function ahead(score: bigint, penalty: bigint, other_score: bigint, other_penalty: bigint): boolean {
    return ((other_score > score) || ((other_score == score) && (other_penalty < penalty)));
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let selected: bigint = 0n;
    let rank_sum: bigint = 0n;
    let groups: bigint = 0n;
    let index: bigint = 0n;
    for (const row of rows) {
        let rank: bigint = 1n;
        let ties: bigint = 0n;
        let first: bigint = index;
        let other_index: bigint = 0n;
        for (const other of rows) {
            if (ahead(row.score, row.penalty, other.score, other.penalty)) {
                rank = (rank + 1n);
            }
            if (((row.score == other.score) && (row.penalty == other.penalty))) {
                ties = (ties + 1n);
                if ((other_index < first)) {
                    first = other_index;
                }
            }
            other_index = (other_index + 1n);
        }
        if ((rank <= limit)) {
            selected = (selected + 1n);
            rank_sum = (rank_sum + rank);
        }
        if (((ties > 1n) && (first == index))) {
            groups = (groups + 1n);
        }
        index = (index + 1n);
    }
    return { selected: selected, rank_sum: rank_sum, tie_groups: groups };
}
