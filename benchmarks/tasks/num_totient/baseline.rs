fn safe_div(left: i64, right: i64) -> i64 { left / right }
fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn totient(n: i64) -> i64 {
    let mut remaining: i64 = n;
    let mut count: i64 = n;
    let mut factor: i64 = 2;
    while ((factor * factor) <= remaining) {
        if (safe_mod(remaining, factor) == 0) {
            count = (count - safe_div(count, factor));
            while (safe_mod(remaining, factor) == 0) {
                remaining = safe_div(remaining, factor);
            }
        }
        factor = (factor + 1);
    }
    if (remaining > 1) {
        count = (count - safe_div(count, remaining));
    }
    return count;
}
