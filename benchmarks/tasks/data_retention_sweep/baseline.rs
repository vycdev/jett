#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: i64,
    pub modified: i64,
    pub size: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub removed: i64,
    pub reclaimed: i64,
    pub retained: i64,
}

pub fn newest_index(rows: &[Entry], key: i64) -> i64 {
    let mut best_index: i64 = (-1);
    let mut best_time: i64 = (-1000000);
    let mut index: i64 = 0;
    for row in rows {
        if ((row.key == key) && (row.modified >= best_time)) {
            best_index = index;
            best_time = row.modified;
        }
        index = (index + 1);
    }
    return best_index;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut removed: i64 = 0;
    let mut reclaimed: i64 = 0;
    let mut retained: i64 = 0;
    let mut index: i64 = 0;
    for row in rows {
        if ((row.modified < limit) && (index != newest_index(rows, row.key))) {
            removed = (removed + 1);
            reclaimed = (reclaimed + row.size);
        } else {
            retained = (retained + 1);
        }
        index = (index + 1);
    }
    return Report {
        removed: removed,
        reclaimed: reclaimed,
        retained: retained,
    };
}
