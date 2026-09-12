include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            accepted: 0,
            rejected: 0,
            expires_sum: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            resource_id: 3,
            timestamp: 8,
            duration: 3
        }]),
        Report {
            accepted: 1,
            rejected: 0,
            expires_sum: 11
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 1,
                timestamp: 6,
                duration: 3
            },
            Entry {
                resource_id: 3,
                timestamp: 7,
                duration: 5
            }
        ]),
        Report {
            accepted: 2,
            rejected: 0,
            expires_sum: 21
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 2,
                timestamp: 11,
                duration: 1
            },
            Entry {
                resource_id: 0,
                timestamp: 0,
                duration: 6
            },
            Entry {
                resource_id: 0,
                timestamp: (-2),
                duration: 5
            }
        ]),
        Report {
            accepted: 2,
            rejected: 1,
            expires_sum: 18
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 3,
                timestamp: 10,
                duration: 2
            },
            Entry {
                resource_id: 2,
                timestamp: 2,
                duration: 3
            },
            Entry {
                resource_id: 4,
                timestamp: 0,
                duration: 4
            },
            Entry {
                resource_id: 0,
                timestamp: 11,
                duration: 2
            }
        ]),
        Report {
            accepted: 4,
            rejected: 0,
            expires_sum: 34
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 3,
                timestamp: 10,
                duration: 3
            },
            Entry {
                resource_id: 0,
                timestamp: 5,
                duration: 5
            },
            Entry {
                resource_id: 4,
                timestamp: 2,
                duration: 6
            },
            Entry {
                resource_id: 1,
                timestamp: 3,
                duration: 1
            },
            Entry {
                resource_id: 1,
                timestamp: 1,
                duration: 1
            }
        ]),
        Report {
            accepted: 4,
            rejected: 1,
            expires_sum: 35
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 3,
                timestamp: 5,
                duration: 6
            },
            Entry {
                resource_id: 1,
                timestamp: (-2),
                duration: 5
            },
            Entry {
                resource_id: 3,
                timestamp: 11,
                duration: 6
            },
            Entry {
                resource_id: 4,
                timestamp: 2,
                duration: 4
            },
            Entry {
                resource_id: 0,
                timestamp: 10,
                duration: 6
            },
            Entry {
                resource_id: 3,
                timestamp: 6,
                duration: 5
            }
        ]),
        Report {
            accepted: 4,
            rejected: 2,
            expires_sum: 39
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 1,
                timestamp: 5,
                duration: 1
            },
            Entry {
                resource_id: 1,
                timestamp: 1,
                duration: 1
            },
            Entry {
                resource_id: 3,
                timestamp: (-1),
                duration: 1
            },
            Entry {
                resource_id: 0,
                timestamp: 8,
                duration: 4
            },
            Entry {
                resource_id: 3,
                timestamp: 7,
                duration: 4
            },
            Entry {
                resource_id: 0,
                timestamp: 5,
                duration: 4
            },
            Entry {
                resource_id: 1,
                timestamp: 11,
                duration: 3
            }
        ]),
        Report {
            accepted: 4,
            rejected: 3,
            expires_sum: 37
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 0,
                timestamp: (-3),
                duration: 3
            },
            Entry {
                resource_id: 1,
                timestamp: 7,
                duration: 4
            },
            Entry {
                resource_id: 1,
                timestamp: 7,
                duration: 1
            },
            Entry {
                resource_id: 3,
                timestamp: (-2),
                duration: 3
            },
            Entry {
                resource_id: 4,
                timestamp: (-1),
                duration: 4
            },
            Entry {
                resource_id: 0,
                timestamp: 4,
                duration: 5
            },
            Entry {
                resource_id: 2,
                timestamp: 8,
                duration: 4
            },
            Entry {
                resource_id: 1,
                timestamp: (-2),
                duration: 6
            }
        ]),
        Report {
            accepted: 3,
            rejected: 5,
            expires_sum: 32
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 2,
                timestamp: 10,
                duration: 2
            },
            Entry {
                resource_id: 2,
                timestamp: 11,
                duration: 2
            },
            Entry {
                resource_id: 4,
                timestamp: 2,
                duration: 2
            },
            Entry {
                resource_id: 3,
                timestamp: 1,
                duration: 2
            },
            Entry {
                resource_id: 2,
                timestamp: 1,
                duration: 5
            },
            Entry {
                resource_id: 2,
                timestamp: 4,
                duration: 2
            },
            Entry {
                resource_id: 0,
                timestamp: 2,
                duration: 1
            },
            Entry {
                resource_id: 3,
                timestamp: (-3),
                duration: 4
            },
            Entry {
                resource_id: 4,
                timestamp: 2,
                duration: 2
            }
        ]),
        Report {
            accepted: 4,
            rejected: 5,
            expires_sum: 22
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 0,
                timestamp: 7,
                duration: 1
            },
            Entry {
                resource_id: 3,
                timestamp: (-3),
                duration: 1
            },
            Entry {
                resource_id: 1,
                timestamp: 5,
                duration: 6
            },
            Entry {
                resource_id: 0,
                timestamp: 10,
                duration: 1
            },
            Entry {
                resource_id: 1,
                timestamp: 8,
                duration: 3
            },
            Entry {
                resource_id: 3,
                timestamp: 6,
                duration: 3
            },
            Entry {
                resource_id: 0,
                timestamp: 2,
                duration: 3
            },
            Entry {
                resource_id: 4,
                timestamp: 5,
                duration: 3
            },
            Entry {
                resource_id: 2,
                timestamp: (-1),
                duration: 2
            },
            Entry {
                resource_id: 0,
                timestamp: 1,
                duration: 2
            }
        ]),
        Report {
            accepted: 5,
            rejected: 5,
            expires_sum: 39
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 2,
                timestamp: 5,
                duration: 3
            },
            Entry {
                resource_id: 4,
                timestamp: 3,
                duration: 6
            },
            Entry {
                resource_id: 3,
                timestamp: 11,
                duration: 1
            },
            Entry {
                resource_id: 1,
                timestamp: 8,
                duration: 6
            },
            Entry {
                resource_id: 3,
                timestamp: 4,
                duration: 2
            },
            Entry {
                resource_id: 0,
                timestamp: 1,
                duration: 1
            },
            Entry {
                resource_id: 4,
                timestamp: 3,
                duration: 2
            },
            Entry {
                resource_id: 1,
                timestamp: 5,
                duration: 6
            },
            Entry {
                resource_id: 4,
                timestamp: 6,
                duration: 5
            },
            Entry {
                resource_id: 1,
                timestamp: 9,
                duration: 5
            },
            Entry {
                resource_id: 4,
                timestamp: 7,
                duration: 2
            }
        ]),
        Report {
            accepted: 5,
            rejected: 6,
            expires_sum: 45
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 1,
                timestamp: 3,
                duration: 2
            },
            Entry {
                resource_id: 1,
                timestamp: 7,
                duration: 2
            },
            Entry {
                resource_id: 3,
                timestamp: 4,
                duration: 6
            },
            Entry {
                resource_id: 2,
                timestamp: 6,
                duration: 3
            },
            Entry {
                resource_id: 2,
                timestamp: 11,
                duration: 3
            },
            Entry {
                resource_id: 1,
                timestamp: 0,
                duration: 5
            },
            Entry {
                resource_id: 0,
                timestamp: 2,
                duration: 2
            },
            Entry {
                resource_id: 1,
                timestamp: 10,
                duration: 3
            },
            Entry {
                resource_id: 1,
                timestamp: (-1),
                duration: 5
            },
            Entry {
                resource_id: 0,
                timestamp: (-1),
                duration: 1
            },
            Entry {
                resource_id: 3,
                timestamp: 11,
                duration: 6
            },
            Entry {
                resource_id: 3,
                timestamp: (-3),
                duration: 4
            }
        ]),
        Report {
            accepted: 8,
            rejected: 4,
            expires_sum: 48
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            resource_id: 3,
            timestamp: (-1),
            duration: 5
        }]),
        Report {
            accepted: 0,
            rejected: 1,
            expires_sum: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 0,
                timestamp: 4,
                duration: 1
            },
            Entry {
                resource_id: 4,
                timestamp: 8,
                duration: 4
            }
        ]),
        Report {
            accepted: 2,
            rejected: 0,
            expires_sum: 17
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 3,
                timestamp: 7,
                duration: 3
            },
            Entry {
                resource_id: 0,
                timestamp: (-1),
                duration: 4
            },
            Entry {
                resource_id: 0,
                timestamp: 3,
                duration: 4
            }
        ]),
        Report {
            accepted: 2,
            rejected: 1,
            expires_sum: 17
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 1,
                timestamp: 11,
                duration: 3
            },
            Entry {
                resource_id: 0,
                timestamp: 5,
                duration: 2
            },
            Entry {
                resource_id: 3,
                timestamp: 4,
                duration: 3
            },
            Entry {
                resource_id: 0,
                timestamp: 4,
                duration: 2
            }
        ]),
        Report {
            accepted: 3,
            rejected: 1,
            expires_sum: 28
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 1,
                timestamp: 1,
                duration: 5
            },
            Entry {
                resource_id: 2,
                timestamp: 5,
                duration: 4
            },
            Entry {
                resource_id: 4,
                timestamp: (-3),
                duration: 3
            },
            Entry {
                resource_id: 2,
                timestamp: 5,
                duration: 5
            },
            Entry {
                resource_id: 1,
                timestamp: (-3),
                duration: 6
            }
        ]),
        Report {
            accepted: 2,
            rejected: 3,
            expires_sum: 15
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 0,
                timestamp: 4,
                duration: 4
            },
            Entry {
                resource_id: 2,
                timestamp: 1,
                duration: 6
            },
            Entry {
                resource_id: 3,
                timestamp: 10,
                duration: 1
            },
            Entry {
                resource_id: 4,
                timestamp: (-3),
                duration: 1
            },
            Entry {
                resource_id: 2,
                timestamp: 4,
                duration: 1
            },
            Entry {
                resource_id: 1,
                timestamp: 4,
                duration: 6
            }
        ]),
        Report {
            accepted: 4,
            rejected: 2,
            expires_sum: 36
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 2,
                timestamp: 1,
                duration: 6
            },
            Entry {
                resource_id: 0,
                timestamp: 6,
                duration: 5
            },
            Entry {
                resource_id: 1,
                timestamp: 6,
                duration: 5
            },
            Entry {
                resource_id: 0,
                timestamp: 0,
                duration: 5
            },
            Entry {
                resource_id: 1,
                timestamp: 0,
                duration: 2
            },
            Entry {
                resource_id: 2,
                timestamp: 11,
                duration: 1
            },
            Entry {
                resource_id: 4,
                timestamp: (-1),
                duration: 4
            }
        ]),
        Report {
            accepted: 4,
            rejected: 3,
            expires_sum: 34
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 4,
                timestamp: 6,
                duration: 1
            },
            Entry {
                resource_id: 4,
                timestamp: (-1),
                duration: 1
            },
            Entry {
                resource_id: 0,
                timestamp: (-3),
                duration: 4
            },
            Entry {
                resource_id: 2,
                timestamp: 9,
                duration: 5
            },
            Entry {
                resource_id: 0,
                timestamp: (-1),
                duration: 2
            },
            Entry {
                resource_id: 0,
                timestamp: 2,
                duration: 2
            },
            Entry {
                resource_id: 1,
                timestamp: (-2),
                duration: 3
            },
            Entry {
                resource_id: 1,
                timestamp: (-1),
                duration: 6
            }
        ]),
        Report {
            accepted: 3,
            rejected: 5,
            expires_sum: 25
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 0,
                timestamp: 0,
                duration: 0
            },
            Entry {
                resource_id: 0,
                timestamp: 0,
                duration: 2
            },
            Entry {
                resource_id: 0,
                timestamp: 2,
                duration: 3
            },
            Entry {
                resource_id: 0,
                timestamp: 1,
                duration: 9
            }
        ]),
        Report {
            accepted: 2,
            rejected: 2,
            expires_sum: 5
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                resource_id: 1,
                timestamp: 0,
                duration: 3
            },
            Entry {
                resource_id: 1,
                timestamp: 2,
                duration: 7
            },
            Entry {
                resource_id: 1,
                timestamp: 3,
                duration: 2
            },
            Entry {
                resource_id: 2,
                timestamp: (-1),
                duration: 4
            }
        ]),
        Report {
            accepted: 2,
            rejected: 2,
            expires_sum: 5
        },
        "fixture 22"
    );
}
