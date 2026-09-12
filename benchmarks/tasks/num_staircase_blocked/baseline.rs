fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
fn has_step(blocked: &[i64], step: i64) -> bool {
    let mut index: i64 = 0;
    while (index < i64::try_from(blocked.len()).unwrap_or(0)) {
        if (nth(&blocked, index) == step) {
            return true;
        }
        index = (index + 1);
    }
    return false;
}
pub fn staircase_blocked(height: i64, blocked: &[i64]) -> i64 {
    let mut previous: i64 = 0;
    let mut current: i64 = 1;
    let mut step: i64 = 1;
    while (step <= height) {
        let mut following: i64 = (current + previous);
        if has_step(&blocked, step) {
            following = 0;
        }
        previous = current;
        current = following;
        step = (step + 1);
    }
    return current;
}
