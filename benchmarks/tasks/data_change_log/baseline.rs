#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub event: i64,
    pub key: i64,
    pub delta: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub applied: i64,
    pub duplicates: i64,
    pub checksum: i64,
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
    let mut seen: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut applied: i64 = 0;
    let mut duplicates: i64 = 0;
    let mut checksum: i64 = 0;
    for row in rows {
        if (seen.get(&row.event).copied().unwrap_or(0) == 0) {
            let mut updated_value_0: i64 = 1;
            seen = put(seen, row.event, updated_value_0);
            checksum = (checksum + ((row.key + 1) * row.delta));
            applied = (applied + 1);
        } else {
            duplicates = (duplicates + 1);
        }
    }
    return Report {
        applied: applied,
        duplicates: duplicates,
        checksum: checksum,
    };
}
