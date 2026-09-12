fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn equilibrium_index(values: &[i64]) -> Option<i64> {
    let mut remaining: i64 = 0;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        remaining = (remaining + nth(&values, index));
        index = (index + 1);
    }
    let mut before: i64 = 0;
    index = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        let mut value: i64 = nth(&values, index);
        remaining = (remaining - value);
        if (before == remaining) {
            return Some(index);
        }
        before = (before + value);
        index = (index + 1);
    }
    return None;
}
