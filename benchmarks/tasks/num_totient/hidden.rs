include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(totient(1), 1, "case 0");
    assert_eq!(totient(2), 1, "case 1");
    assert_eq!(totient(3), 2, "case 2");
    assert_eq!(totient(4), 2, "case 3");
    assert_eq!(totient(5), 4, "case 4");
    assert_eq!(totient(6), 2, "case 5");
    assert_eq!(totient(8), 4, "case 6");
    assert_eq!(totient(9), 6, "case 7");
    assert_eq!(totient(10), 4, "case 8");
    assert_eq!(totient(12), 4, "case 9");
    assert_eq!(totient(30), 8, "case 10");
    assert_eq!(totient(36), 12, "case 11");
    assert_eq!(totient(49), 42, "case 12");
    assert_eq!(totient(97), 96, "case 13");
    assert_eq!(totient(210), 48, "case 14");
    assert_eq!(totient(1024), 512, "case 15");
    assert_eq!(totient(9999), 6000, "case 16");
    assert_eq!(totient(10000), 4000, "case 17");
}
