fn safe_div(left: i64, right: i64) -> i64 { left / right }
fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn collatz_steps(n: i64, limit: i64) -> Option<i64> {
    let mut current: i64 = n;
    let mut steps: i64 = 0;
    while (current != 1) {
        if (steps == limit) {
            return None;
        }
        if (safe_mod(current, 2) == 0) {
            current = safe_div(current, 2);
        } else {
            current = ((3 * current) + 1);
        }
        steps = (steps + 1);
    }
    return Some(steps);
}
