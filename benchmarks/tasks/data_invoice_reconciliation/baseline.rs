#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub invoice: i64,
    pub amount: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub settled: i64,
    pub outstanding: i64,
    pub credit: i64,
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
    let mut sums: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for row in rows {
        let mut updated_value_0: i64 = (sums.get(&row.invoice).copied().unwrap_or(0) + row.amount);
        sums = put(sums, row.invoice, updated_value_0);
    }
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut settled: i64 = 0;
    let mut outstanding: i64 = 0;
    let mut credit: i64 = 0;
    for row in rows {
        if (seen.get(&row.invoice).copied().unwrap_or(0) == 0) {
            let mut updated_value_1: i64 = 1;
            seen = put(seen, row.invoice, updated_value_1);
            let mut balance: i64 = sums.get(&row.invoice).copied().unwrap_or(0);
            if (balance == 0) {
                settled = (settled + 1);
            }
            if (balance > 0) {
                outstanding = (outstanding + balance);
            }
            if (balance < 0) {
                credit = (credit - balance);
            }
        }
    }
    return Report {
        settled: settled,
        outstanding: outstanding,
        credit: credit,
    };
}
