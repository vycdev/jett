#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: i64,
    pub target: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub components: i64,
    pub largest: i64,
    pub isolated: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn spread(rows: &[Entry], seen: &std::collections::BTreeMap<i64, i64>, vertex: i64) -> i64 {
    let mut found: i64 = seen.get(&vertex).copied().unwrap_or(0);
    for row in rows {
        if (row.source == vertex) {
            if (seen.get(&row.target).copied().unwrap_or(0) == 1) {
                found = 1;
            }
        }
        if (row.target == vertex) {
            if (seen.get(&row.source).copied().unwrap_or(0) == 1) {
                found = 1;
            }
        }
    }
    return found;
}

pub fn component_size(rows: &[Entry], start: i64, limit: i64) -> i64 {
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut updated_value_0: i64 = 1;
    seen = put(seen, start, updated_value_0);
    let mut turn: i64 = 0;
    while (turn < limit) {
        let mut vertex: i64 = 0;
        while (vertex < limit) {
            let mut updated_value_1: i64 = spread(rows, &seen, vertex);
            seen = put(seen, vertex, updated_value_1);
            vertex = (vertex + 1);
        }
        turn = (turn + 1);
    }
    let mut count: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        if (seen.get(&vertex).copied().unwrap_or(0) == 1) {
            if (vertex < start) {
                return 0;
            }
            count = (count + 1);
        }
        vertex = (vertex + 1);
    }
    return count;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut components: i64 = 0;
    let mut largest: i64 = 0;
    let mut isolated: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        let mut count: i64 = component_size(rows, vertex, limit);
        if (count > 0) {
            components = (components + 1);
        }
        if (count > largest) {
            largest = count;
        }
        let mut incident: i64 = 0;
        for row in rows {
            if ((row.source == vertex) || (row.target == vertex)) {
                incident = (incident + 1);
            }
        }
        if (incident == 0) {
            isolated = (isolated + 1);
        }
        vertex = (vertex + 1);
    }
    return Report {
        components: components,
        largest: largest,
        isolated: isolated,
    };
}
