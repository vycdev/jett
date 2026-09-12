#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub start: i64,
    pub end: i64,
    pub demand: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub peak: i64,
    pub earliest: i64,
    pub overloaded_starts: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn load_at(rows: &[Entry], moment: i64) -> i64 {
    let mut total: i64 = 0;
    for row in rows {
        if ((row.start <= moment) && (row.end > moment)) {
            total = (total + row.demand);
        }
    }
    return total;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut peak: i64 = 0;
    let mut earliest: i64 = (-1);
    let mut overloaded: i64 = 0;
    for row in rows {
        let mut load: i64 = load_at(rows, row.start);
        if ((load > peak) || ((load == peak) && (row.start < earliest))) {
            peak = load;
            earliest = row.start;
        }
        if (seen.get(&row.start).copied().unwrap_or(0) == 0) {
            if (load > limit) {
                overloaded = (overloaded + 1);
            }
            let mut updated_value_0: i64 = 1;
            seen = put(seen, row.start, updated_value_0);
        }
    }
    return Report {
        peak: peak,
        earliest: earliest,
        overloaded_starts: overloaded,
    };
}
