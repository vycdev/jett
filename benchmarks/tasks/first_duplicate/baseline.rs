use std::collections::HashSet;

pub fn first_duplicate(values: &[i64]) -> Option<i64> {
    let mut seen = HashSet::new();
    for &value in values {
        if !seen.insert(value) {
            return Some(value);
        }
    }
    None
}
