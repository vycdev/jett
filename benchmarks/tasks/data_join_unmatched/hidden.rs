include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            key: 3,
            side: 8,
            quantity: 3
        }]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                side: 6,
                quantity: 3
            },
            Entry {
                key: 3,
                side: 7,
                quantity: 5
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                side: 11,
                quantity: 1
            },
            Entry {
                key: 0,
                side: 0,
                quantity: 6
            },
            Entry {
                key: 0,
                side: (-2),
                quantity: 5
            }
        ]),
        Report {
            matched: 0,
            left_only: 6,
            right_only: 0
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                side: 10,
                quantity: 2
            },
            Entry {
                key: 2,
                side: 2,
                quantity: 3
            },
            Entry {
                key: 4,
                side: 0,
                quantity: 4
            },
            Entry {
                key: 0,
                side: 11,
                quantity: 2
            }
        ]),
        Report {
            matched: 0,
            left_only: 4,
            right_only: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                side: 10,
                quantity: 3
            },
            Entry {
                key: 0,
                side: 5,
                quantity: 5
            },
            Entry {
                key: 4,
                side: 2,
                quantity: 6
            },
            Entry {
                key: 1,
                side: 3,
                quantity: 1
            },
            Entry {
                key: 1,
                side: 1,
                quantity: 1
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 1
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                side: 5,
                quantity: 6
            },
            Entry {
                key: 1,
                side: (-2),
                quantity: 5
            },
            Entry {
                key: 3,
                side: 11,
                quantity: 6
            },
            Entry {
                key: 4,
                side: 2,
                quantity: 4
            },
            Entry {
                key: 0,
                side: 10,
                quantity: 6
            },
            Entry {
                key: 3,
                side: 6,
                quantity: 5
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                side: 5,
                quantity: 1
            },
            Entry {
                key: 1,
                side: 1,
                quantity: 1
            },
            Entry {
                key: 3,
                side: (-1),
                quantity: 1
            },
            Entry {
                key: 0,
                side: 8,
                quantity: 4
            },
            Entry {
                key: 3,
                side: 7,
                quantity: 4
            },
            Entry {
                key: 0,
                side: 5,
                quantity: 4
            },
            Entry {
                key: 1,
                side: 11,
                quantity: 3
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 1
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                side: (-3),
                quantity: 3
            },
            Entry {
                key: 1,
                side: 7,
                quantity: 4
            },
            Entry {
                key: 1,
                side: 7,
                quantity: 1
            },
            Entry {
                key: 3,
                side: (-2),
                quantity: 3
            },
            Entry {
                key: 4,
                side: (-1),
                quantity: 4
            },
            Entry {
                key: 0,
                side: 4,
                quantity: 5
            },
            Entry {
                key: 2,
                side: 8,
                quantity: 4
            },
            Entry {
                key: 1,
                side: (-2),
                quantity: 6
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                side: 10,
                quantity: 2
            },
            Entry {
                key: 2,
                side: 11,
                quantity: 2
            },
            Entry {
                key: 4,
                side: 2,
                quantity: 2
            },
            Entry {
                key: 3,
                side: 1,
                quantity: 2
            },
            Entry {
                key: 2,
                side: 1,
                quantity: 5
            },
            Entry {
                key: 2,
                side: 4,
                quantity: 2
            },
            Entry {
                key: 0,
                side: 2,
                quantity: 1
            },
            Entry {
                key: 3,
                side: (-3),
                quantity: 4
            },
            Entry {
                key: 4,
                side: 2,
                quantity: 2
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 7
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                side: 7,
                quantity: 1
            },
            Entry {
                key: 3,
                side: (-3),
                quantity: 1
            },
            Entry {
                key: 1,
                side: 5,
                quantity: 6
            },
            Entry {
                key: 0,
                side: 10,
                quantity: 1
            },
            Entry {
                key: 1,
                side: 8,
                quantity: 3
            },
            Entry {
                key: 3,
                side: 6,
                quantity: 3
            },
            Entry {
                key: 0,
                side: 2,
                quantity: 3
            },
            Entry {
                key: 4,
                side: 5,
                quantity: 3
            },
            Entry {
                key: 2,
                side: (-1),
                quantity: 2
            },
            Entry {
                key: 0,
                side: 1,
                quantity: 2
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 2
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                side: 5,
                quantity: 3
            },
            Entry {
                key: 4,
                side: 3,
                quantity: 6
            },
            Entry {
                key: 3,
                side: 11,
                quantity: 1
            },
            Entry {
                key: 1,
                side: 8,
                quantity: 6
            },
            Entry {
                key: 3,
                side: 4,
                quantity: 2
            },
            Entry {
                key: 0,
                side: 1,
                quantity: 1
            },
            Entry {
                key: 4,
                side: 3,
                quantity: 2
            },
            Entry {
                key: 1,
                side: 5,
                quantity: 6
            },
            Entry {
                key: 4,
                side: 6,
                quantity: 5
            },
            Entry {
                key: 1,
                side: 9,
                quantity: 5
            },
            Entry {
                key: 4,
                side: 7,
                quantity: 2
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 1
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                side: 3,
                quantity: 2
            },
            Entry {
                key: 1,
                side: 7,
                quantity: 2
            },
            Entry {
                key: 3,
                side: 4,
                quantity: 6
            },
            Entry {
                key: 2,
                side: 6,
                quantity: 3
            },
            Entry {
                key: 2,
                side: 11,
                quantity: 3
            },
            Entry {
                key: 1,
                side: 0,
                quantity: 5
            },
            Entry {
                key: 0,
                side: 2,
                quantity: 2
            },
            Entry {
                key: 1,
                side: 10,
                quantity: 3
            },
            Entry {
                key: 1,
                side: (-1),
                quantity: 5
            },
            Entry {
                key: 0,
                side: (-1),
                quantity: 1
            },
            Entry {
                key: 3,
                side: 11,
                quantity: 6
            },
            Entry {
                key: 3,
                side: (-3),
                quantity: 4
            }
        ]),
        Report {
            matched: 0,
            left_only: 5,
            right_only: 0
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            key: 3,
            side: (-1),
            quantity: 5
        }]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                side: 4,
                quantity: 1
            },
            Entry {
                key: 4,
                side: 8,
                quantity: 4
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                side: 7,
                quantity: 3
            },
            Entry {
                key: 0,
                side: (-1),
                quantity: 4
            },
            Entry {
                key: 0,
                side: 3,
                quantity: 4
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                side: 11,
                quantity: 3
            },
            Entry {
                key: 0,
                side: 5,
                quantity: 2
            },
            Entry {
                key: 3,
                side: 4,
                quantity: 3
            },
            Entry {
                key: 0,
                side: 4,
                quantity: 2
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                side: 1,
                quantity: 5
            },
            Entry {
                key: 2,
                side: 5,
                quantity: 4
            },
            Entry {
                key: 4,
                side: (-3),
                quantity: 3
            },
            Entry {
                key: 2,
                side: 5,
                quantity: 5
            },
            Entry {
                key: 1,
                side: (-3),
                quantity: 6
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 5
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                side: 4,
                quantity: 4
            },
            Entry {
                key: 2,
                side: 1,
                quantity: 6
            },
            Entry {
                key: 3,
                side: 10,
                quantity: 1
            },
            Entry {
                key: 4,
                side: (-3),
                quantity: 1
            },
            Entry {
                key: 2,
                side: 4,
                quantity: 1
            },
            Entry {
                key: 1,
                side: 4,
                quantity: 6
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 6
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                side: 1,
                quantity: 6
            },
            Entry {
                key: 0,
                side: 6,
                quantity: 5
            },
            Entry {
                key: 1,
                side: 6,
                quantity: 5
            },
            Entry {
                key: 0,
                side: 0,
                quantity: 5
            },
            Entry {
                key: 1,
                side: 0,
                quantity: 2
            },
            Entry {
                key: 2,
                side: 11,
                quantity: 1
            },
            Entry {
                key: 4,
                side: (-1),
                quantity: 4
            }
        ]),
        Report {
            matched: 0,
            left_only: 7,
            right_only: 6
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 4,
                side: 6,
                quantity: 1
            },
            Entry {
                key: 4,
                side: (-1),
                quantity: 1
            },
            Entry {
                key: 0,
                side: (-3),
                quantity: 4
            },
            Entry {
                key: 2,
                side: 9,
                quantity: 5
            },
            Entry {
                key: 0,
                side: (-1),
                quantity: 2
            },
            Entry {
                key: 0,
                side: 2,
                quantity: 2
            },
            Entry {
                key: 1,
                side: (-2),
                quantity: 3
            },
            Entry {
                key: 1,
                side: (-1),
                quantity: 6
            }
        ]),
        Report {
            matched: 0,
            left_only: 0,
            right_only: 0
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                side: 0,
                quantity: 2
            },
            Entry {
                key: 0,
                side: 0,
                quantity: 3
            },
            Entry {
                key: 0,
                side: 1,
                quantity: 4
            },
            Entry {
                key: 1,
                side: 2,
                quantity: 9
            }
        ]),
        Report {
            matched: 4,
            left_only: 1,
            right_only: 0
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                side: 0,
                quantity: 4
            },
            Entry {
                key: 1,
                side: 1,
                quantity: 2
            },
            Entry {
                key: 2,
                side: 1,
                quantity: 5
            },
            Entry {
                key: 1,
                side: 0,
                quantity: 1
            }
        ]),
        Report {
            matched: 2,
            left_only: 3,
            right_only: 5
        },
        "fixture 22"
    );
}
