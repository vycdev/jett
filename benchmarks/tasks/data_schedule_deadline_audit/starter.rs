#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub duration: i64,
    pub deadline: i64,
    pub penalty: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub late: i64,
    pub weighted_tardiness: i64,
    pub finish: i64,
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut late: i64 = 0;
    let mut weighted: i64 = 0;
    let mut finish: i64 = limit;
    for row in rows {
        finish = (finish + row.duration);
        if (finish >= row.deadline) {
            late = (late + 1);
            weighted = (weighted + ((finish - row.deadline) * row.penalty));
        }
    }
    return Report {
        late: late,
        weighted_tardiness: weighted,
        finish: finish,
    };
}
