fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn unique_sorted(values: &[i64]) -> Vec<i64> {
    let mut output: Vec<i64> = vec![];
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        let mut value: i64 = nth(&values, index);
        if (index == 0) {
            output.push(value);
        } else {
            if (value != nth(&values, (index - 1))) {
                output.push(value);
            }
        }
        index = (index + 1);
    }
    return output;
}
