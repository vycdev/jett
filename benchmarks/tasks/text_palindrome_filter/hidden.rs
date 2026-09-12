include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), true, "case 0");
    assert_eq!(solve("! ?".to_string()), true, "case 1");
    assert_eq!(solve("a".to_string()), true, "case 2");
    assert_eq!(solve("ab".to_string()), false, "case 3");
    assert_eq!(solve("Aa".to_string()), true, "case 4");
    assert_eq!(solve("Race car!".to_string()), true, "case 5");
    assert_eq!(
        solve("A man, a plan, a canal: Panama".to_string()),
        true,
        "case 6"
    );
    assert_eq!(solve("12 21".to_string()), true, "case 7");
    assert_eq!(solve("12a21".to_string()), true, "case 8");
    assert_eq!(solve("12a22".to_string()), false, "case 9");
    assert_eq!(solve("0P".to_string()), false, "case 10");
    assert_eq!(solve("ab\tBA".to_string()), true, "case 11");
}
