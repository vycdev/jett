#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub job: i64,
    pub prerequisite: i64,
    pub cost: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub ready: i64,
    pub blocked: i64,
    pub total_cost: i64,
}

pub fn ready_job(rows: &[Entry], job: i64) -> bool {
    let mut current: i64 = job;
    while (current != 0) {
        let mut parent: i64 = (-1);
        for row in rows {
            if (row.job == current) {
                parent = row.prerequisite;
            }
        }
        if (parent == (-1)) {
            return false;
        }
        current = parent;
    }
    return true;
}

pub fn solve(rows: &[Entry]) -> Report {
    let mut ready: i64 = 0;
    let mut blocked: i64 = 0;
    let mut total: i64 = 0;
    for row in rows {
        if ready_job(rows, row.job) {
            ready = (ready + 1);
            total = (total + row.cost);
        } else {
            blocked = (blocked + 1);
        }
    }
    return Report {
        ready: ready,
        blocked: blocked,
        total_cost: total,
    };
}
