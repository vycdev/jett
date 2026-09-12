include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(sorted_insert(&[], 3), vec![3], "case 0");
    assert_eq!(sorted_insert(&[1], 0), vec![0, 1], "case 1");
    assert_eq!(sorted_insert(&[1], 1), vec![1, 1], "case 2");
    assert_eq!(sorted_insert(&[1], 2), vec![1, 2], "case 3");
    assert_eq!(sorted_insert(&[1, 2, 3], 0), vec![0, 1, 2, 3], "case 4");
    assert_eq!(sorted_insert(&[1, 2, 3], 2), vec![1, 2, 2, 3], "case 5");
    assert_eq!(sorted_insert(&[1, 2, 3], 4), vec![1, 2, 3, 4], "case 6");
    assert_eq!(sorted_insert(&[0, 0, 0], 0), vec![0, 0, 0, 0], "case 7");
    assert_eq!(sorted_insert(&[-5, -2, 0, 3], -3), vec![-5, -3, -2, 0, 3], "case 8");
    assert_eq!(sorted_insert(&[-5, -2, 0, 3], 0), vec![-5, -2, 0, 0, 3], "case 9");
    assert_eq!(sorted_insert(&[1, 1, 2, 2], 1), vec![1, 1, 1, 2, 2], "case 10");
    assert_eq!(sorted_insert(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 10), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], "case 11");
    assert_eq!(sorted_insert(&[1000], -1000), vec![-1000, 1000], "case 12");
}
