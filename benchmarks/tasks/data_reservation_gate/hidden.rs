include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            accepted: 0,
            rejected: 0,
            occupied: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            accepted: 0,
            rejected: 0,
            occupied: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            accepted: 0,
            rejected: 0,
            occupied: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[Entry { party: 3, seats: 8 }], 2),
        Report {
            accepted: 0,
            rejected: 1,
            occupied: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[Entry { party: 1, seats: 6 }, Entry { party: 3, seats: 7 }],
            3
        ),
        Report {
            accepted: 0,
            rejected: 2,
            occupied: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    party: 2,
                    seats: 11
                },
                Entry { party: 0, seats: 0 },
                Entry {
                    party: 0,
                    seats: (-2)
                }
            ],
            4
        ),
        Report {
            accepted: 0,
            rejected: 3,
            occupied: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    party: 3,
                    seats: 10
                },
                Entry { party: 2, seats: 2 },
                Entry { party: 4, seats: 0 },
                Entry {
                    party: 0,
                    seats: 11
                }
            ],
            5
        ),
        Report {
            accepted: 1,
            rejected: 3,
            occupied: 2
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    party: 3,
                    seats: 10
                },
                Entry { party: 0, seats: 5 },
                Entry { party: 4, seats: 2 },
                Entry { party: 1, seats: 3 },
                Entry { party: 1, seats: 1 }
            ],
            6
        ),
        Report {
            accepted: 2,
            rejected: 3,
            occupied: 6
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 3, seats: 5 },
                Entry {
                    party: 1,
                    seats: (-2)
                },
                Entry {
                    party: 3,
                    seats: 11
                },
                Entry { party: 4, seats: 2 },
                Entry {
                    party: 0,
                    seats: 10
                },
                Entry { party: 3, seats: 6 }
            ],
            7
        ),
        Report {
            accepted: 2,
            rejected: 4,
            occupied: 7
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 1, seats: 5 },
                Entry { party: 1, seats: 1 },
                Entry {
                    party: 3,
                    seats: (-1)
                },
                Entry { party: 0, seats: 8 },
                Entry { party: 3, seats: 7 },
                Entry { party: 0, seats: 5 },
                Entry {
                    party: 1,
                    seats: 11
                }
            ],
            8
        ),
        Report {
            accepted: 1,
            rejected: 6,
            occupied: 5
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    party: 0,
                    seats: (-3)
                },
                Entry { party: 1, seats: 7 },
                Entry { party: 1, seats: 7 },
                Entry {
                    party: 3,
                    seats: (-2)
                },
                Entry {
                    party: 4,
                    seats: (-1)
                },
                Entry { party: 0, seats: 4 },
                Entry { party: 2, seats: 8 },
                Entry {
                    party: 1,
                    seats: (-2)
                }
            ],
            1
        ),
        Report {
            accepted: 0,
            rejected: 8,
            occupied: 0
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    party: 2,
                    seats: 10
                },
                Entry {
                    party: 2,
                    seats: 11
                },
                Entry { party: 4, seats: 2 },
                Entry { party: 3, seats: 1 },
                Entry { party: 2, seats: 1 },
                Entry { party: 2, seats: 4 },
                Entry { party: 0, seats: 2 },
                Entry {
                    party: 3,
                    seats: (-3)
                },
                Entry { party: 4, seats: 2 }
            ],
            2
        ),
        Report {
            accepted: 1,
            rejected: 8,
            occupied: 2
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 0, seats: 7 },
                Entry {
                    party: 3,
                    seats: (-3)
                },
                Entry { party: 1, seats: 5 },
                Entry {
                    party: 0,
                    seats: 10
                },
                Entry { party: 1, seats: 8 },
                Entry { party: 3, seats: 6 },
                Entry { party: 0, seats: 2 },
                Entry { party: 4, seats: 5 },
                Entry {
                    party: 2,
                    seats: (-1)
                },
                Entry { party: 0, seats: 1 }
            ],
            3
        ),
        Report {
            accepted: 1,
            rejected: 9,
            occupied: 2
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 2, seats: 5 },
                Entry { party: 4, seats: 3 },
                Entry {
                    party: 3,
                    seats: 11
                },
                Entry { party: 1, seats: 8 },
                Entry { party: 3, seats: 4 },
                Entry { party: 0, seats: 1 },
                Entry { party: 4, seats: 3 },
                Entry { party: 1, seats: 5 },
                Entry { party: 4, seats: 6 },
                Entry { party: 1, seats: 9 },
                Entry { party: 4, seats: 7 }
            ],
            4
        ),
        Report {
            accepted: 2,
            rejected: 9,
            occupied: 4
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 1, seats: 3 },
                Entry { party: 1, seats: 7 },
                Entry { party: 3, seats: 4 },
                Entry { party: 2, seats: 6 },
                Entry {
                    party: 2,
                    seats: 11
                },
                Entry { party: 1, seats: 0 },
                Entry { party: 0, seats: 2 },
                Entry {
                    party: 1,
                    seats: 10
                },
                Entry {
                    party: 1,
                    seats: (-1)
                },
                Entry {
                    party: 0,
                    seats: (-1)
                },
                Entry {
                    party: 3,
                    seats: 11
                },
                Entry {
                    party: 3,
                    seats: (-3)
                }
            ],
            5
        ),
        Report {
            accepted: 2,
            rejected: 10,
            occupied: 5
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            accepted: 0,
            rejected: 0,
            occupied: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                party: 3,
                seats: (-1)
            }],
            7
        ),
        Report {
            accepted: 0,
            rejected: 1,
            occupied: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[Entry { party: 0, seats: 4 }, Entry { party: 4, seats: 8 }],
            8
        ),
        Report {
            accepted: 1,
            rejected: 1,
            occupied: 4
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 3, seats: 7 },
                Entry {
                    party: 0,
                    seats: (-1)
                },
                Entry { party: 0, seats: 3 }
            ],
            1
        ),
        Report {
            accepted: 0,
            rejected: 3,
            occupied: 0
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    party: 1,
                    seats: 11
                },
                Entry { party: 0, seats: 5 },
                Entry { party: 3, seats: 4 },
                Entry { party: 0, seats: 4 }
            ],
            2
        ),
        Report {
            accepted: 0,
            rejected: 4,
            occupied: 0
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 1, seats: 1 },
                Entry { party: 2, seats: 5 },
                Entry {
                    party: 4,
                    seats: (-3)
                },
                Entry { party: 2, seats: 5 },
                Entry {
                    party: 1,
                    seats: (-3)
                }
            ],
            3
        ),
        Report {
            accepted: 1,
            rejected: 4,
            occupied: 1
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 0, seats: 4 },
                Entry { party: 2, seats: 1 },
                Entry {
                    party: 3,
                    seats: 10
                },
                Entry {
                    party: 4,
                    seats: (-3)
                },
                Entry { party: 2, seats: 4 },
                Entry { party: 1, seats: 4 }
            ],
            4
        ),
        Report {
            accepted: 1,
            rejected: 5,
            occupied: 4
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 2, seats: 1 },
                Entry { party: 0, seats: 6 },
                Entry { party: 1, seats: 6 },
                Entry { party: 0, seats: 0 },
                Entry { party: 1, seats: 0 },
                Entry {
                    party: 2,
                    seats: 11
                },
                Entry {
                    party: 4,
                    seats: (-1)
                }
            ],
            5
        ),
        Report {
            accepted: 1,
            rejected: 6,
            occupied: 1
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 4, seats: 6 },
                Entry {
                    party: 4,
                    seats: (-1)
                },
                Entry {
                    party: 0,
                    seats: (-3)
                },
                Entry { party: 2, seats: 9 },
                Entry {
                    party: 0,
                    seats: (-1)
                },
                Entry { party: 0, seats: 2 },
                Entry {
                    party: 1,
                    seats: (-2)
                },
                Entry {
                    party: 1,
                    seats: (-1)
                }
            ],
            6
        ),
        Report {
            accepted: 1,
            rejected: 7,
            occupied: 6
        },
        "fixture 23"
    );
    assert_eq!(
        solve(&[Entry { party: 0, seats: 5 }], 5),
        Report {
            accepted: 1,
            rejected: 0,
            occupied: 5
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[Entry { party: 0, seats: 0 }, Entry { party: 0, seats: 2 }],
            2
        ),
        Report {
            accepted: 1,
            rejected: 1,
            occupied: 2
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 1, seats: 2 },
                Entry { party: 2, seats: 3 },
                Entry { party: 1, seats: 1 }
            ],
            5
        ),
        Report {
            accepted: 2,
            rejected: 1,
            occupied: 5
        },
        "fixture 26"
    );
    assert_eq!(
        solve(
            &[
                Entry { party: 1, seats: 6 },
                Entry { party: 1, seats: 3 },
                Entry { party: 1, seats: 1 },
                Entry { party: 2, seats: 2 }
            ],
            5
        ),
        Report {
            accepted: 2,
            rejected: 2,
            occupied: 5
        },
        "fixture 27"
    );
}
