#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: i64,
    pub target: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub distance_sum: i64,
    pub farthest: i64,
    pub unreachable: i64,
}

fn put(
    mut table: std::collections::BTreeMap<i64, i64>,
    key: i64,
    value: i64,
) -> std::collections::BTreeMap<i64, i64> {
    table.insert(key, value);
    table
}

pub fn relax(
    rows: &[Entry],
    distance: &std::collections::BTreeMap<i64, i64>,
) -> std::collections::BTreeMap<i64, i64> {
    let mut updated: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    for row in rows {
        let mut prior: i64 = distance.get(&row.source).copied().unwrap_or(1000);
        let mut old: i64 = distance.get(&row.target).copied().unwrap_or(1000);
        let mut best: i64 = updated.get(&row.target).copied().unwrap_or(old);
        if ((prior + 1) < best) {
            let mut updated_value_0: i64 = (prior + 1);
            updated = put(updated, row.target, updated_value_0);
        }
    }
    return updated;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut distance: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut updated_value_1: i64 = 0;
    distance = put(distance, 0, updated_value_1);
    let mut turn: i64 = 0;
    while (turn < limit) {
        let mut updated: std::collections::BTreeMap<i64, i64> = relax(rows, &distance);
        let mut vertex: i64 = 0;
        while (vertex < limit) {
            let mut updated_value_2: i64 = updated
                .get(&vertex)
                .copied()
                .unwrap_or(distance.get(&vertex).copied().unwrap_or(1000));
            distance = put(distance, vertex, updated_value_2);
            vertex = (vertex + 1);
        }
        turn = (turn + 1);
    }
    let mut total: i64 = 0;
    let mut farthest: i64 = 0;
    let mut missing: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        let mut hops: i64 = distance.get(&vertex).copied().unwrap_or(1000);
        if (hops == 1000) {
            missing = (missing + 1);
        } else {
            total = (total + hops);
            if (hops > farthest) {
                farthest = hops;
            }
        }
        vertex = (vertex + 1);
    }
    return Report {
        distance_sum: total,
        farthest: farthest,
        unreachable: missing,
    };
}
