#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: i64,
    pub side: i64,
    pub quantity: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub matched: i64,
    pub left_only: i64,
    pub right_only: i64,
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
    let mut left: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut right: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for row in rows {
        if (row.side == 0) {
            let mut updated_value_0: i64 =
                (left.get(&row.key).copied().unwrap_or(0) + row.quantity);
            left = put(left, row.key, updated_value_0);
        }
        if (row.side == 1) {
            let mut updated_value_1: i64 =
                (right.get(&row.key).copied().unwrap_or(0) + row.quantity);
            right = put(right, row.key, updated_value_1);
        }
    }
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut matched: i64 = 0;
    let mut left_only: i64 = 0;
    let mut right_only: i64 = 0;
    for row in rows {
        if (seen.get(&row.key).copied().unwrap_or(0) == 0) {
            let mut updated_value_2: i64 = 1;
            seen = put(seen, row.key, updated_value_2);
            let mut a: i64 = left.get(&row.key).copied().unwrap_or(0);
            let mut b: i64 = right.get(&row.key).copied().unwrap_or(0);
            let mut pairs: i64 = a;
            if (b < pairs) {
                pairs = b;
            }
            matched = (matched + pairs);
            left_only = ((left_only + a) - pairs);
            right_only = ((right_only + b) - pairs);
        }
    }
    return Report {
        matched: matched,
        left_only: left_only,
        right_only: right_only,
    };
}
