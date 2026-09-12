#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub party: i64,
    pub seats: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub accepted: i64,
    pub rejected: i64,
    pub occupied: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut parties: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut accepted: i64 = 0;
    let mut rejected: i64 = 0;
    let mut occupied: i64 = 0;
    for row in rows {
        let mut invalid: bool =
            ((row.seats <= 0) || (parties.get(&row.party).copied().unwrap_or(0) == 1));
        if (invalid || ((occupied + row.seats) > limit)) {
            rejected = (rejected + 1);
        } else {
            accepted = (accepted + 1);
            occupied = (occupied + row.seats);
            let mut updated_value_0: i64 = 1;
            parties = put(parties, row.party, updated_value_0);
        }
    }
    return Report {
        accepted: accepted,
        rejected: rejected,
        occupied: occupied,
    };
}
