include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            peak: 0,
            earliest: (-1),
            overloaded_starts: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            peak: 0,
            earliest: (-1),
            overloaded_starts: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            peak: 0,
            earliest: (-1),
            overloaded_starts: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                start: 11,
                end: 15,
                demand: 3
            }],
            2
        ),
        Report {
            peak: 3,
            earliest: 11,
            overloaded_starts: 1
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 3,
                    end: 8,
                    demand: 3
                },
                Entry {
                    start: 6,
                    end: 12,
                    demand: 5
                }
            ],
            3
        ),
        Report {
            peak: 8,
            earliest: 6,
            overloaded_starts: 1
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 10,
                    end: 13,
                    demand: 1
                },
                Entry {
                    start: 0,
                    end: 2,
                    demand: 1
                },
                Entry {
                    start: 1,
                    end: 6,
                    demand: 4
                }
            ],
            4
        ),
        Report {
            peak: 5,
            earliest: 1,
            overloaded_starts: 1
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 3,
                    end: 6,
                    demand: 3
                },
                Entry {
                    start: 5,
                    end: 10,
                    demand: 2
                },
                Entry {
                    start: 7,
                    end: 8,
                    demand: 2
                },
                Entry {
                    start: 6,
                    end: 13,
                    demand: 3
                }
            ],
            5
        ),
        Report {
            peak: 7,
            earliest: 7,
            overloaded_starts: 1
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 0,
                    end: 5,
                    demand: 5
                },
                Entry {
                    start: 9,
                    end: 12,
                    demand: 2
                },
                Entry {
                    start: 6,
                    end: 7,
                    demand: 2
                },
                Entry {
                    start: 4,
                    end: 5,
                    demand: 4
                },
                Entry {
                    start: 8,
                    end: 15,
                    demand: 2
                }
            ],
            6
        ),
        Report {
            peak: 9,
            earliest: 4,
            overloaded_starts: 1
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 1,
                    end: 8,
                    demand: 5
                },
                Entry {
                    start: 7,
                    end: 13,
                    demand: 5
                },
                Entry {
                    start: 5,
                    end: 9,
                    demand: 1
                },
                Entry {
                    start: 10,
                    end: 14,
                    demand: 5
                },
                Entry {
                    start: 9,
                    end: 11,
                    demand: 5
                },
                Entry {
                    start: 0,
                    end: 2,
                    demand: 3
                }
            ],
            7
        ),
        Report {
            peak: 15,
            earliest: 10,
            overloaded_starts: 4
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 1,
                    end: 5,
                    demand: 2
                },
                Entry {
                    start: 0,
                    end: 1,
                    demand: 4
                },
                Entry {
                    start: 6,
                    end: 12,
                    demand: 4
                },
                Entry {
                    start: 11,
                    end: 12,
                    demand: 5
                },
                Entry {
                    start: 7,
                    end: 14,
                    demand: 2
                },
                Entry {
                    start: 5,
                    end: 12,
                    demand: 1
                },
                Entry {
                    start: 0,
                    end: 3,
                    demand: 2
                }
            ],
            8
        ),
        Report {
            peak: 12,
            earliest: 11,
            overloaded_starts: 1
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 10,
                    end: 14,
                    demand: 2
                },
                Entry {
                    start: 10,
                    end: 17,
                    demand: 1
                },
                Entry {
                    start: 7,
                    end: 8,
                    demand: 3
                },
                Entry {
                    start: 9,
                    end: 11,
                    demand: 4
                },
                Entry {
                    start: 11,
                    end: 12,
                    demand: 4
                },
                Entry {
                    start: 9,
                    end: 12,
                    demand: 4
                },
                Entry {
                    start: 2,
                    end: 3,
                    demand: 3
                },
                Entry {
                    start: 3,
                    end: 9,
                    demand: 3
                }
            ],
            1
        ),
        Report {
            peak: 11,
            earliest: 10,
            overloaded_starts: 6
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 2,
                    end: 9,
                    demand: 5
                },
                Entry {
                    start: 5,
                    end: 7,
                    demand: 4
                },
                Entry {
                    start: 4,
                    end: 6,
                    demand: 3
                },
                Entry {
                    start: 4,
                    end: 11,
                    demand: 5
                },
                Entry {
                    start: 5,
                    end: 9,
                    demand: 2
                },
                Entry {
                    start: 10,
                    end: 11,
                    demand: 3
                },
                Entry {
                    start: 0,
                    end: 4,
                    demand: 1
                },
                Entry {
                    start: 6,
                    end: 11,
                    demand: 3
                },
                Entry {
                    start: 3,
                    end: 4,
                    demand: 1
                }
            ],
            2
        ),
        Report {
            peak: 19,
            earliest: 5,
            overloaded_starts: 6
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 7,
                    end: 8,
                    demand: 1
                },
                Entry {
                    start: 2,
                    end: 7,
                    demand: 1
                },
                Entry {
                    start: 1,
                    end: 7,
                    demand: 2
                },
                Entry {
                    start: 11,
                    end: 14,
                    demand: 4
                },
                Entry {
                    start: 9,
                    end: 12,
                    demand: 1
                },
                Entry {
                    start: 5,
                    end: 8,
                    demand: 5
                },
                Entry {
                    start: 8,
                    end: 11,
                    demand: 3
                },
                Entry {
                    start: 2,
                    end: 4,
                    demand: 1
                },
                Entry {
                    start: 4,
                    end: 6,
                    demand: 3
                },
                Entry {
                    start: 8,
                    end: 11,
                    demand: 5
                }
            ],
            3
        ),
        Report {
            peak: 11,
            earliest: 5,
            overloaded_starts: 7
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 6,
                    end: 12,
                    demand: 4
                },
                Entry {
                    start: 0,
                    end: 2,
                    demand: 4
                },
                Entry {
                    start: 7,
                    end: 9,
                    demand: 1
                },
                Entry {
                    start: 4,
                    end: 5,
                    demand: 5
                },
                Entry {
                    start: 6,
                    end: 8,
                    demand: 2
                },
                Entry {
                    start: 8,
                    end: 14,
                    demand: 5
                },
                Entry {
                    start: 9,
                    end: 14,
                    demand: 2
                },
                Entry {
                    start: 9,
                    end: 14,
                    demand: 2
                },
                Entry {
                    start: 3,
                    end: 7,
                    demand: 2
                },
                Entry {
                    start: 2,
                    end: 8,
                    demand: 2
                },
                Entry {
                    start: 7,
                    end: 11,
                    demand: 3
                }
            ],
            4
        ),
        Report {
            peak: 16,
            earliest: 9,
            overloaded_starts: 5
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 9,
                    end: 12,
                    demand: 3
                },
                Entry {
                    start: 5,
                    end: 12,
                    demand: 2
                },
                Entry {
                    start: 3,
                    end: 8,
                    demand: 1
                },
                Entry {
                    start: 5,
                    end: 7,
                    demand: 2
                },
                Entry {
                    start: 4,
                    end: 6,
                    demand: 2
                },
                Entry {
                    start: 9,
                    end: 10,
                    demand: 2
                },
                Entry {
                    start: 1,
                    end: 8,
                    demand: 4
                },
                Entry {
                    start: 11,
                    end: 15,
                    demand: 1
                },
                Entry {
                    start: 6,
                    end: 10,
                    demand: 2
                },
                Entry {
                    start: 8,
                    end: 9,
                    demand: 4
                },
                Entry {
                    start: 1,
                    end: 6,
                    demand: 4
                },
                Entry {
                    start: 6,
                    end: 12,
                    demand: 3
                }
            ],
            5
        ),
        Report {
            peak: 15,
            earliest: 5,
            overloaded_starts: 8
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            peak: 0,
            earliest: (-1),
            overloaded_starts: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                start: 1,
                end: 3,
                demand: 4
            }],
            7
        ),
        Report {
            peak: 4,
            earliest: 1,
            overloaded_starts: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 0,
                    end: 4,
                    demand: 4
                },
                Entry {
                    start: 2,
                    end: 5,
                    demand: 1
                }
            ],
            8
        ),
        Report {
            peak: 5,
            earliest: 2,
            overloaded_starts: 0
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 8,
                    end: 10,
                    demand: 4
                },
                Entry {
                    start: 7,
                    end: 10,
                    demand: 1
                },
                Entry {
                    start: 7,
                    end: 9,
                    demand: 2
                }
            ],
            1
        ),
        Report {
            peak: 7,
            earliest: 8,
            overloaded_starts: 2
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 4,
                    end: 9,
                    demand: 3
                },
                Entry {
                    start: 8,
                    end: 12,
                    demand: 5
                },
                Entry {
                    start: 0,
                    end: 3,
                    demand: 3
                },
                Entry {
                    start: 8,
                    end: 15,
                    demand: 5
                }
            ],
            2
        ),
        Report {
            peak: 13,
            earliest: 8,
            overloaded_starts: 3
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 10,
                    end: 12,
                    demand: 1
                },
                Entry {
                    start: 10,
                    end: 11,
                    demand: 4
                },
                Entry {
                    start: 6,
                    end: 9,
                    demand: 3
                },
                Entry {
                    start: 10,
                    end: 14,
                    demand: 1
                },
                Entry {
                    start: 8,
                    end: 9,
                    demand: 1
                }
            ],
            3
        ),
        Report {
            peak: 6,
            earliest: 10,
            overloaded_starts: 2
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 5,
                    end: 9,
                    demand: 1
                },
                Entry {
                    start: 3,
                    end: 7,
                    demand: 3
                },
                Entry {
                    start: 4,
                    end: 10,
                    demand: 1
                },
                Entry {
                    start: 9,
                    end: 16,
                    demand: 5
                },
                Entry {
                    start: 10,
                    end: 12,
                    demand: 5
                },
                Entry {
                    start: 9,
                    end: 16,
                    demand: 1
                }
            ],
            4
        ),
        Report {
            peak: 11,
            earliest: 10,
            overloaded_starts: 3
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 3,
                    end: 8,
                    demand: 2
                },
                Entry {
                    start: 3,
                    end: 5,
                    demand: 3
                },
                Entry {
                    start: 0,
                    end: 6,
                    demand: 5
                },
                Entry {
                    start: 2,
                    end: 9,
                    demand: 4
                },
                Entry {
                    start: 8,
                    end: 13,
                    demand: 1
                },
                Entry {
                    start: 8,
                    end: 10,
                    demand: 1
                },
                Entry {
                    start: 10,
                    end: 11,
                    demand: 1
                }
            ],
            5
        ),
        Report {
            peak: 14,
            earliest: 3,
            overloaded_starts: 3
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 7,
                    end: 14,
                    demand: 3
                },
                Entry {
                    start: 8,
                    end: 15,
                    demand: 1
                },
                Entry {
                    start: 2,
                    end: 9,
                    demand: 2
                },
                Entry {
                    start: 10,
                    end: 11,
                    demand: 3
                },
                Entry {
                    start: 3,
                    end: 5,
                    demand: 1
                },
                Entry {
                    start: 5,
                    end: 7,
                    demand: 2
                },
                Entry {
                    start: 10,
                    end: 12,
                    demand: 3
                },
                Entry {
                    start: 4,
                    end: 9,
                    demand: 4
                }
            ],
            6
        ),
        Report {
            peak: 10,
            earliest: 8,
            overloaded_starts: 5
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 0,
                    end: 2,
                    demand: 3
                },
                Entry {
                    start: 2,
                    end: 4,
                    demand: 3
                }
            ],
            3
        ),
        Report {
            peak: 3,
            earliest: 0,
            overloaded_starts: 0
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 2,
                    end: 4,
                    demand: 5
                },
                Entry {
                    start: 0,
                    end: 2,
                    demand: 5
                },
                Entry {
                    start: 0,
                    end: 2,
                    demand: 1
                }
            ],
            5
        ),
        Report {
            peak: 6,
            earliest: 0,
            overloaded_starts: 1
        },
        "fixture 25"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    start: 4,
                    end: 8,
                    demand: 2
                },
                Entry {
                    start: 0,
                    end: 4,
                    demand: 3
                },
                Entry {
                    start: 2,
                    end: 6,
                    demand: 4
                },
                Entry {
                    start: 2,
                    end: 3,
                    demand: 1
                }
            ],
            5
        ),
        Report {
            peak: 8,
            earliest: 2,
            overloaded_starts: 2
        },
        "fixture 26"
    );
}
