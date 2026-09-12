#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub sensor: i64,
    pub operation: i64,
    pub timestamp: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub completed: i64,
    pub active: i64,
    pub rejected: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn step(operation: i64, timestamp: i64, opened: i64) -> i64 {
    if (timestamp < 0) {
        return (-2);
    }
    if ((operation == 0) && (opened == (-1))) {
        return timestamp;
    }
    if ((operation == 1) && (opened >= 0) && (timestamp >= opened)) {
        return (-1);
    }
    return (-2);
}

pub fn solve(rows: &[Entry]) -> Report {
    let mut opened: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut completed: i64 = 0;
    let mut active: i64 = 0;
    let mut rejected: i64 = 0;
    for row in rows {
        let mut prior: i64 = opened.get(&row.sensor).copied().unwrap_or((-1));
        let mut next_time: i64 = step(row.operation, row.timestamp, prior);
        if (next_time == (-2)) {
            rejected = (rejected + 1);
        } else {
            let mut updated_value_0: i64 = next_time;
            opened = put(opened, row.sensor, updated_value_0);
            if (next_time == (-1)) {
                completed = (completed + 1);
                active = (active - 1);
            } else {
                active = (active + 1);
            }
        }
    }
    return Report {
        completed: completed,
        active: active,
        rejected: rejected,
    };
}
