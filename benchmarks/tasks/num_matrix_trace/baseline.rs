fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn matrix_trace(values: &[i64], rows: i64) -> i64 {
    let mut total: i64 = 0;
    let mut row: i64 = 0;
    while (row < rows) {
        total = (total + nth(&values, ((row * rows) + row)));
        row = (row + 1);
    }
    return total;
}
