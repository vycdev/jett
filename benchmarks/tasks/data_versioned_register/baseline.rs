#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: i64,
    pub version: i64,
    pub value: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub accepted: i64,
    pub stale: i64,
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
    let mut versions: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut values: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut accepted: i64 = 0;
    let mut stale: i64 = 0;
    let mut checksum: i64 = 0;
    for row in rows {
        if (row.version > versions.get(&row.key).copied().unwrap_or((-1))) {
            checksum = (checksum
                + ((row.key + 1) * (row.value - values.get(&row.key).copied().unwrap_or(0))));
            let mut updated_value_0: i64 = row.value;
            values = put(values, row.key, updated_value_0);
            let mut updated_value_1: i64 = row.version;
            versions = put(versions, row.key, updated_value_1);
            accepted = (accepted + 1);
        } else {
            stale = (stale + 1);
        }
    }
    return Report {
        accepted: accepted,
        stale: stale,
        checksum: checksum,
    };
}
