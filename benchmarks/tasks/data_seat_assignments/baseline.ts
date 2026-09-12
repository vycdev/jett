
export interface Entry {
    readonly passenger: bigint;
    readonly preferred: bigint;
}

export interface Report {
    readonly assigned: bigint;
    readonly rejected: bigint;
    readonly seat_checksum: bigint;
}

export function choose_seat(occupied: Map<bigint, bigint>, preferred: bigint, limit: bigint): bigint {
    if (((preferred > 0n) && (preferred <= limit))) {
        if (((occupied.get(preferred) ?? 0n) == 0n)) {
            return preferred;
        }
    }
    let seat: bigint = 1n;
    while ((seat <= limit)) {
        if (((occupied.get(seat) ?? 0n) == 0n)) {
            return seat;
        }
        seat = (seat + 1n);
    }
    return 0n;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let occupied: Map<bigint, bigint> = new Map<bigint, bigint>();
    let passengers: Map<bigint, bigint> = new Map<bigint, bigint>();
    let assigned: bigint = 0n;
    let rejected: bigint = 0n;
    let checksum: bigint = 0n;
    for (const row of rows) {
        let seat: bigint = 0n;
        if (((passengers.get(row.passenger) ?? 0n) == 0n)) {
            seat = choose_seat(occupied, row.preferred, limit);
        }
        if ((seat == 0n)) {
            rejected = (rejected + 1n);
        }
        else {
            assigned = (assigned + 1n);
            checksum = (checksum + ((row.passenger + 1n) * seat));
            let updated_value_0: bigint = 1n;
            occupied = occupied.set(seat, updated_value_0);
            let updated_value_1: bigint = 1n;
            passengers = passengers.set(row.passenger, updated_value_1);
        }
    }
    return { assigned: assigned, rejected: rejected, seat_checksum: checksum };
}
