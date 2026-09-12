#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub start: i64,
    pub end: i64,
    pub resource_id: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub conflict_pairs: i64,
    pub affected_bookings: i64,
    pub longest_overlap: i64,
}

pub fn overlap(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> i64 {
    let mut start: i64 = a_start;
    let mut end: i64 = a_end;
    if (b_start > start) {
        start = b_start;
    }
    if (b_end < end) {
        end = b_end;
    }
    if (end > start) {
        return (end - start);
    }
    return 0;
}

pub fn solve(rows: &[Entry]) -> Report {
    let mut pairs: i64 = 0;
    let mut affected: i64 = 0;
    let mut longest: i64 = 0;
    let mut left_index: i64 = 0;
    for left in rows {
        let mut hit: bool = false;
        let mut right_index: i64 = 0;
        for right in rows {
            let mut duration: i64 = 0;
            if ((left_index != right_index) && (left.resource_id == right.resource_id)) {
                duration = overlap(left.start, left.end, right.start, right.end);
            }
            if (duration > 0) {
                hit = true;
                if (left_index < right_index) {
                    pairs = (pairs + 1);
                }
                if (duration > longest) {
                    longest = duration;
                }
            }
            right_index = (right_index + 1);
        }
        if hit {
            affected = (affected + 1);
        }
        left_index = (left_index + 1);
    }
    return Report {
        conflict_pairs: pairs,
        affected_bookings: affected,
        longest_overlap: longest,
    };
}
