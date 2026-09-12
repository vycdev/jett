#[derive(Debug, PartialEq, Eq)]
pub struct SearchRange { pub first: i64, pub last: i64 }
fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn search_range(values: &[i64], target: i64) -> SearchRange {
    let mut first: i64 = -(1);
    let mut last: i64 = -(1);
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        if (nth(&values, index) == target) {
            if (first == -(1)) {
                first = index;
            }
            last = index;
        }
        index = (index + 1);
    }
    return SearchRange {first: first, last: last};
}
