include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(coin_change(&[], 0), 0, "case 0");
    assert_eq!(coin_change(&[], 1), -1, "case 1");
    assert_eq!(coin_change(&[1], 0), 0, "case 2");
    assert_eq!(coin_change(&[1], 7), 7, "case 3");
    assert_eq!(coin_change(&[2], 3), -1, "case 4");
    assert_eq!(coin_change(&[2], 8), 4, "case 5");
    assert_eq!(coin_change(&[1, 3, 4], 6), 2, "case 6");
    assert_eq!(coin_change(&[5, 2], 11), 4, "case 7");
    assert_eq!(coin_change(&[3, 7], 10), 2, "case 8");
    assert_eq!(coin_change(&[3, 7], 5), -1, "case 9");
    assert_eq!(coin_change(&[2, 2, 4], 8), 2, "case 10");
    assert_eq!(coin_change(&[50], 100), 2, "case 11");
    assert_eq!(coin_change(&[7, 10, 25], 99), 6, "case 12");
    assert_eq!(coin_change(&[9, 6, 5, 1], 11), 2, "case 13");
}
