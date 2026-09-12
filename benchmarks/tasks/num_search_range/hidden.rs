include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(search_range(&[], 0), SearchRange {first: -1, last: -1}, "case 0");
    assert_eq!(search_range(&[1], 1), SearchRange {first: 0, last: 0}, "case 1");
    assert_eq!(search_range(&[1], 0), SearchRange {first: -1, last: -1}, "case 2");
    assert_eq!(search_range(&[1], 2), SearchRange {first: -1, last: -1}, "case 3");
    assert_eq!(search_range(&[1, 2, 2, 2, 3], 2), SearchRange {first: 1, last: 3}, "case 4");
    assert_eq!(search_range(&[1, 1, 2], 1), SearchRange {first: 0, last: 1}, "case 5");
    assert_eq!(search_range(&[1, 2, 2], 2), SearchRange {first: 1, last: 2}, "case 6");
    assert_eq!(search_range(&[0, 0, 0], 0), SearchRange {first: 0, last: 2}, "case 7");
    assert_eq!(search_range(&[-5, -2, -2, 0, 3], -2), SearchRange {first: 1, last: 2}, "case 8");
    assert_eq!(search_range(&[-5, -2, 0, 3], 1), SearchRange {first: -1, last: -1}, "case 9");
    assert_eq!(search_range(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 10), SearchRange {first: 10, last: 10}, "case 10");
    assert_eq!(search_range(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 20), SearchRange {first: -1, last: -1}, "case 11");
    assert_eq!(search_range(&[7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], 7), SearchRange {first: 0, last: 99}, "case 12");
}
