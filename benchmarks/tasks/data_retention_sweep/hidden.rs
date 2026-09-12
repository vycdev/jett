include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                key: 3,
                modified: 8,
                size: 3
            }],
            2
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 1
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 1,
                    modified: 6,
                    size: 3
                },
                Entry {
                    key: 3,
                    modified: 7,
                    size: 5
                }
            ],
            3
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 2
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 2,
                    modified: 11,
                    size: 1
                },
                Entry {
                    key: 0,
                    modified: 0,
                    size: 6
                },
                Entry {
                    key: 0,
                    modified: (-2),
                    size: 5
                }
            ],
            4
        ),
        Report {
            removed: 1,
            reclaimed: 5,
            retained: 2
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 3,
                    modified: 10,
                    size: 2
                },
                Entry {
                    key: 2,
                    modified: 2,
                    size: 3
                },
                Entry {
                    key: 4,
                    modified: 0,
                    size: 4
                },
                Entry {
                    key: 0,
                    modified: 11,
                    size: 2
                }
            ],
            5
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 4
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 3,
                    modified: 10,
                    size: 3
                },
                Entry {
                    key: 0,
                    modified: 5,
                    size: 5
                },
                Entry {
                    key: 4,
                    modified: 2,
                    size: 6
                },
                Entry {
                    key: 1,
                    modified: 3,
                    size: 1
                },
                Entry {
                    key: 1,
                    modified: 1,
                    size: 1
                }
            ],
            6
        ),
        Report {
            removed: 1,
            reclaimed: 1,
            retained: 4
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 3,
                    modified: 5,
                    size: 6
                },
                Entry {
                    key: 1,
                    modified: (-2),
                    size: 5
                },
                Entry {
                    key: 3,
                    modified: 11,
                    size: 6
                },
                Entry {
                    key: 4,
                    modified: 2,
                    size: 4
                },
                Entry {
                    key: 0,
                    modified: 10,
                    size: 6
                },
                Entry {
                    key: 3,
                    modified: 6,
                    size: 5
                }
            ],
            7
        ),
        Report {
            removed: 2,
            reclaimed: 11,
            retained: 4
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 1,
                    modified: 5,
                    size: 1
                },
                Entry {
                    key: 1,
                    modified: 1,
                    size: 1
                },
                Entry {
                    key: 3,
                    modified: (-1),
                    size: 1
                },
                Entry {
                    key: 0,
                    modified: 8,
                    size: 4
                },
                Entry {
                    key: 3,
                    modified: 7,
                    size: 4
                },
                Entry {
                    key: 0,
                    modified: 5,
                    size: 4
                },
                Entry {
                    key: 1,
                    modified: 11,
                    size: 3
                }
            ],
            8
        ),
        Report {
            removed: 4,
            reclaimed: 7,
            retained: 3
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 0,
                    modified: (-3),
                    size: 3
                },
                Entry {
                    key: 1,
                    modified: 7,
                    size: 4
                },
                Entry {
                    key: 1,
                    modified: 7,
                    size: 1
                },
                Entry {
                    key: 3,
                    modified: (-2),
                    size: 3
                },
                Entry {
                    key: 4,
                    modified: (-1),
                    size: 4
                },
                Entry {
                    key: 0,
                    modified: 4,
                    size: 5
                },
                Entry {
                    key: 2,
                    modified: 8,
                    size: 4
                },
                Entry {
                    key: 1,
                    modified: (-2),
                    size: 6
                }
            ],
            1
        ),
        Report {
            removed: 2,
            reclaimed: 9,
            retained: 6
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 2,
                    modified: 10,
                    size: 2
                },
                Entry {
                    key: 2,
                    modified: 11,
                    size: 2
                },
                Entry {
                    key: 4,
                    modified: 2,
                    size: 2
                },
                Entry {
                    key: 3,
                    modified: 1,
                    size: 2
                },
                Entry {
                    key: 2,
                    modified: 1,
                    size: 5
                },
                Entry {
                    key: 2,
                    modified: 4,
                    size: 2
                },
                Entry {
                    key: 0,
                    modified: 2,
                    size: 1
                },
                Entry {
                    key: 3,
                    modified: (-3),
                    size: 4
                },
                Entry {
                    key: 4,
                    modified: 2,
                    size: 2
                }
            ],
            2
        ),
        Report {
            removed: 2,
            reclaimed: 9,
            retained: 7
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 0,
                    modified: 7,
                    size: 1
                },
                Entry {
                    key: 3,
                    modified: (-3),
                    size: 1
                },
                Entry {
                    key: 1,
                    modified: 5,
                    size: 6
                },
                Entry {
                    key: 0,
                    modified: 10,
                    size: 1
                },
                Entry {
                    key: 1,
                    modified: 8,
                    size: 3
                },
                Entry {
                    key: 3,
                    modified: 6,
                    size: 3
                },
                Entry {
                    key: 0,
                    modified: 2,
                    size: 3
                },
                Entry {
                    key: 4,
                    modified: 5,
                    size: 3
                },
                Entry {
                    key: 2,
                    modified: (-1),
                    size: 2
                },
                Entry {
                    key: 0,
                    modified: 1,
                    size: 2
                }
            ],
            3
        ),
        Report {
            removed: 3,
            reclaimed: 6,
            retained: 7
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 2,
                    modified: 5,
                    size: 3
                },
                Entry {
                    key: 4,
                    modified: 3,
                    size: 6
                },
                Entry {
                    key: 3,
                    modified: 11,
                    size: 1
                },
                Entry {
                    key: 1,
                    modified: 8,
                    size: 6
                },
                Entry {
                    key: 3,
                    modified: 4,
                    size: 2
                },
                Entry {
                    key: 0,
                    modified: 1,
                    size: 1
                },
                Entry {
                    key: 4,
                    modified: 3,
                    size: 2
                },
                Entry {
                    key: 1,
                    modified: 5,
                    size: 6
                },
                Entry {
                    key: 4,
                    modified: 6,
                    size: 5
                },
                Entry {
                    key: 1,
                    modified: 9,
                    size: 5
                },
                Entry {
                    key: 4,
                    modified: 7,
                    size: 2
                }
            ],
            4
        ),
        Report {
            removed: 2,
            reclaimed: 8,
            retained: 9
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 1,
                    modified: 3,
                    size: 2
                },
                Entry {
                    key: 1,
                    modified: 7,
                    size: 2
                },
                Entry {
                    key: 3,
                    modified: 4,
                    size: 6
                },
                Entry {
                    key: 2,
                    modified: 6,
                    size: 3
                },
                Entry {
                    key: 2,
                    modified: 11,
                    size: 3
                },
                Entry {
                    key: 1,
                    modified: 0,
                    size: 5
                },
                Entry {
                    key: 0,
                    modified: 2,
                    size: 2
                },
                Entry {
                    key: 1,
                    modified: 10,
                    size: 3
                },
                Entry {
                    key: 1,
                    modified: (-1),
                    size: 5
                },
                Entry {
                    key: 0,
                    modified: (-1),
                    size: 1
                },
                Entry {
                    key: 3,
                    modified: 11,
                    size: 6
                },
                Entry {
                    key: 3,
                    modified: (-3),
                    size: 4
                }
            ],
            5
        ),
        Report {
            removed: 6,
            reclaimed: 23,
            retained: 6
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                key: 3,
                modified: (-1),
                size: 5
            }],
            7
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 1
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 0,
                    modified: 4,
                    size: 1
                },
                Entry {
                    key: 4,
                    modified: 8,
                    size: 4
                }
            ],
            8
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 2
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 3,
                    modified: 7,
                    size: 3
                },
                Entry {
                    key: 0,
                    modified: (-1),
                    size: 4
                },
                Entry {
                    key: 0,
                    modified: 3,
                    size: 4
                }
            ],
            1
        ),
        Report {
            removed: 1,
            reclaimed: 4,
            retained: 2
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 1,
                    modified: 11,
                    size: 3
                },
                Entry {
                    key: 0,
                    modified: 5,
                    size: 2
                },
                Entry {
                    key: 3,
                    modified: 4,
                    size: 3
                },
                Entry {
                    key: 0,
                    modified: 4,
                    size: 2
                }
            ],
            2
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 4
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 1,
                    modified: 1,
                    size: 5
                },
                Entry {
                    key: 2,
                    modified: 5,
                    size: 4
                },
                Entry {
                    key: 4,
                    modified: (-3),
                    size: 3
                },
                Entry {
                    key: 2,
                    modified: 5,
                    size: 5
                },
                Entry {
                    key: 1,
                    modified: (-3),
                    size: 6
                }
            ],
            3
        ),
        Report {
            removed: 1,
            reclaimed: 6,
            retained: 4
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 0,
                    modified: 4,
                    size: 4
                },
                Entry {
                    key: 2,
                    modified: 1,
                    size: 6
                },
                Entry {
                    key: 3,
                    modified: 10,
                    size: 1
                },
                Entry {
                    key: 4,
                    modified: (-3),
                    size: 1
                },
                Entry {
                    key: 2,
                    modified: 4,
                    size: 1
                },
                Entry {
                    key: 1,
                    modified: 4,
                    size: 6
                }
            ],
            4
        ),
        Report {
            removed: 1,
            reclaimed: 6,
            retained: 5
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 2,
                    modified: 1,
                    size: 6
                },
                Entry {
                    key: 0,
                    modified: 6,
                    size: 5
                },
                Entry {
                    key: 1,
                    modified: 6,
                    size: 5
                },
                Entry {
                    key: 0,
                    modified: 0,
                    size: 5
                },
                Entry {
                    key: 1,
                    modified: 0,
                    size: 2
                },
                Entry {
                    key: 2,
                    modified: 11,
                    size: 1
                },
                Entry {
                    key: 4,
                    modified: (-1),
                    size: 4
                }
            ],
            5
        ),
        Report {
            removed: 3,
            reclaimed: 13,
            retained: 4
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 4,
                    modified: 6,
                    size: 1
                },
                Entry {
                    key: 4,
                    modified: (-1),
                    size: 1
                },
                Entry {
                    key: 0,
                    modified: (-3),
                    size: 4
                },
                Entry {
                    key: 2,
                    modified: 9,
                    size: 5
                },
                Entry {
                    key: 0,
                    modified: (-1),
                    size: 2
                },
                Entry {
                    key: 0,
                    modified: 2,
                    size: 2
                },
                Entry {
                    key: 1,
                    modified: (-2),
                    size: 3
                },
                Entry {
                    key: 1,
                    modified: (-1),
                    size: 6
                }
            ],
            6
        ),
        Report {
            removed: 4,
            reclaimed: 10,
            retained: 4
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 0,
                    modified: 2,
                    size: 3
                },
                Entry {
                    key: 0,
                    modified: 2,
                    size: 7
                }
            ],
            3
        ),
        Report {
            removed: 1,
            reclaimed: 3,
            retained: 1
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 0,
                    modified: 3,
                    size: 5
                },
                Entry {
                    key: 0,
                    modified: 4,
                    size: 2
                },
                Entry {
                    key: 1,
                    modified: 1,
                    size: 9
                }
            ],
            3
        ),
        Report {
            removed: 0,
            reclaimed: 0,
            retained: 3
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    key: 1,
                    modified: 2,
                    size: 5
                },
                Entry {
                    key: 1,
                    modified: 2,
                    size: 7
                },
                Entry {
                    key: 2,
                    modified: 1,
                    size: 9
                },
                Entry {
                    key: 1,
                    modified: 5,
                    size: 4
                }
            ],
            4
        ),
        Report {
            removed: 2,
            reclaimed: 12,
            retained: 2
        },
        "fixture 26"
    );
}
