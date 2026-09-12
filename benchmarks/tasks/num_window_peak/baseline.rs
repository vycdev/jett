fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn window_peak(values: &[i64], width: i64) -> Option<i64> {
    if (width == 0) {
        return None;
    }
    if (width > i64::try_from(values.len()).unwrap_or(0)) {
        return None;
    }
    let mut total: i64 = 0;
    let mut index: i64 = 0;
    while (index < width) {
        total = (total + nth(&values, index));
        index = (index + 1);
    }
    let mut best: i64 = total;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        total = ((total + nth(&values, index)) - nth(&values, (index - width)));
        if (total > best) {
            best = total;
        }
        index = (index + 1);
    }
    return Some(best);
}
