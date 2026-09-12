fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn max_subarray(values: &[i64]) -> Option<i64> {
    if (i64::try_from(values.len()).unwrap_or(0) == 0) {
        return None;
    }
    let mut ending: i64 = nth(&values, 0);
    let mut best: i64 = ending;
    let mut index: i64 = 1;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        let mut value: i64 = nth(&values, index);
        if (ending > 0) {
            ending = (ending + value);
        } else {
            ending = value;
        }
        if (ending > best) {
            best = ending;
        }
        index = (index + 1);
    }
    return Some(best);
}
