include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("1".to_string(), "1".to_string()), 0, "case 0");
    assert_eq!(solve("1".to_string(), "1.0.0".to_string()), 0, "case 1");
    assert_eq!(solve("0.0".to_string(), "0".to_string()), 0, "case 2");
    assert_eq!(solve("1.2".to_string(), "1.10".to_string()), -1, "case 3");
    assert_eq!(solve("2".to_string(), "1.999".to_string()), 1, "case 4");
    assert_eq!(solve("1.0.1".to_string(), "1".to_string()), 1, "case 5");
    assert_eq!(solve("1".to_string(), "1.0.1".to_string()), -1, "case 6");
    assert_eq!(solve("999".to_string(), "998.999".to_string()), 1, "case 7");
    assert_eq!(solve("0.1".to_string(), "0.0.9".to_string()), 1, "case 8");
    assert_eq!(solve("1.2.3".to_string(), "1.2.3".to_string()), 0, "case 9");
    assert_eq!(
        solve("1.2.3".to_string(), "1.2.4".to_string()),
        -1,
        "case 10"
    );
    assert_eq!(
        solve("1.0.0.0.0.0.0.0".to_string(), "1".to_string()),
        0,
        "case 11"
    );
}
