include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string(), "".to_string()),
        "".to_string(),
        "case 0"
    );
    assert_eq!(
        solve("a".to_string(), "".to_string()),
        "".to_string(),
        "case 1"
    );
    assert_eq!(
        solve("".to_string(), "a".to_string()),
        "".to_string(),
        "case 2"
    );
    assert_eq!(
        solve("abc".to_string(), "abc".to_string()),
        "abc".to_string(),
        "case 3"
    );
    assert_eq!(
        solve("abc".to_string(), "abd".to_string()),
        "ab".to_string(),
        "case 4"
    );
    assert_eq!(
        solve("abc".to_string(), "ab".to_string()),
        "ab".to_string(),
        "case 5"
    );
    assert_eq!(
        solve("ab".to_string(), "abc".to_string()),
        "ab".to_string(),
        "case 6"
    );
    assert_eq!(
        solve("a".to_string(), "A".to_string()),
        "".to_string(),
        "case 7"
    );
    assert_eq!(
        solve("foo bar".to_string(), "foo baz".to_string()),
        "foo ba".to_string(),
        "case 8"
    );
    assert_eq!(
        solve(" x".to_string(), " y".to_string()),
        " ".to_string(),
        "case 9"
    );
    assert_eq!(
        solve("123".to_string(), "129".to_string()),
        "12".to_string(),
        "case 10"
    );
    assert_eq!(
        solve("abc".to_string(), "zabc".to_string()),
        "".to_string(),
        "case 11"
    );
}
