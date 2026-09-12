fn safe_div(left: i64, right: i64) -> i64 { left / right }
pub fn binomial(n: i64, k: i64) -> i64 {
    if (k > n) {
        return 0;
    }
    let mut answer: i64 = 1;
    let mut index: i64 = 1;
    while (index <= k) {
        answer = safe_div((answer * ((n - index) + 1)), index);
        index = (index + 1);
    }
    return answer;
}
