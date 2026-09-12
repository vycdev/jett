include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(window_peak(&[], 0), None, "case 0");
    assert_eq!(window_peak(&[], 1), None, "case 1");
    assert_eq!(window_peak(&[7], 0), None, "case 2");
    assert_eq!(window_peak(&[7], 1), Some(7), "case 3");
    assert_eq!(window_peak(&[7], 2), None, "case 4");
    assert_eq!(window_peak(&[1, 2, 3, 4], 2), Some(7), "case 5");
    assert_eq!(window_peak(&[-5, -2, -7], 2), Some(-7), "case 6");
    assert_eq!(window_peak(&[2, -1, 2, -1, 2], 3), Some(3), "case 7");
    assert_eq!(window_peak(&[5, -9, 5], 1), Some(5), "case 8");
    assert_eq!(window_peak(&[5, -9, 5], 3), Some(1), "case 9");
    assert_eq!(window_peak(&[0, 0, 0], 2), Some(0), "case 10");
    assert_eq!(window_peak(&[10, -5, -5, 10], 2), Some(5), "case 11");
    assert_eq!(window_peak(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19], 5), Some(85), "case 12");
    assert_eq!(window_peak(&[20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0], 7), Some(119), "case 13");
}
