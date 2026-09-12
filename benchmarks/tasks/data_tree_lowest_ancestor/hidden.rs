include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[Entry { node: 1, parent: 0 }], 2),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[Entry { node: 1, parent: 0 }, Entry { node: 2, parent: 1 }],
            3
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 3, parent: 0 },
                Entry { node: 1, parent: 0 },
                Entry { node: 2, parent: 0 }
            ],
            4
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 3, parent: 0 },
                Entry { node: 1, parent: 0 },
                Entry { node: 2, parent: 1 },
                Entry { node: 4, parent: 0 }
            ],
            5
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 5, parent: 4 },
                Entry { node: 2, parent: 1 },
                Entry { node: 3, parent: 0 },
                Entry { node: 4, parent: 0 },
                Entry { node: 1, parent: 0 }
            ],
            6
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 5, parent: 2 },
                Entry { node: 3, parent: 2 },
                Entry { node: 2, parent: 1 },
                Entry { node: 4, parent: 0 },
                Entry { node: 6, parent: 3 },
                Entry { node: 1, parent: 0 }
            ],
            7
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 6, parent: 1 },
                Entry { node: 7, parent: 6 },
                Entry { node: 4, parent: 0 },
                Entry { node: 2, parent: 1 },
                Entry { node: 5, parent: 1 },
                Entry { node: 3, parent: 1 },
                Entry { node: 1, parent: 0 }
            ],
            8
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[Entry { node: 1, parent: 0 }], 1),
        Report {
            ancestor: 1,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[Entry { node: 1, parent: 0 }, Entry { node: 2, parent: 0 }],
            2
        ),
        Report {
            ancestor: 2,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 1, parent: 0 },
                Entry { node: 3, parent: 1 },
                Entry { node: 2, parent: 0 }
            ],
            3
        ),
        Report {
            ancestor: 3,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 4, parent: 1 },
                Entry { node: 2, parent: 1 },
                Entry { node: 1, parent: 0 },
                Entry { node: 3, parent: 1 }
            ],
            4
        ),
        Report {
            ancestor: 4,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 2, parent: 1 },
                Entry { node: 3, parent: 0 },
                Entry { node: 4, parent: 3 },
                Entry { node: 1, parent: 0 },
                Entry { node: 5, parent: 0 }
            ],
            5
        ),
        Report {
            ancestor: 5,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[Entry { node: 1, parent: 0 }], 7),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[Entry { node: 1, parent: 0 }, Entry { node: 2, parent: 1 }],
            8
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[Entry { node: 1, parent: 0 }, Entry { node: 2, parent: 1 }],
            2
        ),
        Report {
            ancestor: 2,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 1, parent: 0 },
                Entry { node: 2, parent: 1 },
                Entry { node: 3, parent: 0 }
            ],
            3
        ),
        Report {
            ancestor: 3,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 4, parent: 3 },
                Entry { node: 1, parent: 0 },
                Entry { node: 3, parent: 0 },
                Entry { node: 2, parent: 0 }
            ],
            4
        ),
        Report {
            ancestor: 4,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 4, parent: 3 },
                Entry { node: 1, parent: 0 },
                Entry { node: 2, parent: 1 },
                Entry { node: 5, parent: 2 },
                Entry { node: 3, parent: 0 }
            ],
            5
        ),
        Report {
            ancestor: 5,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 3, parent: 0 },
                Entry { node: 2, parent: 0 },
                Entry { node: 6, parent: 0 },
                Entry { node: 5, parent: 1 },
                Entry { node: 4, parent: 2 },
                Entry { node: 1, parent: 0 }
            ],
            6
        ),
        Report {
            ancestor: 6,
            selected_distance: 0,
            largest_distance: 0
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 1, parent: 0 },
                Entry { node: 2, parent: 1 },
                Entry { node: 3, parent: 1 },
                Entry { node: 4, parent: 2 }
            ],
            1
        ),
        Report {
            ancestor: 1,
            selected_distance: 0,
            largest_distance: 2
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[Entry { node: 1, parent: 0 }, Entry { node: 2, parent: 0 }],
            1
        ),
        Report {
            ancestor: 0,
            selected_distance: (-1),
            largest_distance: (-1)
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry { node: 1, parent: 0 },
                Entry { node: 2, parent: 1 },
                Entry { node: 3, parent: 1 },
                Entry { node: 4, parent: 2 }
            ],
            3
        ),
        Report {
            ancestor: 1,
            selected_distance: 1,
            largest_distance: 2
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    node: 30,
                    parent: 2
                },
                Entry {
                    node: 12,
                    parent: 9
                },
                Entry { node: 2, parent: 0 },
                Entry { node: 9, parent: 2 }
            ],
            12
        ),
        Report {
            ancestor: 2,
            selected_distance: 2,
            largest_distance: 1
        },
        "fixture 26"
    );
}
