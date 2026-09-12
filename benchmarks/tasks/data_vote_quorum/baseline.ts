
export interface Entry {
    readonly voter: bigint;
    readonly choice: bigint;
    readonly weight: bigint;
}

export interface Report {
    readonly yes_weight: bigint;
    readonly no_weight: bigint;
    readonly quorum_met: bigint;
}

export function contribution(choice: bigint, weight: bigint, wanted: bigint): bigint {
    if (((choice == wanted) && (weight > 0n))) {
        return weight;
    }
    return 0n;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let yes_votes: Map<bigint, bigint> = new Map<bigint, bigint>();
    let no_votes: Map<bigint, bigint> = new Map<bigint, bigint>();
    let yes: bigint = 0n;
    let no: bigint = 0n;
    for (const row of rows) {
        let next_yes: bigint = contribution(row.choice, row.weight, 1n);
        let next_no: bigint = contribution(row.choice, row.weight, 0n);
        yes = ((yes + next_yes) - (yes_votes.get(row.voter) ?? 0n));
        no = ((no + next_no) - (no_votes.get(row.voter) ?? 0n));
        let updated_value_0: bigint = next_yes;
        yes_votes = yes_votes.set(row.voter, updated_value_0);
        let updated_value_1: bigint = next_no;
        no_votes = no_votes.set(row.voter, updated_value_1);
    }
    let met: bigint = 0n;
    if (((yes >= limit) && (yes > no))) {
        met = 1n;
    }
    return { yes_weight: yes, no_weight: no, quorum_met: met };
}
