#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub covered: i64,
    pub gaps: i64,
    pub longest_gap: i64,
}

pub fn covered_at(rows: &[Entry], moment: i64) -> bool {
    for row in rows {
        if ((row.start <= moment) && (row.end > moment)) {
            return true;
        }
    }
    return false;
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut covered: i64 = 0;
    let mut gaps: i64 = 0;
    let mut longest: i64 = 0;
    let mut current: i64 = 0;
    let mut moment: i64 = 0;
    while (moment < limit) {
        if covered_at(rows, moment) {
            covered = (covered + 1);
            current = 0;
        } else {
            if (current == 0) {
                gaps = (gaps + 1);
            }
            current = (current + 1);
            if (current > longest) {
                longest = current;
            }
        }
        moment = (moment + 1);
    }
    return Report {
        covered: covered,
        gaps: gaps,
        longest_gap: longest,
    };
}
