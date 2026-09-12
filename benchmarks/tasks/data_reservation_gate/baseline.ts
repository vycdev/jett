
export interface Entry {
    readonly party: bigint;
    readonly seats: bigint;
}

export interface Report {
    readonly accepted: bigint;
    readonly rejected: bigint;
    readonly occupied: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let parties: Map<bigint, bigint> = new Map<bigint, bigint>();
    let accepted: bigint = 0n;
    let rejected: bigint = 0n;
    let occupied: bigint = 0n;
    for (const row of rows) {
        let invalid: boolean = ((row.seats <= 0n) || ((parties.get(row.party) ?? 0n) == 1n));
        if ((invalid || ((occupied + row.seats) > limit))) {
            rejected = (rejected + 1n);
        }
        else {
            accepted = (accepted + 1n);
            occupied = (occupied + row.seats);
            let updated_value_0: bigint = 1n;
            parties = parties.set(row.party, updated_value_0);
        }
    }
    return { accepted: accepted, rejected: rejected, occupied: occupied };
}
