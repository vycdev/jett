include!("solution.rs");

fn interval(start: i64, end: i64) -> Interval {
    Interval { start, end }
}

#[test]
fn hidden_merge_sorted_intervals() {
    assert_eq!(merge_sorted_intervals(vec![]), vec![]);
    assert_eq!(
        merge_sorted_intervals(vec![interval(1, 3)]),
        vec![interval(1, 3)]
    );
    assert_eq!(
        merge_sorted_intervals(vec![interval(1, 3), interval(2, 6), interval(8, 10)]),
        vec![interval(1, 6), interval(8, 10)]
    );
    assert_eq!(
        merge_sorted_intervals(vec![interval(1, 10), interval(2, 3), interval(10, 12)]),
        vec![interval(1, 12)]
    );
    assert_eq!(
        merge_sorted_intervals(vec![interval(-5, -2), interval(-2, 0), interval(4, 4)]),
        vec![interval(-5, 0), interval(4, 4)]
    );
}
