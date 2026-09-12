fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn rotate_left(values: &[i64], distance: i64) -> Vec<i64> {
    let mut rotated: Vec<i64> = vec![];
    let mut count: i64 = i64::try_from(values.len()).unwrap_or(0);
    if (count == 0) {
        return rotated;
    }
    let mut shift: i64 = safe_mod(distance, count);
    let mut index: i64 = 0;
    while (index < count) {
        let mut position: i64 = safe_mod((index + shift), count);
        rotated.push(nth(&values, position));
        index = (index + 1);
    }
    return rotated;
}
