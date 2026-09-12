include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            assigned: 0,
            rejected: 0,
            seat_checksum: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            assigned: 0,
            rejected: 0,
            seat_checksum: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            assigned: 0,
            rejected: 0,
            seat_checksum: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                passenger: 3,
                preferred: 8
            }],
            2
        ),
        Report {
            assigned: 1,
            rejected: 0,
            seat_checksum: 4
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 1,
                    preferred: 6
                },
                Entry {
                    passenger: 3,
                    preferred: 7
                }
            ],
            3
        ),
        Report {
            assigned: 2,
            rejected: 0,
            seat_checksum: 10
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 2,
                    preferred: 11
                },
                Entry {
                    passenger: 0,
                    preferred: 0
                },
                Entry {
                    passenger: 0,
                    preferred: (-2)
                }
            ],
            4
        ),
        Report {
            assigned: 2,
            rejected: 1,
            seat_checksum: 5
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 3,
                    preferred: 10
                },
                Entry {
                    passenger: 2,
                    preferred: 2
                },
                Entry {
                    passenger: 4,
                    preferred: 0
                },
                Entry {
                    passenger: 0,
                    preferred: 11
                }
            ],
            5
        ),
        Report {
            assigned: 4,
            rejected: 0,
            seat_checksum: 29
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 3,
                    preferred: 10
                },
                Entry {
                    passenger: 0,
                    preferred: 5
                },
                Entry {
                    passenger: 4,
                    preferred: 2
                },
                Entry {
                    passenger: 1,
                    preferred: 3
                },
                Entry {
                    passenger: 1,
                    preferred: 1
                }
            ],
            6
        ),
        Report {
            assigned: 4,
            rejected: 1,
            seat_checksum: 25
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 3,
                    preferred: 5
                },
                Entry {
                    passenger: 1,
                    preferred: (-2)
                },
                Entry {
                    passenger: 3,
                    preferred: 11
                },
                Entry {
                    passenger: 4,
                    preferred: 2
                },
                Entry {
                    passenger: 0,
                    preferred: 10
                },
                Entry {
                    passenger: 3,
                    preferred: 6
                }
            ],
            7
        ),
        Report {
            assigned: 4,
            rejected: 2,
            seat_checksum: 35
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 1,
                    preferred: 5
                },
                Entry {
                    passenger: 1,
                    preferred: 1
                },
                Entry {
                    passenger: 3,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: 8
                },
                Entry {
                    passenger: 3,
                    preferred: 7
                },
                Entry {
                    passenger: 0,
                    preferred: 5
                },
                Entry {
                    passenger: 1,
                    preferred: 11
                }
            ],
            8
        ),
        Report {
            assigned: 3,
            rejected: 4,
            seat_checksum: 22
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 0,
                    preferred: (-3)
                },
                Entry {
                    passenger: 1,
                    preferred: 7
                },
                Entry {
                    passenger: 1,
                    preferred: 7
                },
                Entry {
                    passenger: 3,
                    preferred: (-2)
                },
                Entry {
                    passenger: 4,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: 4
                },
                Entry {
                    passenger: 2,
                    preferred: 8
                },
                Entry {
                    passenger: 1,
                    preferred: (-2)
                }
            ],
            1
        ),
        Report {
            assigned: 1,
            rejected: 7,
            seat_checksum: 1
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 2,
                    preferred: 10
                },
                Entry {
                    passenger: 2,
                    preferred: 11
                },
                Entry {
                    passenger: 4,
                    preferred: 2
                },
                Entry {
                    passenger: 3,
                    preferred: 1
                },
                Entry {
                    passenger: 2,
                    preferred: 1
                },
                Entry {
                    passenger: 2,
                    preferred: 4
                },
                Entry {
                    passenger: 0,
                    preferred: 2
                },
                Entry {
                    passenger: 3,
                    preferred: (-3)
                },
                Entry {
                    passenger: 4,
                    preferred: 2
                }
            ],
            2
        ),
        Report {
            assigned: 2,
            rejected: 7,
            seat_checksum: 13
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 0,
                    preferred: 7
                },
                Entry {
                    passenger: 3,
                    preferred: (-3)
                },
                Entry {
                    passenger: 1,
                    preferred: 5
                },
                Entry {
                    passenger: 0,
                    preferred: 10
                },
                Entry {
                    passenger: 1,
                    preferred: 8
                },
                Entry {
                    passenger: 3,
                    preferred: 6
                },
                Entry {
                    passenger: 0,
                    preferred: 2
                },
                Entry {
                    passenger: 4,
                    preferred: 5
                },
                Entry {
                    passenger: 2,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: 1
                }
            ],
            3
        ),
        Report {
            assigned: 3,
            rejected: 7,
            seat_checksum: 15
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 2,
                    preferred: 5
                },
                Entry {
                    passenger: 4,
                    preferred: 3
                },
                Entry {
                    passenger: 3,
                    preferred: 11
                },
                Entry {
                    passenger: 1,
                    preferred: 8
                },
                Entry {
                    passenger: 3,
                    preferred: 4
                },
                Entry {
                    passenger: 0,
                    preferred: 1
                },
                Entry {
                    passenger: 4,
                    preferred: 3
                },
                Entry {
                    passenger: 1,
                    preferred: 5
                },
                Entry {
                    passenger: 4,
                    preferred: 6
                },
                Entry {
                    passenger: 1,
                    preferred: 9
                },
                Entry {
                    passenger: 4,
                    preferred: 7
                }
            ],
            4
        ),
        Report {
            assigned: 4,
            rejected: 7,
            seat_checksum: 34
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 1,
                    preferred: 3
                },
                Entry {
                    passenger: 1,
                    preferred: 7
                },
                Entry {
                    passenger: 3,
                    preferred: 4
                },
                Entry {
                    passenger: 2,
                    preferred: 6
                },
                Entry {
                    passenger: 2,
                    preferred: 11
                },
                Entry {
                    passenger: 1,
                    preferred: 0
                },
                Entry {
                    passenger: 0,
                    preferred: 2
                },
                Entry {
                    passenger: 1,
                    preferred: 10
                },
                Entry {
                    passenger: 1,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: (-1)
                },
                Entry {
                    passenger: 3,
                    preferred: 11
                },
                Entry {
                    passenger: 3,
                    preferred: (-3)
                }
            ],
            5
        ),
        Report {
            assigned: 4,
            rejected: 8,
            seat_checksum: 27
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            assigned: 0,
            rejected: 0,
            seat_checksum: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                passenger: 3,
                preferred: (-1)
            }],
            7
        ),
        Report {
            assigned: 1,
            rejected: 0,
            seat_checksum: 4
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 0,
                    preferred: 4
                },
                Entry {
                    passenger: 4,
                    preferred: 8
                }
            ],
            8
        ),
        Report {
            assigned: 2,
            rejected: 0,
            seat_checksum: 44
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 3,
                    preferred: 7
                },
                Entry {
                    passenger: 0,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: 3
                }
            ],
            1
        ),
        Report {
            assigned: 1,
            rejected: 2,
            seat_checksum: 4
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 1,
                    preferred: 11
                },
                Entry {
                    passenger: 0,
                    preferred: 5
                },
                Entry {
                    passenger: 3,
                    preferred: 4
                },
                Entry {
                    passenger: 0,
                    preferred: 4
                }
            ],
            2
        ),
        Report {
            assigned: 2,
            rejected: 2,
            seat_checksum: 4
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 1,
                    preferred: 1
                },
                Entry {
                    passenger: 2,
                    preferred: 5
                },
                Entry {
                    passenger: 4,
                    preferred: (-3)
                },
                Entry {
                    passenger: 2,
                    preferred: 5
                },
                Entry {
                    passenger: 1,
                    preferred: (-3)
                }
            ],
            3
        ),
        Report {
            assigned: 3,
            rejected: 2,
            seat_checksum: 23
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 0,
                    preferred: 4
                },
                Entry {
                    passenger: 2,
                    preferred: 1
                },
                Entry {
                    passenger: 3,
                    preferred: 10
                },
                Entry {
                    passenger: 4,
                    preferred: (-3)
                },
                Entry {
                    passenger: 2,
                    preferred: 4
                },
                Entry {
                    passenger: 1,
                    preferred: 4
                }
            ],
            4
        ),
        Report {
            assigned: 4,
            rejected: 2,
            seat_checksum: 30
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 2,
                    preferred: 1
                },
                Entry {
                    passenger: 0,
                    preferred: 6
                },
                Entry {
                    passenger: 1,
                    preferred: 6
                },
                Entry {
                    passenger: 0,
                    preferred: 0
                },
                Entry {
                    passenger: 1,
                    preferred: 0
                },
                Entry {
                    passenger: 2,
                    preferred: 11
                },
                Entry {
                    passenger: 4,
                    preferred: (-1)
                }
            ],
            5
        ),
        Report {
            assigned: 4,
            rejected: 3,
            seat_checksum: 31
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 4,
                    preferred: 6
                },
                Entry {
                    passenger: 4,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: (-3)
                },
                Entry {
                    passenger: 2,
                    preferred: 9
                },
                Entry {
                    passenger: 0,
                    preferred: (-1)
                },
                Entry {
                    passenger: 0,
                    preferred: 2
                },
                Entry {
                    passenger: 1,
                    preferred: (-2)
                },
                Entry {
                    passenger: 1,
                    preferred: (-1)
                }
            ],
            6
        ),
        Report {
            assigned: 4,
            rejected: 4,
            seat_checksum: 43
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 0,
                    preferred: 0
                },
                Entry {
                    passenger: 1,
                    preferred: (-1)
                },
                Entry {
                    passenger: 2,
                    preferred: 9
                },
                Entry {
                    passenger: 0,
                    preferred: 3
                }
            ],
            3
        ),
        Report {
            assigned: 3,
            rejected: 1,
            seat_checksum: 14
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    passenger: 2,
                    preferred: 2
                },
                Entry {
                    passenger: 3,
                    preferred: 2
                },
                Entry {
                    passenger: 2,
                    preferred: 3
                },
                Entry {
                    passenger: 4,
                    preferred: 9
                },
                Entry {
                    passenger: 5,
                    preferred: 1
                }
            ],
            3
        ),
        Report {
            assigned: 3,
            rejected: 2,
            seat_checksum: 25
        },
        "fixture 25"
    );
}
