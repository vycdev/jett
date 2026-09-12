include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(digit_checksum(0), 0, "case 0");
    assert_eq!(digit_checksum(1), 1, "case 1");
    assert_eq!(digit_checksum(9), 9, "case 2");
    assert_eq!(digit_checksum(10), -1, "case 3");
    assert_eq!(digit_checksum(11), 0, "case 4");
    assert_eq!(digit_checksum(12), 1, "case 5");
    assert_eq!(digit_checksum(123), 2, "case 6");
    assert_eq!(digit_checksum(1234), 2, "case 7");
    assert_eq!(digit_checksum(90909), 27, "case 8");
    assert_eq!(digit_checksum(100001), 0, "case 9");
    assert_eq!(digit_checksum(987654321), 5, "case 10");
    assert_eq!(digit_checksum(1000000000000), 1, "case 11");
    assert_eq!(digit_checksum(999999999999), 0, "case 12");
    assert_eq!(digit_checksum(10101010101), 6, "case 13");
}
