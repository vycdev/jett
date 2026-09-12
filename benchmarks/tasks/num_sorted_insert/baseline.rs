fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn sorted_insert(values: &[i64], value: i64) -> Vec<i64> {
    let mut output: Vec<i64> = vec![];
    let mut inserted: bool = false;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        let mut item: i64 = nth(&values, index);
        if !(inserted) {
            if (value <= item) {
                output.push(value);
                inserted = true;
            }
        }
        output.push(item);
        index = (index + 1);
    }
    if !(inserted) {
        output.push(value);
    }
    return output;
}
