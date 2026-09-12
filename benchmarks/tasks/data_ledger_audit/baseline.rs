#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub account: i64,
    pub delta: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub total: i64,
    pub lowest_balance: i64,
    pub first_overdraw: i64,
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
    let mut balances: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut total: i64 = 0;
    let mut lowest: i64 = limit;
    let mut first: i64 = (-1);
    let mut index: i64 = 0;
    for row in rows {
        if (seen.get(&row.account).copied().unwrap_or(0) == 0) {
            total = (total + limit);
            let mut updated_value_0: i64 = 1;
            seen = put(seen, row.account, updated_value_0);
        }
        let mut balance: i64 = (balances.get(&row.account).copied().unwrap_or(limit) + row.delta);
        let mut updated_value_1: i64 = balance;
        balances = put(balances, row.account, updated_value_1);
        total = (total + row.delta);
        if ((index == 0) || (balance < lowest)) {
            lowest = balance;
        }
        if ((balance < 0) && (first == (-1))) {
            first = index;
        }
        index = (index + 1);
    }
    return Report {
        total: total,
        lowest_balance: lowest,
        first_overdraw: first,
    };
}
