#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub rooms: i64,
    pub assignment_checksum: i64,
    pub reuses: i64,
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
    let mut ends: std::collections::BTreeMap<i64, i64> = std::collections::BTreeMap::new();
    let mut rooms: i64 = 0;
    let mut checksum: i64 = 0;
    let mut reuses: i64 = 0;
    let mut index: i64 = 1;
    for row in rows {
        let mut selected: i64 = 0;
        let mut room: i64 = 1;
        while (room <= rooms) {
            if ((selected == 0) && (ends.get(&room).copied().unwrap_or(0) <= row.start)) {
                selected = room;
            }
            room = (room + 1);
        }
        if (selected == 0) {
            rooms = (rooms + 1);
            selected = rooms;
        } else {
            reuses = (reuses + 1);
        }
        let mut updated_value_0: i64 = row.end;
        ends = put(ends, selected, updated_value_0);
        checksum = (checksum + (index * selected));
        index = (index + 1);
    }
    return Report {
        rooms: rooms,
        assignment_checksum: checksum,
        reuses: reuses,
    };
}
