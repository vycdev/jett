fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn coin_change(coins: &[i64], amount: i64) -> i64 {
    let mut costs: Vec<i64> = vec![0];
    let mut total: i64 = 1;
    while (total <= amount) {
        let mut best: i64 = 1000;
        let mut index: i64 = 0;
        while (index < i64::try_from(coins.len()).unwrap_or(0)) {
            let mut coin: i64 = nth(&coins, index);
            if (coin <= total) {
                let mut prior: i64 = nth(&costs, (total - coin));
                if ((prior + 1) < best) {
                    best = (prior + 1);
                }
            }
            index = (index + 1);
        }
        costs.push(best);
        total = (total + 1);
    }
    let mut answer: i64 = nth(&costs, amount);
    if (answer == 1000) {
        return -(1);
    }
    return answer;
}
