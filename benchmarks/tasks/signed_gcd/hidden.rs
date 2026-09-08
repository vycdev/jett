include!("solution.rs");

#[test]
fn hidden_cases() {
    assert_eq!(signed_gcd(0, 0), 0);
    assert_eq!(signed_gcd(54, 24), 6);
    assert_eq!(signed_gcd(-54, 24), 6);
    assert_eq!(signed_gcd(54, -24), 6);
    assert_eq!(signed_gcd(-7, -13), 1);
    assert_eq!(signed_gcd(0, 42), 42);
    assert_eq!(signed_gcd(270, 192), 6);
}
