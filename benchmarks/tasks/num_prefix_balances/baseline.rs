fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn prefix_balances(values: &[i64], opening: i64) -> Vec<i64> {
    let mut balances: Vec<i64> = vec![opening];
    let mut balance: i64 = opening;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        balance = (balance + nth(&values, index));
        balances.push(balance);
        index = (index + 1);
    }
    return balances;
}
