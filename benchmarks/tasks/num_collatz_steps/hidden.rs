include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(collatz_steps(1, 0), Some(0), "case 0");
    assert_eq!(collatz_steps(2, 0), None, "case 1");
    assert_eq!(collatz_steps(2, 1), Some(1), "case 2");
    assert_eq!(collatz_steps(3, 6), None, "case 3");
    assert_eq!(collatz_steps(3, 7), Some(7), "case 4");
    assert_eq!(collatz_steps(6, 8), Some(8), "case 5");
    assert_eq!(collatz_steps(7, 15), None, "case 6");
    assert_eq!(collatz_steps(7, 16), Some(16), "case 7");
    assert_eq!(collatz_steps(27, 110), None, "case 8");
    assert_eq!(collatz_steps(27, 111), Some(111), "case 9");
    assert_eq!(collatz_steps(1000000, 200), Some(152), "case 10");
    assert_eq!(collatz_steps(999999, 200), None, "case 11");
    assert_eq!(collatz_steps(1024, 9), None, "case 12");
    assert_eq!(collatz_steps(1024, 10), Some(10), "case 13");
}
