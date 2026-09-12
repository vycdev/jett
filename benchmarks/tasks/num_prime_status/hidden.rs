include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(prime_status(-100), false, "case 0");
    assert_eq!(prime_status(-1), false, "case 1");
    assert_eq!(prime_status(0), false, "case 2");
    assert_eq!(prime_status(1), false, "case 3");
    assert_eq!(prime_status(2), true, "case 4");
    assert_eq!(prime_status(3), true, "case 5");
    assert_eq!(prime_status(4), false, "case 6");
    assert_eq!(prime_status(9), false, "case 7");
    assert_eq!(prime_status(25), false, "case 8");
    assert_eq!(prime_status(49), false, "case 9");
    assert_eq!(prime_status(97), true, "case 10");
    assert_eq!(prime_status(121), false, "case 11");
    assert_eq!(prime_status(997), true, "case 12");
    assert_eq!(prime_status(1024), false, "case 13");
    assert_eq!(prime_status(65521), true, "case 14");
    assert_eq!(prime_status(999983), true, "case 15");
    assert_eq!(prime_status(1000000), false, "case 16");
}
