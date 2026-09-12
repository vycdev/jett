include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(rotate_left(&[], 0), vec![], "case 0");
    assert_eq!(rotate_left(&[], 100), vec![], "case 1");
    assert_eq!(rotate_left(&[7], 0), vec![7], "case 2");
    assert_eq!(rotate_left(&[7], 999), vec![7], "case 3");
    assert_eq!(rotate_left(&[1, 2, 3], 0), vec![1, 2, 3], "case 4");
    assert_eq!(rotate_left(&[1, 2, 3], 1), vec![2, 3, 1], "case 5");
    assert_eq!(rotate_left(&[1, 2, 3], 2), vec![3, 1, 2], "case 6");
    assert_eq!(rotate_left(&[1, 2, 3], 3), vec![1, 2, 3], "case 7");
    assert_eq!(rotate_left(&[1, 2, 3], 4), vec![2, 3, 1], "case 8");
    assert_eq!(rotate_left(&[-1, 0, 1, 0], 7), vec![0, -1, 0, 1], "case 9");
    assert_eq!(rotate_left(&[2, 2, 3, 2], 2), vec![3, 2, 2, 2], "case 10");
    assert_eq!(rotate_left(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 1000000), vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], "case 11");
    assert_eq!(rotate_left(&[0, 1, 2, 3, 4, 5, 6], 999999), vec![0, 1, 2, 3, 4, 5, 6], "case 12");
}
