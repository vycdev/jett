include!("solution.rs");

#[test]
fn hidden_first_duplicate() {
    assert_eq!(first_duplicate(&[]), None);
    assert_eq!(first_duplicate(&[7]), None);
    assert_eq!(first_duplicate(&[2, 1, 3, 1, 2]), Some(1));
    assert_eq!(first_duplicate(&[4, 4, 5, 5]), Some(4));
    assert_eq!(first_duplicate(&[-2, 0, -2]), Some(-2));
    assert_eq!(first_duplicate(&[0, 1, 0, 1]), Some(0));
}
