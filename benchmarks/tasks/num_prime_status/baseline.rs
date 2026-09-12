fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn prime_status(n: i64) -> bool {
    if (n < 2) {
        return false;
    }
    let mut divisor: i64 = 2;
    while ((divisor * divisor) <= n) {
        if (safe_mod(n, divisor) == 0) {
            return false;
        }
        divisor = (divisor + 1);
    }
    return true;
}
