include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(pair_sum_count(&[], 0), 0, "case 0");
    assert_eq!(pair_sum_count(&[0], 0), 0, "case 1");
    assert_eq!(pair_sum_count(&[0, 0], 0), 1, "case 2");
    assert_eq!(pair_sum_count(&[0, 0, 0, 0], 0), 6, "case 3");
    assert_eq!(pair_sum_count(&[1, 2, 3, 4], 5), 2, "case 4");
    assert_eq!(pair_sum_count(&[1, 1, 1, 2, 2], 3), 6, "case 5");
    assert_eq!(pair_sum_count(&[-2, -1, 0, 1, 2], 0), 2, "case 6");
    assert_eq!(pair_sum_count(&[5, -5, 5, -5], 0), 4, "case 7");
    assert_eq!(pair_sum_count(&[1, 2, 3], 10), 0, "case 8");
    assert_eq!(pair_sum_count(&[1000, 1000], 2000), 1, "case 9");
    assert_eq!(pair_sum_count(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1], 2), 4950, "case 10");
    assert_eq!(pair_sum_count(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 19), 10, "case 11");
    assert_eq!(pair_sum_count(&[2, 2, 2], 4), 3, "case 12");
}
