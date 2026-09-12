fn safe_div(left: i64, right: i64) -> i64 { left / right }
fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn digit_checksum(n: i64) -> i64 {
    let mut remaining: i64 = n;
    let mut sign: i64 = 1;
    let mut checksum: i64 = 0;
    while (remaining > 0) {
        checksum = (checksum + (sign * safe_mod(remaining, 10)));
        sign = -(sign);
        remaining = safe_div(remaining, 10);
    }
    return checksum;
}
