include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), false, "case 0");
    assert_eq!(solve("_".to_string()), true, "case 1");
    assert_eq!(solve("a".to_string()), true, "case 2");
    assert_eq!(solve("A0_z".to_string()), true, "case 3");
    assert_eq!(solve("0a".to_string()), false, "case 4");
    assert_eq!(solve("a-b".to_string()), false, "case 5");
    assert_eq!(solve("a b".to_string()), false, "case 6");
    assert_eq!(solve("if".to_string()), true, "case 7");
    assert_eq!(solve("__".to_string()), true, "case 8");
    assert_eq!(solve("a\n".to_string()), false, "case 9");
    assert_eq!(solve("$a".to_string()), false, "case 10");
    assert_eq!(solve("z9".to_string()), true, "case 11");
}
