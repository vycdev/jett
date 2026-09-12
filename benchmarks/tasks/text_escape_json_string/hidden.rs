include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "\"\"".to_string(), "case 0");
    assert_eq!(solve("a".to_string()), "\"a\"".to_string(), "case 1");
    assert_eq!(solve("\"".to_string()), "\"\\\"\"".to_string(), "case 2");
    assert_eq!(solve("\\".to_string()), "\"\\\\\"".to_string(), "case 3");
    assert_eq!(solve("\n".to_string()), "\"\\n\"".to_string(), "case 4");
    assert_eq!(solve("\t".to_string()), "\"\\t\"".to_string(), "case 5");
    assert_eq!(solve("\r".to_string()), "\"\\r\"".to_string(), "case 6");
    assert_eq!(
        solve("\u{0008}".to_string()),
        "\"\\u{0008}\"".to_string(),
        "case 7"
    );
    assert_eq!(
        solve("\u{000c}".to_string()),
        "\"\\u{000c}\"".to_string(),
        "case 8"
    );
    assert_eq!(solve("/".to_string()), "\"/\"".to_string(), "case 9");
    assert_eq!(
        solve("a\"b\\c".to_string()),
        "\"a\\\"b\\\\c\"".to_string(),
        "case 10"
    );
    assert_eq!(
        solve("x\ny\tz".to_string()),
        "\"x\\ny\\tz\"".to_string(),
        "case 11"
    );
}
