#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub patient: i64,
    pub severity: i64,
    pub arrival: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub selected: i64,
    pub patient_checksum: i64,
    pub severity_sum: i64,
}

pub fn precedes(
    severity: i64,
    arrival: i64,
    index: i64,
    other_severity: i64,
    other_arrival: i64,
    other_index: i64,
) -> bool {
    if (other_severity != severity) {
        return (other_severity > severity);
    }
    if (other_arrival != arrival) {
        return (other_arrival < arrival);
    }
    return (other_index < index);
}

pub fn rank_of(rows: &[Entry], severity: i64, arrival: i64, index: i64) -> i64 {
    let mut rank: i64 = 1;
    let mut other_index: i64 = 0;
    for other in rows {
        if precedes(
            severity,
            arrival,
            index,
            other.severity,
            other.arrival,
            other_index,
        ) {
            rank = (rank + 1);
        }
        other_index = (other_index + 1);
    }
    return rank;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut selected: i64 = 0;
    let mut checksum: i64 = 0;
    let mut total: i64 = 0;
    let mut index: i64 = 0;
    for row in rows {
        let mut rank: i64 = rank_of(rows, row.severity, row.arrival, index);
        if (rank <= limit) {
            selected = (selected + 1);
            checksum = (checksum + (rank * (row.patient + 1)));
            total = (total + row.severity);
        }
        index = (index + 1);
    }
    return Report {
        selected: selected,
        patient_checksum: checksum,
        severity_sum: total,
    };
}
