include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(solve("".to_string()), "".to_string(), "case 0");
    assert_eq!(solve(" ".to_string()), " ".to_string(), "case 1");
    assert_eq!(solve("HELLO".to_string()), "Hello".to_string(), "case 2");
    assert_eq!(
        solve("hello WORLD".to_string()),
        "Hello World".to_string(),
        "case 3"
    );
    assert_eq!(
        solve("  a  B ".to_string()),
        "  A  B ".to_string(),
        "case 4"
    );
    assert_eq!(solve("a-b".to_string()), "A-b".to_string(), "case 5");
    assert_eq!(solve("1ABC".to_string()), "1abc".to_string(), "case 6");
    assert_eq!(solve("!HELLO".to_string()), "!hello".to_string(), "case 7");
    assert_eq!(solve("x\tY".to_string()), "X\ty".to_string(), "case 8");
    assert_eq!(solve("a b c".to_string()), "A B C".to_string(), "case 9");
    assert_eq!(solve("MiXeD".to_string()), "Mixed".to_string(), "case 10");
    assert_eq!(
        solve("A  B  C".to_string()),
        "A  B  C".to_string(),
        "case 11"
    );
}
