include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            completed: 0,
            active: 0,
            rejected: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            sensor: 3,
            operation: 2,
            timestamp: 1
        }]),
        Report {
            completed: 0,
            active: 0,
            rejected: 1
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: 4
            },
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: (-2)
            }
        ]),
        Report {
            completed: 0,
            active: 0,
            rejected: 2
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 0,
                operation: 1,
                timestamp: 8
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 6
            },
            Entry {
                sensor: 3,
                operation: 1,
                timestamp: 3
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 2
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 2,
                operation: 2,
                timestamp: 7
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: (-1)
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: 3
            },
            Entry {
                sensor: 0,
                operation: 2,
                timestamp: 9
            }
        ]),
        Report {
            completed: 0,
            active: 0,
            rejected: 4
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: (-2)
            },
            Entry {
                sensor: 1,
                operation: 2,
                timestamp: (-1)
            },
            Entry {
                sensor: 3,
                operation: 1,
                timestamp: (-1)
            },
            Entry {
                sensor: 4,
                operation: 3,
                timestamp: 9
            },
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: 5
            }
        ]),
        Report {
            completed: 0,
            active: 0,
            rejected: 5
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: 7
            },
            Entry {
                sensor: 4,
                operation: 1,
                timestamp: 6
            },
            Entry {
                sensor: 0,
                operation: 1,
                timestamp: 2
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: 0
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 9
            },
            Entry {
                sensor: 3,
                operation: 3,
                timestamp: 8
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 5
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 6
            },
            Entry {
                sensor: 3,
                operation: 1,
                timestamp: 3
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 2
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: 0
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: (-1)
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 4
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: 7
            }
        ]),
        Report {
            completed: 0,
            active: 2,
            rejected: 5
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 2,
                operation: 3,
                timestamp: 0
            },
            Entry {
                sensor: 0,
                operation: 2,
                timestamp: 1
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 7
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 4
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 2
            },
            Entry {
                sensor: 2,
                operation: 2,
                timestamp: 5
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 3
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: (-2)
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 7
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 3,
                operation: 2,
                timestamp: 1
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 5
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 0
            },
            Entry {
                sensor: 4,
                operation: 0,
                timestamp: (-1)
            },
            Entry {
                sensor: 1,
                operation: 2,
                timestamp: 4
            },
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: 9
            },
            Entry {
                sensor: 0,
                operation: 2,
                timestamp: 2
            },
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: 3
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: (-1)
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 8
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 3
            },
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: 6
            },
            Entry {
                sensor: 3,
                operation: 3,
                timestamp: (-2)
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: 5
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 2
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: 1
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 7
            },
            Entry {
                sensor: 4,
                operation: 1,
                timestamp: 1
            },
            Entry {
                sensor: 3,
                operation: 1,
                timestamp: 0
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: 5
            }
        ]),
        Report {
            completed: 1,
            active: 0,
            rejected: 8
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 2,
                operation: 2,
                timestamp: 2
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 1
            },
            Entry {
                sensor: 4,
                operation: 0,
                timestamp: 3
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 2
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 7
            },
            Entry {
                sensor: 0,
                operation: 1,
                timestamp: (-1)
            },
            Entry {
                sensor: 3,
                operation: 3,
                timestamp: (-2)
            },
            Entry {
                sensor: 3,
                operation: 3,
                timestamp: 0
            },
            Entry {
                sensor: 4,
                operation: 0,
                timestamp: 5
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: 4
            },
            Entry {
                sensor: 2,
                operation: 0,
                timestamp: 0
            }
        ]),
        Report {
            completed: 0,
            active: 2,
            rejected: 9
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 4
            },
            Entry {
                sensor: 3,
                operation: 1,
                timestamp: 3
            },
            Entry {
                sensor: 0,
                operation: 1,
                timestamp: 5
            },
            Entry {
                sensor: 3,
                operation: 2,
                timestamp: (-2)
            },
            Entry {
                sensor: 3,
                operation: 1,
                timestamp: 1
            },
            Entry {
                sensor: 2,
                operation: 2,
                timestamp: 6
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 2
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: (-2)
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: 4
            },
            Entry {
                sensor: 2,
                operation: 2,
                timestamp: 8
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 6
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 3
            }
        ]),
        Report {
            completed: 0,
            active: 2,
            rejected: 10
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            sensor: 3,
            operation: 0,
            timestamp: 1
        }]),
        Report {
            completed: 0,
            active: 1,
            rejected: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 3,
                operation: 2,
                timestamp: 2
            },
            Entry {
                sensor: 0,
                operation: 1,
                timestamp: 7
            }
        ]),
        Report {
            completed: 0,
            active: 0,
            rejected: 2
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 4,
                operation: 0,
                timestamp: 1
            },
            Entry {
                sensor: 4,
                operation: 1,
                timestamp: 1
            },
            Entry {
                sensor: 1,
                operation: 2,
                timestamp: (-2)
            }
        ]),
        Report {
            completed: 1,
            active: 0,
            rejected: 1
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 4,
                operation: 1,
                timestamp: 5
            },
            Entry {
                sensor: 4,
                operation: 0,
                timestamp: 6
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 8
            },
            Entry {
                sensor: 0,
                operation: 0,
                timestamp: 5
            }
        ]),
        Report {
            completed: 0,
            active: 3,
            rejected: 1
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 2,
                operation: 0,
                timestamp: 0
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 3
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: (-1)
            },
            Entry {
                sensor: 2,
                operation: 1,
                timestamp: 0
            },
            Entry {
                sensor: 1,
                operation: 2,
                timestamp: 2
            }
        ]),
        Report {
            completed: 1,
            active: 1,
            rejected: 2
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 4,
                operation: 3,
                timestamp: (-2)
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: 6
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 2
            },
            Entry {
                sensor: 4,
                operation: 1,
                timestamp: (-2)
            },
            Entry {
                sensor: 4,
                operation: 3,
                timestamp: (-2)
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 0
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 5
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 5
            },
            Entry {
                sensor: 3,
                operation: 2,
                timestamp: (-1)
            },
            Entry {
                sensor: 3,
                operation: 3,
                timestamp: 5
            },
            Entry {
                sensor: 0,
                operation: 3,
                timestamp: (-2)
            },
            Entry {
                sensor: 4,
                operation: 1,
                timestamp: (-1)
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 3
            },
            Entry {
                sensor: 2,
                operation: 3,
                timestamp: (-1)
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 6
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 2,
                operation: 3,
                timestamp: (-1)
            },
            Entry {
                sensor: 2,
                operation: 0,
                timestamp: (-1)
            },
            Entry {
                sensor: 2,
                operation: 2,
                timestamp: 5
            },
            Entry {
                sensor: 1,
                operation: 3,
                timestamp: 3
            },
            Entry {
                sensor: 4,
                operation: 2,
                timestamp: 6
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 1
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 1
            },
            Entry {
                sensor: 3,
                operation: 0,
                timestamp: 9
            }
        ]),
        Report {
            completed: 0,
            active: 1,
            rejected: 7
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 0
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 0
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 1
            }
        ]),
        Report {
            completed: 1,
            active: 0,
            rejected: 1
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: (-1)
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 2
            },
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 3
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 1
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 2
            }
        ]),
        Report {
            completed: 1,
            active: 0,
            rejected: 3
        },
        "fixture 22"
    );
    assert_eq!(
        solve(&[
            Entry {
                sensor: 1,
                operation: 0,
                timestamp: 2
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 1
            },
            Entry {
                sensor: 1,
                operation: 1,
                timestamp: 3
            },
            Entry {
                sensor: 2,
                operation: 0,
                timestamp: 0
            }
        ]),
        Report {
            completed: 1,
            active: 1,
            rejected: 1
        },
        "fixture 23"
    );
}
