fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn lis_length(values: &[i64]) -> i64 {
    let mut lengths: Vec<i64> = vec![];
    let mut best: i64 = 0;
    let mut index: i64 = 0;
    while (index < i64::try_from(values.len()).unwrap_or(0)) {
        let mut ending: i64 = 1;
        let mut prior: i64 = 0;
        while (prior < index) {
            if (nth(&values, prior) < nth(&values, index)) {
                let mut candidate: i64 = (nth(&lengths, prior) + 1);
                if (candidate > ending) {
                    ending = candidate;
                }
            }
            prior = (prior + 1);
        }
        lengths.push(ending);
        if (ending > best) {
            best = ending;
        }
        index = (index + 1);
    }
    return best;
}
