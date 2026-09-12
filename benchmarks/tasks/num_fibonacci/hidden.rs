include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(fibonacci(0), 0, "case 0");
    assert_eq!(fibonacci(1), 1, "case 1");
    assert_eq!(fibonacci(2), 1, "case 2");
    assert_eq!(fibonacci(3), 2, "case 3");
    assert_eq!(fibonacci(4), 3, "case 4");
    assert_eq!(fibonacci(5), 5, "case 5");
    assert_eq!(fibonacci(8), 21, "case 6");
    assert_eq!(fibonacci(10), 55, "case 7");
    assert_eq!(fibonacci(20), 6765, "case 8");
    assert_eq!(fibonacci(30), 832040, "case 9");
    assert_eq!(fibonacci(40), 102334155, "case 10");
    assert_eq!(fibonacci(50), 12586269025, "case 11");
    assert_eq!(fibonacci(60), 1548008755920, "case 12");
    assert_eq!(fibonacci(70), 190392490709135, "case 13");
}
