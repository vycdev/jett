#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: i64,
    pub target: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub reachable: i64,
    pub unreachable: i64,
    pub reachable_edge_count: i64,
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
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    if (limit > 0) {
        let mut updated_value_0: i64 = 1;
        seen = put(seen, 0, updated_value_0);
    }
    let mut turn: i64 = 0;
    while (turn < limit) {
        for row in rows {
            if (seen.get(&row.source).copied().unwrap_or(0) == 1) {
                let mut updated_value_1: i64 = 1;
                seen = put(seen, row.target, updated_value_1);
            }
        }
        turn = (turn + 1);
    }
    let mut reachable: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        reachable = (reachable + seen.get(&vertex).copied().unwrap_or(0));
        vertex = (vertex + 1);
    }
    let mut edges: i64 = 0;
    for row in rows {
        edges = (edges + seen.get(&row.source).copied().unwrap_or(0));
    }
    return Report {
        reachable: reachable,
        unreachable: (limit - reachable),
        reachable_edge_count: edges,
    };
}
