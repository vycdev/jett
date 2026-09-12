include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string(), "".to_string()), 1, "case 0");
    assert_eq!(solve("".to_string(), "a".to_string()), 0, "case 1");
    assert_eq!(solve("abc".to_string(), "".to_string()), 4, "case 2");
    assert_eq!(solve("aaaa".to_string(), "aa".to_string()), 3, "case 3");
    assert_eq!(solve("ababa".to_string(), "aba".to_string()), 2, "case 4");
    assert_eq!(solve("abc".to_string(), "abc".to_string()), 1, "case 5");
    assert_eq!(solve("ab".to_string(), "abc".to_string()), 0, "case 6");
    assert_eq!(solve("AaA".to_string(), "a".to_string()), 1, "case 7");
    assert_eq!(solve("xxxxx".to_string(), "xx".to_string()), 4, "case 8");
    assert_eq!(solve("one one".to_string(), "one".to_string()), 2, "case 9");
    assert_eq!(solve(" ".to_string(), " ".to_string()), 1, "case 10");
    assert_eq!(solve("abc".to_string(), "z".to_string()), 0, "case 11");
}
