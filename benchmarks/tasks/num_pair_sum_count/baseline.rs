fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn pair_sum_count(values: &[i64], target: i64) -> i64 {
    let mut count: i64 = 0;
    let mut first: i64 = 0;
    while (first < i64::try_from(values.len()).unwrap_or(0)) {
        let mut second: i64 = (first + 1);
        while (second < i64::try_from(values.len()).unwrap_or(0)) {
            if ((nth(&values, first) + nth(&values, second)) == target) {
                count = (count + 1);
            }
            second = (second + 1);
        }
        first = (first + 1);
    }
    return count;
}
