include!("solution.rs");
#[test]
fn hidden_cases() {
    assert_eq!(
        solve("a=b".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "b".to_string()
        },
        "case 0"
    );
    assert_eq!(
        solve(" a = b ".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "b".to_string()
        },
        "case 1"
    );
    assert_eq!(
        solve("a=".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "".to_string()
        },
        "case 2"
    );
    assert_eq!(
        solve("a=  ".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "".to_string()
        },
        "case 3"
    );
    assert_eq!(
        solve("a=b=c".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "b=c".to_string()
        },
        "case 4"
    );
    assert_eq!(
        solve("a==".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "=".to_string()
        },
        "case 5"
    );
    assert_eq!(
        solve("x y=z z".to_string()),
        TextPair {
            first: "x y".to_string(),
            second: "z z".to_string()
        },
        "case 6"
    );
    assert_eq!(
        solve("\ta\t=\tb\t".to_string()),
        TextPair {
            first: "\ta\t".to_string(),
            second: "\tb\t".to_string()
        },
        "case 7"
    );
    assert_eq!(
        solve("x= a  b ".to_string()),
        TextPair {
            first: "x".to_string(),
            second: "a  b".to_string()
        },
        "case 8"
    );
    assert_eq!(
        solve("KEY=value".to_string()),
        TextPair {
            first: "KEY".to_string(),
            second: "value".to_string()
        },
        "case 9"
    );
    assert_eq!(
        solve("a= = ".to_string()),
        TextPair {
            first: "a".to_string(),
            second: "=".to_string()
        },
        "case 10"
    );
    assert_eq!(
        solve("  k  = v=x ".to_string()),
        TextPair {
            first: "k".to_string(),
            second: "v=x".to_string()
        },
        "case 11"
    );
}
