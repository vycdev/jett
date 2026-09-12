include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(divisor_sum(1), 1, "case 0");
    assert_eq!(divisor_sum(2), 3, "case 1");
    assert_eq!(divisor_sum(3), 4, "case 2");
    assert_eq!(divisor_sum(4), 7, "case 3");
    assert_eq!(divisor_sum(6), 12, "case 4");
    assert_eq!(divisor_sum(12), 28, "case 5");
    assert_eq!(divisor_sum(16), 31, "case 6");
    assert_eq!(divisor_sum(25), 31, "case 7");
    assert_eq!(divisor_sum(36), 91, "case 8");
    assert_eq!(divisor_sum(49), 57, "case 9");
    assert_eq!(divisor_sum(64), 127, "case 10");
    assert_eq!(divisor_sum(97), 98, "case 11");
    assert_eq!(divisor_sum(120), 360, "case 12");
    assert_eq!(divisor_sum(360), 1170, "case 13");
    assert_eq!(divisor_sum(99991), 99992, "case 14");
    assert_eq!(divisor_sum(100000), 246078, "case 15");
}
