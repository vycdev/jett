fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn polynomial_value(coefficients: &[i64], x: i64) -> i64 {
    let mut answer: i64 = 0;
    let mut index: i64 = i64::try_from(coefficients.len()).unwrap_or(0);
    while (index > 0) {
        index = (index - 1);
        answer = ((answer * x) + nth(&coefficients, index));
    }
    return answer;
}
