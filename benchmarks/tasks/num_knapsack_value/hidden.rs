include!("solution.rs");

#[test]
fn hidden_cases() {

    assert_eq!(knapsack_value(&[], &[], 0), 0, "case 0");
    assert_eq!(knapsack_value(&[], &[], 8), 0, "case 1");
    assert_eq!(knapsack_value(&[1], &[7], 0), 0, "case 2");
    assert_eq!(knapsack_value(&[5], &[10], 4), 0, "case 3");
    assert_eq!(knapsack_value(&[5], &[10], 5), 10, "case 4");
    assert_eq!(knapsack_value(&[2, 3, 4], &[4, 5, 7], 5), 9, "case 5");
    assert_eq!(knapsack_value(&[2, 2, 2], &[3, 3, 3], 4), 6, "case 6");
    assert_eq!(knapsack_value(&[1, 1, 1], &[0, 5, 7], 2), 12, "case 7");
    assert_eq!(knapsack_value(&[6, 3, 4, 2], &[30, 14, 16, 9], 10), 46, "case 8");
    assert_eq!(knapsack_value(&[10, 20, 30], &[60, 100, 100], 40), 160, "case 9");
    assert_eq!(knapsack_value(&[1, 3, 4], &[1, 4, 5], 7), 9, "case 10");
    assert_eq!(knapsack_value(&[40], &[100], 40), 100, "case 11");
    assert_eq!(knapsack_value(&[7, 6, 5, 4, 3, 2], &[5, 6, 7, 8, 9, 10], 12), 27, "case 12");
}
