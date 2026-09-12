#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub resource_id: i64,
    pub timestamp: i64,
    pub duration: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub accepted: i64,
    pub rejected: i64,
    pub expires_sum: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn solve(rows: &[Entry]) -> Report {
    let mut expiries: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut accepted: i64 = 0;
    let mut rejected: i64 = 0;
    let mut total: i64 = 0;
    for row in rows {
        let mut prior: i64 = expiries.get(&row.resource_id).copied().unwrap_or(0);
        if ((row.timestamp >= prior) && (row.duration > 0)) {
            let mut next_expiry: i64 = (row.timestamp + row.duration);
            total = ((total + next_expiry) - prior);
            let mut updated_value_0: i64 = next_expiry;
            expiries = put(expiries, row.resource_id, updated_value_0);
            accepted = (accepted + 1);
        } else {
            rejected = (rejected + 1);
        }
    }
    return Report {
        accepted: accepted,
        rejected: rejected,
        expires_sum: total,
    };
}
