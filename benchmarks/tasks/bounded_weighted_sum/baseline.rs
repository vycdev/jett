pub fn bounded_weighted_sum(values: &[i64], cap: i64) -> i64 {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (*value).clamp(-cap, cap) * (index as i64 + 1))
        .sum()
}
