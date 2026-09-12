fn safe_div(left: i64, right: i64) -> i64 { left / right }
fn safe_mod(left: i64, right: i64) -> i64 { left % right }
pub fn modular_power(base: i64, exponent: i64, modulus: i64) -> i64 {
    let mut power: i64 = safe_mod(base, modulus);
    let mut remaining: i64 = exponent;
    let mut answer: i64 = safe_mod(1, modulus);
    while (remaining > 0) {
        if (safe_mod(remaining, 2) == 1) {
            answer = safe_mod((answer * power), modulus);
        }
        power = safe_mod((power * power), modulus);
        remaining = safe_div(remaining, 2);
    }
    return answer;
}
