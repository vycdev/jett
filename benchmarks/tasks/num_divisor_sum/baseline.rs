fn safe_div(left: i64, right: i64) -> i64 { left / right }
fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn divisor_sum(n: i64) -> i64 {
    let mut total: i64 = 0;
    let mut divisor: i64 = 1;
    while ((divisor * divisor) <= n) {
        if (safe_mod(n, divisor) == 0) {
            total = (total + divisor);
            let mut partner: i64 = safe_div(n, divisor);
            if (partner != divisor) {
                total = (total + partner);
            }
        }
        divisor = (divisor + 1);
    }
    return total;
}
