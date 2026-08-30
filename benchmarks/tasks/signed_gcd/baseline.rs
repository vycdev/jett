pub fn signed_gcd(a: i64, b: i64) -> i64 {
    let (mut left, mut right) = (a.abs(), b.abs());
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}
