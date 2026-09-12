include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            accepted: 0,
            stale: 0,
            checksum: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            key: 3,
            version: 8,
            value: 3
        }]),
        Report {
            accepted: 1,
            stale: 0,
            checksum: 12
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                version: 6,
                value: 3
            },
            Entry {
                key: 3,
                version: 7,
                value: 5
            }
        ]),
        Report {
            accepted: 2,
            stale: 0,
            checksum: 26
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                version: 11,
                value: 1
            },
            Entry {
                key: 0,
                version: 0,
                value: 6
            },
            Entry {
                key: 0,
                version: (-2),
                value: 5
            }
        ]),
        Report {
            accepted: 2,
            stale: 1,
            checksum: 9
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                version: 10,
                value: 2
            },
            Entry {
                key: 2,
                version: 2,
                value: 3
            },
            Entry {
                key: 4,
                version: 0,
                value: 4
            },
            Entry {
                key: 0,
                version: 11,
                value: 2
            }
        ]),
        Report {
            accepted: 4,
            stale: 0,
            checksum: 39
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                version: 10,
                value: 3
            },
            Entry {
                key: 0,
                version: 5,
                value: 5
            },
            Entry {
                key: 4,
                version: 2,
                value: 6
            },
            Entry {
                key: 1,
                version: 3,
                value: 1
            },
            Entry {
                key: 1,
                version: 1,
                value: 1
            }
        ]),
        Report {
            accepted: 4,
            stale: 1,
            checksum: 49
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                version: 5,
                value: 6
            },
            Entry {
                key: 1,
                version: (-2),
                value: 5
            },
            Entry {
                key: 3,
                version: 11,
                value: 6
            },
            Entry {
                key: 4,
                version: 2,
                value: 4
            },
            Entry {
                key: 0,
                version: 10,
                value: 6
            },
            Entry {
                key: 3,
                version: 6,
                value: 5
            }
        ]),
        Report {
            accepted: 4,
            stale: 2,
            checksum: 50
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                version: 5,
                value: 1
            },
            Entry {
                key: 1,
                version: 1,
                value: 1
            },
            Entry {
                key: 3,
                version: (-1),
                value: 1
            },
            Entry {
                key: 0,
                version: 8,
                value: 4
            },
            Entry {
                key: 3,
                version: 7,
                value: 4
            },
            Entry {
                key: 0,
                version: 5,
                value: 4
            },
            Entry {
                key: 1,
                version: 11,
                value: 3
            }
        ]),
        Report {
            accepted: 4,
            stale: 3,
            checksum: 26
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                version: (-3),
                value: 3
            },
            Entry {
                key: 1,
                version: 7,
                value: 4
            },
            Entry {
                key: 1,
                version: 7,
                value: 1
            },
            Entry {
                key: 3,
                version: (-2),
                value: 3
            },
            Entry {
                key: 4,
                version: (-1),
                value: 4
            },
            Entry {
                key: 0,
                version: 4,
                value: 5
            },
            Entry {
                key: 2,
                version: 8,
                value: 4
            },
            Entry {
                key: 1,
                version: (-2),
                value: 6
            }
        ]),
        Report {
            accepted: 3,
            stale: 5,
            checksum: 25
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                version: 10,
                value: 2
            },
            Entry {
                key: 2,
                version: 11,
                value: 2
            },
            Entry {
                key: 4,
                version: 2,
                value: 2
            },
            Entry {
                key: 3,
                version: 1,
                value: 2
            },
            Entry {
                key: 2,
                version: 1,
                value: 5
            },
            Entry {
                key: 2,
                version: 4,
                value: 2
            },
            Entry {
                key: 0,
                version: 2,
                value: 1
            },
            Entry {
                key: 3,
                version: (-3),
                value: 4
            },
            Entry {
                key: 4,
                version: 2,
                value: 2
            }
        ]),
        Report {
            accepted: 5,
            stale: 4,
            checksum: 25
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                version: 7,
                value: 1
            },
            Entry {
                key: 3,
                version: (-3),
                value: 1
            },
            Entry {
                key: 1,
                version: 5,
                value: 6
            },
            Entry {
                key: 0,
                version: 10,
                value: 1
            },
            Entry {
                key: 1,
                version: 8,
                value: 3
            },
            Entry {
                key: 3,
                version: 6,
                value: 3
            },
            Entry {
                key: 0,
                version: 2,
                value: 3
            },
            Entry {
                key: 4,
                version: 5,
                value: 3
            },
            Entry {
                key: 2,
                version: (-1),
                value: 2
            },
            Entry {
                key: 0,
                version: 1,
                value: 2
            }
        ]),
        Report {
            accepted: 6,
            stale: 4,
            checksum: 34
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                version: 5,
                value: 3
            },
            Entry {
                key: 4,
                version: 3,
                value: 6
            },
            Entry {
                key: 3,
                version: 11,
                value: 1
            },
            Entry {
                key: 1,
                version: 8,
                value: 6
            },
            Entry {
                key: 3,
                version: 4,
                value: 2
            },
            Entry {
                key: 0,
                version: 1,
                value: 1
            },
            Entry {
                key: 4,
                version: 3,
                value: 2
            },
            Entry {
                key: 1,
                version: 5,
                value: 6
            },
            Entry {
                key: 4,
                version: 6,
                value: 5
            },
            Entry {
                key: 1,
                version: 9,
                value: 5
            },
            Entry {
                key: 4,
                version: 7,
                value: 2
            }
        ]),
        Report {
            accepted: 8,
            stale: 3,
            checksum: 34
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                version: 3,
                value: 2
            },
            Entry {
                key: 1,
                version: 7,
                value: 2
            },
            Entry {
                key: 3,
                version: 4,
                value: 6
            },
            Entry {
                key: 2,
                version: 6,
                value: 3
            },
            Entry {
                key: 2,
                version: 11,
                value: 3
            },
            Entry {
                key: 1,
                version: 0,
                value: 5
            },
            Entry {
                key: 0,
                version: 2,
                value: 2
            },
            Entry {
                key: 1,
                version: 10,
                value: 3
            },
            Entry {
                key: 1,
                version: (-1),
                value: 5
            },
            Entry {
                key: 0,
                version: (-1),
                value: 1
            },
            Entry {
                key: 3,
                version: 11,
                value: 6
            },
            Entry {
                key: 3,
                version: (-3),
                value: 4
            }
        ]),
        Report {
            accepted: 8,
            stale: 4,
            checksum: 41
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            key: 3,
            version: (-1),
            value: 5
        }]),
        Report {
            accepted: 0,
            stale: 1,
            checksum: 0
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                version: 4,
                value: 1
            },
            Entry {
                key: 4,
                version: 8,
                value: 4
            }
        ]),
        Report {
            accepted: 2,
            stale: 0,
            checksum: 21
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 3,
                version: 7,
                value: 3
            },
            Entry {
                key: 0,
                version: (-1),
                value: 4
            },
            Entry {
                key: 0,
                version: 3,
                value: 4
            }
        ]),
        Report {
            accepted: 2,
            stale: 1,
            checksum: 16
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                version: 11,
                value: 3
            },
            Entry {
                key: 0,
                version: 5,
                value: 2
            },
            Entry {
                key: 3,
                version: 4,
                value: 3
            },
            Entry {
                key: 0,
                version: 4,
                value: 2
            }
        ]),
        Report {
            accepted: 3,
            stale: 1,
            checksum: 20
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                version: 1,
                value: 5
            },
            Entry {
                key: 2,
                version: 5,
                value: 4
            },
            Entry {
                key: 4,
                version: (-3),
                value: 3
            },
            Entry {
                key: 2,
                version: 5,
                value: 5
            },
            Entry {
                key: 1,
                version: (-3),
                value: 6
            }
        ]),
        Report {
            accepted: 2,
            stale: 3,
            checksum: 22
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                version: 4,
                value: 4
            },
            Entry {
                key: 2,
                version: 1,
                value: 6
            },
            Entry {
                key: 3,
                version: 10,
                value: 1
            },
            Entry {
                key: 4,
                version: (-3),
                value: 1
            },
            Entry {
                key: 2,
                version: 4,
                value: 1
            },
            Entry {
                key: 1,
                version: 4,
                value: 6
            }
        ]),
        Report {
            accepted: 5,
            stale: 1,
            checksum: 23
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 2,
                version: 1,
                value: 6
            },
            Entry {
                key: 0,
                version: 6,
                value: 5
            },
            Entry {
                key: 1,
                version: 6,
                value: 5
            },
            Entry {
                key: 0,
                version: 0,
                value: 5
            },
            Entry {
                key: 1,
                version: 0,
                value: 2
            },
            Entry {
                key: 2,
                version: 11,
                value: 1
            },
            Entry {
                key: 4,
                version: (-1),
                value: 4
            }
        ]),
        Report {
            accepted: 4,
            stale: 3,
            checksum: 18
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 4,
                version: 6,
                value: 1
            },
            Entry {
                key: 4,
                version: (-1),
                value: 1
            },
            Entry {
                key: 0,
                version: (-3),
                value: 4
            },
            Entry {
                key: 2,
                version: 9,
                value: 5
            },
            Entry {
                key: 0,
                version: (-1),
                value: 2
            },
            Entry {
                key: 0,
                version: 2,
                value: 2
            },
            Entry {
                key: 1,
                version: (-2),
                value: 3
            },
            Entry {
                key: 1,
                version: (-1),
                value: 6
            }
        ]),
        Report {
            accepted: 3,
            stale: 5,
            checksum: 22
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                version: 0,
                value: 3
            },
            Entry {
                key: 0,
                version: 0,
                value: 9
            }
        ]),
        Report {
            accepted: 1,
            stale: 1,
            checksum: 3
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 0,
                version: (-1),
                value: 8
            },
            Entry {
                key: 0,
                version: (-2),
                value: 9
            },
            Entry {
                key: 0,
                version: 0,
                value: 0
            }
        ]),
        Report {
            accepted: 1,
            stale: 2,
            checksum: 0
        },
        "fixture 22"
    );
    assert_eq!(
        solve(&[
            Entry {
                key: 1,
                version: 0,
                value: 8
            },
            Entry {
                key: 1,
                version: 0,
                value: 9
            },
            Entry {
                key: 1,
                version: 2,
                value: (-3)
            },
            Entry {
                key: 2,
                version: (-1),
                value: 5
            }
        ]),
        Report {
            accepted: 2,
            stale: 2,
            checksum: (-6)
        },
        "fixture 23"
    );
}
