fn safe_div(left: i64, right: i64) -> i64 { left / right }
pub fn integer_sqrt(n: i64) -> i64 {
    let mut lower: i64 = 0;
    let mut upper: i64 = 1000001;
    while ((lower + 1) < upper) {
        let mut middle: i64 = safe_div((lower + upper), 2);
        if ((middle * middle) <= n) {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    return lower;
}
