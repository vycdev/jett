include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(integer_sqrt(0), 0, "case 0");
    assert_eq!(integer_sqrt(1), 1, "case 1");
    assert_eq!(integer_sqrt(2), 1, "case 2");
    assert_eq!(integer_sqrt(3), 1, "case 3");
    assert_eq!(integer_sqrt(4), 2, "case 4");
    assert_eq!(integer_sqrt(8), 2, "case 5");
    assert_eq!(integer_sqrt(9), 3, "case 6");
    assert_eq!(integer_sqrt(10), 3, "case 7");
    assert_eq!(integer_sqrt(15), 3, "case 8");
    assert_eq!(integer_sqrt(16), 4, "case 9");
    assert_eq!(integer_sqrt(17), 4, "case 10");
    assert_eq!(integer_sqrt(99980000), 9998, "case 11");
    assert_eq!(integer_sqrt(99980001), 9999, "case 12");
    assert_eq!(integer_sqrt(99980002), 9999, "case 13");
    assert_eq!(integer_sqrt(999999999999), 999999, "case 14");
    assert_eq!(integer_sqrt(1000000000000), 1000000, "case 15");
}
