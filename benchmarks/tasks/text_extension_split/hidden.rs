include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("".to_string()),
        TextPair {
            first: "".to_string(),
            second: "".to_string()
        },
        "case 0"
    );
    assert_eq!(
        solve("a".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "".to_string()
        },
        "case 1"
    );
    assert_eq!(
        solve("a.txt".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "txt".to_string()
        },
        "case 2"
    );
    assert_eq!(
        solve("a.tar.gz".to_string()),
        TextPair {
            first: "a.tar".to_string(),
            second: "gz".to_string()
        },
        "case 3"
    );
    assert_eq!(
        solve(".env".to_string()),
        TextPair {
            first: ".env".to_string(),
            second: "".to_string()
        },
        "case 4"
    );
    assert_eq!(
        solve(".env.local".to_string()),
        TextPair {
            first: ".env".to_string(),
            second: "local".to_string()
        },
        "case 5"
    );
    assert_eq!(
        solve("a.".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "".to_string()
        },
        "case 6"
    );
    assert_eq!(
        solve(".".to_string()),
        TextPair {
            first: ".".to_string(),
            second: "".to_string()
        },
        "case 7"
    );
    assert_eq!(
        solve("..".to_string()),
        TextPair {
            first: ".".to_string(),
            second: "".to_string()
        },
        "case 8"
    );
    assert_eq!(
        solve("...".to_string()),
        TextPair {
            first: "..".to_string(),
            second: "".to_string()
        },
        "case 9"
    );
    assert_eq!(
        solve("A.TXT".to_string()),
        TextPair {
            first: "A".to_string(),
            second: "TXT".to_string()
        },
        "case 10"
    );
    assert_eq!(
        solve(" a .x".to_string()),
        TextPair {
            first: " a ".to_string(),
            second: "x".to_string()
        },
        "case 11"
    );
}
