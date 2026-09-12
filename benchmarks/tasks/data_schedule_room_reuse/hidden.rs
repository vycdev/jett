include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            rooms: 0,
            assignment_checksum: 0,
            reuses: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry { start: 11, end: 15 }]),
        Report {
            rooms: 1,
            assignment_checksum: 1,
            reuses: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[Entry { start: 3, end: 8 }, Entry { start: 6, end: 12 }]),
        Report {
            rooms: 2,
            assignment_checksum: 5,
            reuses: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 2 },
            Entry { start: 1, end: 6 },
            Entry { start: 10, end: 13 }
        ]),
        Report {
            rooms: 2,
            assignment_checksum: 8,
            reuses: 1
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry { start: 3, end: 6 },
            Entry { start: 5, end: 10 },
            Entry { start: 6, end: 13 },
            Entry { start: 7, end: 8 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 20,
            reuses: 1
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 5 },
            Entry { start: 4, end: 5 },
            Entry { start: 6, end: 7 },
            Entry { start: 8, end: 15 },
            Entry { start: 9, end: 12 }
        ]),
        Report {
            rooms: 2,
            assignment_checksum: 22,
            reuses: 3
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 2 },
            Entry { start: 1, end: 8 },
            Entry { start: 5, end: 9 },
            Entry { start: 7, end: 13 },
            Entry { start: 9, end: 11 },
            Entry { start: 10, end: 14 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 37,
            reuses: 3
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 1 },
            Entry { start: 0, end: 3 },
            Entry { start: 1, end: 5 },
            Entry { start: 5, end: 12 },
            Entry { start: 6, end: 12 },
            Entry { start: 7, end: 14 },
            Entry { start: 11, end: 12 }
        ]),
        Report {
            rooms: 4,
            assignment_checksum: 68,
            reuses: 3
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry { start: 2, end: 3 },
            Entry { start: 3, end: 9 },
            Entry { start: 7, end: 8 },
            Entry { start: 9, end: 11 },
            Entry { start: 9, end: 12 },
            Entry { start: 10, end: 14 },
            Entry { start: 10, end: 17 },
            Entry { start: 11, end: 12 }
        ]),
        Report {
            rooms: 4,
            assignment_checksum: 77,
            reuses: 4
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 4 },
            Entry { start: 2, end: 9 },
            Entry { start: 3, end: 4 },
            Entry { start: 4, end: 6 },
            Entry { start: 4, end: 11 },
            Entry { start: 5, end: 7 },
            Entry { start: 5, end: 9 },
            Entry { start: 6, end: 11 },
            Entry { start: 10, end: 11 }
        ]),
        Report {
            rooms: 5,
            assignment_checksum: 118,
            reuses: 4
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry { start: 1, end: 7 },
            Entry { start: 2, end: 7 },
            Entry { start: 2, end: 4 },
            Entry { start: 4, end: 6 },
            Entry { start: 5, end: 8 },
            Entry { start: 7, end: 8 },
            Entry { start: 8, end: 11 },
            Entry { start: 8, end: 11 },
            Entry { start: 9, end: 12 },
            Entry { start: 11, end: 14 }
        ]),
        Report {
            rooms: 4,
            assignment_checksum: 112,
            reuses: 6
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 2 },
            Entry { start: 2, end: 8 },
            Entry { start: 3, end: 7 },
            Entry { start: 4, end: 5 },
            Entry { start: 6, end: 12 },
            Entry { start: 6, end: 8 },
            Entry { start: 7, end: 9 },
            Entry { start: 7, end: 11 },
            Entry { start: 8, end: 14 },
            Entry { start: 9, end: 14 },
            Entry { start: 9, end: 14 }
        ]),
        Report {
            rooms: 5,
            assignment_checksum: 187,
            reuses: 6
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry { start: 1, end: 8 },
            Entry { start: 1, end: 6 },
            Entry { start: 3, end: 8 },
            Entry { start: 4, end: 6 },
            Entry { start: 5, end: 12 },
            Entry { start: 5, end: 7 },
            Entry { start: 6, end: 10 },
            Entry { start: 6, end: 12 },
            Entry { start: 8, end: 9 },
            Entry { start: 9, end: 12 },
            Entry { start: 9, end: 10 },
            Entry { start: 11, end: 15 }
        ]),
        Report {
            rooms: 6,
            assignment_checksum: 213,
            reuses: 6
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry { start: 1, end: 3 }]),
        Report {
            rooms: 1,
            assignment_checksum: 1,
            reuses: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[Entry { start: 0, end: 4 }, Entry { start: 2, end: 5 }]),
        Report {
            rooms: 2,
            assignment_checksum: 5,
            reuses: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry { start: 7, end: 10 },
            Entry { start: 7, end: 9 },
            Entry { start: 8, end: 10 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 14,
            reuses: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 3 },
            Entry { start: 4, end: 9 },
            Entry { start: 8, end: 12 },
            Entry { start: 8, end: 15 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 21,
            reuses: 1
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry { start: 6, end: 9 },
            Entry { start: 8, end: 9 },
            Entry { start: 10, end: 12 },
            Entry { start: 10, end: 11 },
            Entry { start: 10, end: 14 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 31,
            reuses: 2
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry { start: 3, end: 7 },
            Entry { start: 4, end: 10 },
            Entry { start: 5, end: 9 },
            Entry { start: 9, end: 16 },
            Entry { start: 9, end: 16 },
            Entry { start: 10, end: 12 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 45,
            reuses: 3
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 6 },
            Entry { start: 2, end: 9 },
            Entry { start: 3, end: 8 },
            Entry { start: 3, end: 5 },
            Entry { start: 8, end: 13 },
            Entry { start: 8, end: 10 },
            Entry { start: 10, end: 11 }
        ]),
        Report {
            rooms: 4,
            assignment_checksum: 67,
            reuses: 3
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry { start: 2, end: 9 },
            Entry { start: 3, end: 5 },
            Entry { start: 4, end: 9 },
            Entry { start: 5, end: 7 },
            Entry { start: 7, end: 14 },
            Entry { start: 8, end: 15 },
            Entry { start: 10, end: 11 },
            Entry { start: 10, end: 12 }
        ]),
        Report {
            rooms: 4,
            assignment_checksum: 87,
            reuses: 4
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 1 },
            Entry { start: 0, end: 2 },
            Entry { start: 0, end: 3 },
            Entry { start: 3, end: 4 },
            Entry { start: 3, end: 4 }
        ]),
        Report {
            rooms: 3,
            assignment_checksum: 28,
            reuses: 2
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry { start: 0, end: 5 },
            Entry { start: 1, end: 3 },
            Entry { start: 3, end: 6 },
            Entry { start: 5, end: 7 }
        ]),
        Report {
            rooms: 2,
            assignment_checksum: 15,
            reuses: 2
        },
        "fixture 22"
    );
}
