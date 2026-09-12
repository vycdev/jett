fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn longest_positive_run(values: &[i64]) -> i64 {
    let mut current: i64 = 0;
    let mut best: i64 = 0;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        if (nth(&values, index) > 0) {
            current = (current + 1);
            if (current > best) {
                best = current;
            }
        } else {
            current = 0;
        }
        index = (index + 1);
    }
    return best;
}
