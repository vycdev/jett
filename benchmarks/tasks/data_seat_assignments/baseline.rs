#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub passenger: i64,
    pub preferred: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub assigned: i64,
    pub rejected: i64,
    pub seat_checksum: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn choose_seat(
    occupied: &std::collections::BTreeMap<i64, i64>,
    preferred: i64,
    limit: i64,
) -> i64 {
    if ((preferred > 0) && (preferred <= limit)) {
        if (occupied.get(&preferred).copied().unwrap_or(0) == 0) {
            return preferred;
        }
    }
    let mut seat: i64 = 1;
    while (seat <= limit) {
        if (occupied.get(&seat).copied().unwrap_or(0) == 0) {
            return seat;
        }
        seat = (seat + 1);
    }
    return 0;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut occupied: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut passengers: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut assigned: i64 = 0;
    let mut rejected: i64 = 0;
    let mut checksum: i64 = 0;
    for row in rows {
        let mut seat: i64 = 0;
        if (passengers.get(&row.passenger).copied().unwrap_or(0) == 0) {
            seat = choose_seat(&occupied, row.preferred, limit);
        }
        if (seat == 0) {
            rejected = (rejected + 1);
        } else {
            assigned = (assigned + 1);
            checksum = (checksum + ((row.passenger + 1) * seat));
            let mut updated_value_0: i64 = 1;
            occupied = put(occupied, seat, updated_value_0);
            let mut updated_value_1: i64 = 1;
            passengers = put(passengers, row.passenger, updated_value_1);
        }
    }
    return Report {
        assigned: assigned,
        rejected: rejected,
        seat_checksum: checksum,
    };
}
