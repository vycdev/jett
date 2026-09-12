#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub prerequisite: i64,
    pub dependent: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub layers: i64,
    pub last_layer_count: i64,
    pub layer_sum: i64,
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
    let mut levels: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut maximum: i64 = (-1);
    let mut last_count: i64 = 0;
    let mut total: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        let mut level: i64 = 0;
        for row in rows {
            if (row.dependent == vertex) {
                let mut prior: i64 = (levels.get(&row.prerequisite).copied().unwrap_or(0) + 1);
                if (prior > level) {
                    level = prior;
                }
            }
        }
        let mut updated_value_0: i64 = level;
        levels = put(levels, vertex, updated_value_0);
        total = (total + level);
        if (level > maximum) {
            maximum = level;
            last_count = 0;
        }
        if (level == maximum) {
            last_count = (last_count + 1);
        }
        vertex = (vertex + 1);
    }
    return Report {
        layers: (maximum + 1),
        last_layer_count: last_count,
        layer_sum: total,
    };
}
