include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), true, "case 0");
    assert_eq!(solve("abc".to_string()), true, "case 1");
    assert_eq!(solve("()[]{}".to_string()), true, "case 2");
    assert_eq!(solve("([{}])".to_string()), true, "case 3");
    assert_eq!(solve("([)]".to_string()), false, "case 4");
    assert_eq!(solve("(".to_string()), false, "case 5");
    assert_eq!(solve(")".to_string()), false, "case 6");
    assert_eq!(solve("a[b(c)d]e".to_string()), true, "case 7");
    assert_eq!(solve("{{}}".to_string()), true, "case 8");
    assert_eq!(solve("{]".to_string()), false, "case 9");
    assert_eq!(solve("\"(\"".to_string()), false, "case 10");
    assert_eq!(solve("(()())".to_string()), true, "case 11");
}
