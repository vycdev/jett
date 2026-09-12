include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve("abc".to_string()), "abc".to_string(), "case 1");
    assert_eq!(solve("#x".to_string()), "".to_string(), "case 2");
    assert_eq!(solve("a#b".to_string()), "a".to_string(), "case 3");
    assert_eq!(solve("a#b\nc".to_string()), "a\nc".to_string(), "case 4");
    assert_eq!(
        solve("\"#x\"#y".to_string()),
        "\"#x\"".to_string(),
        "case 5"
    );
    assert_eq!(solve("#a\n#b\n".to_string()), "\n\n".to_string(), "case 6");
    assert_eq!(solve("a # x".to_string()), "a ".to_string(), "case 7");
    assert_eq!(
        solve("\"a\\\"#b\"#c".to_string()),
        "\"a\\\"#b\"".to_string(),
        "case 8"
    );
    assert_eq!(solve("\\#x".to_string()), "\\".to_string(), "case 9");
    assert_eq!(
        solve("\"#open".to_string()),
        "\"#open".to_string(),
        "case 10"
    );
    assert_eq!(
        solve("a\n#x\nb".to_string()),
        "a\n\nb".to_string(),
        "case 11"
    );
}
