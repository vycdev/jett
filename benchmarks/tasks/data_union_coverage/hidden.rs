include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            covered: 0,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 5
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 1
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[Entry { start: 11, end: 15 }], 2),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 2
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[Entry { start: 3, end: 8 }, Entry { start: 6, end: 12 }],
            3
        ),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 3
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 10, end: 13 },
                Entry { start: 0, end: 2 },
                Entry { start: 1, end: 6 }
            ],
            4
        ),
        Report {
            covered: 4,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 3, end: 6 },
                Entry { start: 5, end: 10 },
                Entry { start: 7, end: 8 },
                Entry { start: 6, end: 13 }
            ],
            5
        ),
        Report {
            covered: 2,
            gaps: 1,
            longest_gap: 3
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 0, end: 5 },
                Entry { start: 9, end: 12 },
                Entry { start: 6, end: 7 },
                Entry { start: 4, end: 5 },
                Entry { start: 8, end: 15 }
            ],
            6
        ),
        Report {
            covered: 5,
            gaps: 1,
            longest_gap: 1
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 1, end: 8 },
                Entry { start: 7, end: 13 },
                Entry { start: 5, end: 9 },
                Entry { start: 10, end: 14 },
                Entry { start: 9, end: 11 },
                Entry { start: 0, end: 2 }
            ],
            7
        ),
        Report {
            covered: 7,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 1, end: 5 },
                Entry { start: 0, end: 1 },
                Entry { start: 6, end: 12 },
                Entry { start: 11, end: 12 },
                Entry { start: 7, end: 14 },
                Entry { start: 5, end: 12 },
                Entry { start: 0, end: 3 }
            ],
            8
        ),
        Report {
            covered: 8,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 10, end: 14 },
                Entry { start: 10, end: 17 },
                Entry { start: 7, end: 8 },
                Entry { start: 9, end: 11 },
                Entry { start: 11, end: 12 },
                Entry { start: 9, end: 12 },
                Entry { start: 2, end: 3 },
                Entry { start: 3, end: 9 }
            ],
            1
        ),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 1
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 2, end: 9 },
                Entry { start: 5, end: 7 },
                Entry { start: 4, end: 6 },
                Entry { start: 4, end: 11 },
                Entry { start: 5, end: 9 },
                Entry { start: 10, end: 11 },
                Entry { start: 0, end: 4 },
                Entry { start: 6, end: 11 },
                Entry { start: 3, end: 4 }
            ],
            2
        ),
        Report {
            covered: 2,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 7, end: 8 },
                Entry { start: 2, end: 7 },
                Entry { start: 1, end: 7 },
                Entry { start: 11, end: 14 },
                Entry { start: 9, end: 12 },
                Entry { start: 5, end: 8 },
                Entry { start: 8, end: 11 },
                Entry { start: 2, end: 4 },
                Entry { start: 4, end: 6 },
                Entry { start: 8, end: 11 }
            ],
            3
        ),
        Report {
            covered: 2,
            gaps: 1,
            longest_gap: 1
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 6, end: 12 },
                Entry { start: 0, end: 2 },
                Entry { start: 7, end: 9 },
                Entry { start: 4, end: 5 },
                Entry { start: 6, end: 8 },
                Entry { start: 8, end: 14 },
                Entry { start: 9, end: 14 },
                Entry { start: 9, end: 14 },
                Entry { start: 3, end: 7 },
                Entry { start: 2, end: 8 },
                Entry { start: 7, end: 11 }
            ],
            4
        ),
        Report {
            covered: 4,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 9, end: 12 },
                Entry { start: 5, end: 12 },
                Entry { start: 3, end: 8 },
                Entry { start: 5, end: 7 },
                Entry { start: 4, end: 6 },
                Entry { start: 9, end: 10 },
                Entry { start: 1, end: 8 },
                Entry { start: 11, end: 15 },
                Entry { start: 6, end: 10 },
                Entry { start: 8, end: 9 },
                Entry { start: 1, end: 6 },
                Entry { start: 6, end: 12 }
            ],
            5
        ),
        Report {
            covered: 4,
            gaps: 1,
            longest_gap: 1
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 6
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[Entry { start: 1, end: 3 }], 7),
        Report {
            covered: 2,
            gaps: 2,
            longest_gap: 4
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[Entry { start: 0, end: 4 }, Entry { start: 2, end: 5 }], 8),
        Report {
            covered: 5,
            gaps: 1,
            longest_gap: 3
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 8, end: 10 },
                Entry { start: 7, end: 10 },
                Entry { start: 7, end: 9 }
            ],
            1
        ),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 1
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 4, end: 9 },
                Entry { start: 8, end: 12 },
                Entry { start: 0, end: 3 },
                Entry { start: 8, end: 15 }
            ],
            2
        ),
        Report {
            covered: 2,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 10, end: 12 },
                Entry { start: 10, end: 11 },
                Entry { start: 6, end: 9 },
                Entry { start: 10, end: 14 },
                Entry { start: 8, end: 9 }
            ],
            3
        ),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 3
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 5, end: 9 },
                Entry { start: 3, end: 7 },
                Entry { start: 4, end: 10 },
                Entry { start: 9, end: 16 },
                Entry { start: 10, end: 12 },
                Entry { start: 9, end: 16 }
            ],
            4
        ),
        Report {
            covered: 1,
            gaps: 1,
            longest_gap: 3
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 3, end: 8 },
                Entry { start: 3, end: 5 },
                Entry { start: 0, end: 6 },
                Entry { start: 2, end: 9 },
                Entry { start: 8, end: 13 },
                Entry { start: 8, end: 10 },
                Entry { start: 10, end: 11 }
            ],
            5
        ),
        Report {
            covered: 5,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 7, end: 14 },
                Entry { start: 8, end: 15 },
                Entry { start: 2, end: 9 },
                Entry { start: 10, end: 11 },
                Entry { start: 3, end: 5 },
                Entry { start: 5, end: 7 },
                Entry { start: 10, end: 12 },
                Entry { start: 4, end: 9 }
            ],
            6
        ),
        Report {
            covered: 4,
            gaps: 1,
            longest_gap: 2
        },
        "fixture 23"
    );
    assert_eq!(
        solve(&[Entry { start: 0, end: 3 }, Entry { start: 3, end: 5 }], 5),
        Report {
            covered: 5,
            gaps: 0,
            longest_gap: 0
        },
        "fixture 24"
    );
    assert_eq!(
        solve(&[Entry { start: 9, end: 12 }], 5),
        Report {
            covered: 0,
            gaps: 1,
            longest_gap: 5
        },
        "fixture 25"
    );
    assert_eq!(
        solve(&[Entry { start: 0, end: 1 }, Entry { start: 4, end: 5 }], 5),
        Report {
            covered: 2,
            gaps: 1,
            longest_gap: 3
        },
        "fixture 26"
    );
    assert_eq!(
        solve(
            &[
                Entry { start: 1, end: 3 },
                Entry { start: 2, end: 4 },
                Entry { start: 6, end: 9 }
            ],
            8
        ),
        Report {
            covered: 5,
            gaps: 2,
            longest_gap: 2
        },
        "fixture 27"
    );
}
