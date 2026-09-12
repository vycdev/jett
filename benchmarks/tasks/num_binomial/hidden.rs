include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(binomial(0, 0), 1, "case 0");
    assert_eq!(binomial(0, 1), 0, "case 1");
    assert_eq!(binomial(1, 0), 1, "case 2");
    assert_eq!(binomial(1, 1), 1, "case 3");
    assert_eq!(binomial(1, 2), 0, "case 4");
    assert_eq!(binomial(5, 2), 10, "case 5");
    assert_eq!(binomial(5, 3), 10, "case 6");
    assert_eq!(binomial(10, 1), 10, "case 7");
    assert_eq!(binomial(10, 9), 10, "case 8");
    assert_eq!(binomial(20, 10), 184756, "case 9");
    assert_eq!(binomial(30, 15), 155117520, "case 10");
    assert_eq!(binomial(30, 0), 1, "case 11");
    assert_eq!(binomial(30, 30), 1, "case 12");
    assert_eq!(binomial(30, 35), 0, "case 13");
}
