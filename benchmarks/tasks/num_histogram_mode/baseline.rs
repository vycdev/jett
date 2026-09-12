#[derive(Debug, PartialEq, Eq)]
pub struct Mode { pub value: i64, pub count: i64 }
fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
fn occurrence_count(values: &[i64], target: i64) -> i64 {
    let mut count: i64 = 0;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        if (nth(&values, index) == target) {
            count = (count + 1);
        }
        index = (index + 1);
    }
    return count;
}
pub fn histogram_mode(values: &[i64]) -> Mode {
    let mut best_value: i64 = 0;
    let mut best_count: i64 = 0;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        let mut value: i64 = nth(&values, index);
        let mut count: i64 = occurrence_count(&values, value);
        let mut replace: bool = (count > best_count);
        if (count == best_count) {
            if (value < best_value) {
                replace = true;
            }
        }
        if replace {
            best_value = value;
            best_count = count;
        }
        index = (index + 1);
    }
    return Mode {value: best_value, count: best_count};
}
