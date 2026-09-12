fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn max_product_pair(values: &[i64]) -> Option<i64> {
    if (i64::try_from(values.len()).unwrap_or(0) < 2) {
        return None;
    }
    let mut best: i64 = (nth(&values, 0) * nth(&values, 1));
    let mut first: i64 = 0;
    while (first < i64::try_from(values.len()).unwrap_or(0)) {
        let mut second: i64 = (first + 1);
        while (second < i64::try_from(values.len()).unwrap_or(0)) {
            let mut candidate: i64 = (nth(&values, first) * nth(&values, second));
            if (candidate > best) {
                best = candidate;
            }
            second = (second + 1);
        }
        first = (first + 1);
    }
    return Some(best);
}
