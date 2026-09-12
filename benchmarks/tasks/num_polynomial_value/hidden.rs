include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(polynomial_value(&[], 3), 0, "case 0");
    assert_eq!(polynomial_value(&[5], 0), 5, "case 1");
    assert_eq!(polynomial_value(&[5], -10), 5, "case 2");
    assert_eq!(polynomial_value(&[1, 2, 3], 0), 1, "case 3");
    assert_eq!(polynomial_value(&[1, 2, 3], 1), 6, "case 4");
    assert_eq!(polynomial_value(&[1, 2, 3], 2), 17, "case 5");
    assert_eq!(polynomial_value(&[1, 2, 3], -2), 9, "case 6");
    assert_eq!(polynomial_value(&[0, 0, 1], 3), 9, "case 7");
    assert_eq!(polynomial_value(&[1, -1, 1, -1], -1), 4, "case 8");
    assert_eq!(polynomial_value(&[100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100], 10), 11111111111100, "case 9");
    assert_eq!(polynomial_value(&[-100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100, -100], -10), 9090909090900, "case 10");
    assert_eq!(polynomial_value(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], 10), 0, "case 11");
    assert_eq!(polynomial_value(&[3, 0, -4, 0, 5], 2), 67, "case 12");
}
