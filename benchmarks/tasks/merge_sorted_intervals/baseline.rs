#[derive(Debug, PartialEq, Eq)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
}

pub fn merge_sorted_intervals(intervals: Vec<Interval>) -> Vec<Interval> {
    let mut merged: Vec<Interval> = Vec::new();
    for interval in intervals {
        if let Some(current) = merged.last_mut()
            && interval.start <= current.end
        {
            if interval.end > current.end {
                current.end = interval.end;
            }
        } else {
            merged.push(Interval {
                start: interval.start,
                end: interval.end,
            });
        }
    }
    merged
}
