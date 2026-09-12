#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: i64,
    pub target: i64,
    pub cost: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub reachable: i64,
    pub cost_sum: i64,
    pub most_expensive: i64,
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
    let mut costs: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut updated_value_0: i64 = 0;
    costs = put(costs, 0, updated_value_0);
    let mut reachable: i64 = 0;
    let mut total: i64 = 0;
    let mut maximum: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        let mut best: i64 = costs.get(&vertex).copied().unwrap_or(1000000);
        for row in rows {
            if (row.target == vertex) {
                let mut candidate: i64 =
                    (costs.get(&row.source).copied().unwrap_or(1000000) + row.cost);
                if (candidate < best) {
                    best = candidate;
                }
            }
        }
        let mut updated_value_1: i64 = best;
        costs = put(costs, vertex, updated_value_1);
        if (best < 1000000) {
            reachable = (reachable + 1);
            total = (total + best);
            if (best > maximum) {
                maximum = best;
            }
        }
        vertex = (vertex + 1);
    }
    return Report {
        reachable: reachable,
        cost_sum: total,
        most_expensive: maximum,
    };
}
