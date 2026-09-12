include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            applied: 0,
            duplicates: 0,
            checksum: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            event: 3,
            key: 2,
            delta: 1
        }]),
        Report {
            applied: 1,
            duplicates: 0,
            checksum: 3
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 4,
                key: 2,
                delta: 4
            },
            Entry {
                event: 4,
                key: 2,
                delta: (-2)
            }
        ]),
        Report {
            applied: 1,
            duplicates: 1,
            checksum: 12
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 0,
                key: 1,
                delta: 8
            },
            Entry {
                event: 0,
                key: 0,
                delta: 6
            },
            Entry {
                event: 3,
                key: 1,
                delta: 3
            }
        ]),
        Report {
            applied: 2,
            duplicates: 1,
            checksum: 22
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 2,
                key: 2,
                delta: 7
            },
            Entry {
                event: 1,
                key: 3,
                delta: (-1)
            },
            Entry {
                event: 1,
                key: 3,
                delta: 3
            },
            Entry {
                event: 0,
                key: 2,
                delta: 9
            }
        ]),
        Report {
            applied: 3,
            duplicates: 1,
            checksum: 44
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 1,
                key: 3,
                delta: (-2)
            },
            Entry {
                event: 1,
                key: 2,
                delta: (-1)
            },
            Entry {
                event: 3,
                key: 1,
                delta: (-1)
            },
            Entry {
                event: 4,
                key: 3,
                delta: 9
            },
            Entry {
                event: 4,
                key: 2,
                delta: 5
            }
        ]),
        Report {
            applied: 3,
            duplicates: 2,
            checksum: 26
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 0,
                key: 3,
                delta: 7
            },
            Entry {
                event: 4,
                key: 1,
                delta: 6
            },
            Entry {
                event: 0,
                key: 1,
                delta: 2
            },
            Entry {
                event: 0,
                key: 3,
                delta: 0
            },
            Entry {
                event: 0,
                key: 0,
                delta: 9
            },
            Entry {
                event: 3,
                key: 3,
                delta: 8
            }
        ]),
        Report {
            applied: 3,
            duplicates: 3,
            checksum: 72
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 3,
                key: 0,
                delta: 6
            },
            Entry {
                event: 3,
                key: 1,
                delta: 3
            },
            Entry {
                event: 0,
                key: 0,
                delta: 2
            },
            Entry {
                event: 1,
                key: 3,
                delta: 0
            },
            Entry {
                event: 0,
                key: 3,
                delta: (-1)
            },
            Entry {
                event: 2,
                key: 1,
                delta: 4
            },
            Entry {
                event: 0,
                key: 3,
                delta: 7
            }
        ]),
        Report {
            applied: 4,
            duplicates: 3,
            checksum: 16
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 2,
                key: 3,
                delta: 0
            },
            Entry {
                event: 0,
                key: 2,
                delta: 1
            },
            Entry {
                event: 2,
                key: 1,
                delta: 7
            },
            Entry {
                event: 2,
                key: 1,
                delta: 4
            },
            Entry {
                event: 2,
                key: 1,
                delta: 2
            },
            Entry {
                event: 2,
                key: 2,
                delta: 5
            },
            Entry {
                event: 1,
                key: 0,
                delta: 3
            },
            Entry {
                event: 0,
                key: 3,
                delta: (-2)
            }
        ]),
        Report {
            applied: 3,
            duplicates: 5,
            checksum: 6
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 3,
                key: 2,
                delta: 1
            },
            Entry {
                event: 0,
                key: 0,
                delta: 5
            },
            Entry {
                event: 0,
                key: 0,
                delta: 0
            },
            Entry {
                event: 4,
                key: 0,
                delta: (-1)
            },
            Entry {
                event: 1,
                key: 2,
                delta: 4
            },
            Entry {
                event: 4,
                key: 2,
                delta: 9
            },
            Entry {
                event: 0,
                key: 2,
                delta: 2
            },
            Entry {
                event: 4,
                key: 2,
                delta: 3
            },
            Entry {
                event: 1,
                key: 1,
                delta: (-1)
            }
        ]),
        Report {
            applied: 4,
            duplicates: 5,
            checksum: 19
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 2,
                key: 1,
                delta: 3
            },
            Entry {
                event: 4,
                key: 2,
                delta: 6
            },
            Entry {
                event: 3,
                key: 3,
                delta: (-2)
            },
            Entry {
                event: 1,
                key: 3,
                delta: 5
            },
            Entry {
                event: 1,
                key: 0,
                delta: 2
            },
            Entry {
                event: 0,
                key: 3,
                delta: 1
            },
            Entry {
                event: 1,
                key: 1,
                delta: 7
            },
            Entry {
                event: 4,
                key: 1,
                delta: 1
            },
            Entry {
                event: 3,
                key: 1,
                delta: 0
            },
            Entry {
                event: 1,
                key: 3,
                delta: 5
            }
        ]),
        Report {
            applied: 5,
            duplicates: 5,
            checksum: 40
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 2,
                key: 2,
                delta: 2
            },
            Entry {
                event: 2,
                key: 1,
                delta: 1
            },
            Entry {
                event: 4,
                key: 0,
                delta: 3
            },
            Entry {
                event: 1,
                key: 1,
                delta: 2
            },
            Entry {
                event: 1,
                key: 1,
                delta: 7
            },
            Entry {
                event: 0,
                key: 1,
                delta: (-1)
            },
            Entry {
                event: 3,
                key: 3,
                delta: (-2)
            },
            Entry {
                event: 3,
                key: 3,
                delta: 0
            },
            Entry {
                event: 4,
                key: 0,
                delta: 5
            },
            Entry {
                event: 0,
                key: 3,
                delta: 4
            },
            Entry {
                event: 2,
                key: 0,
                delta: 0
            }
        ]),
        Report {
            applied: 5,
            duplicates: 6,
            checksum: 3
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 3,
                key: 0,
                delta: 4
            },
            Entry {
                event: 3,
                key: 1,
                delta: 3
            },
            Entry {
                event: 0,
                key: 1,
                delta: 5
            },
            Entry {
                event: 3,
                key: 2,
                delta: (-2)
            },
            Entry {
                event: 3,
                key: 1,
                delta: 1
            },
            Entry {
                event: 2,
                key: 2,
                delta: 6
            },
            Entry {
                event: 3,
                key: 0,
                delta: 2
            },
            Entry {
                event: 2,
                key: 1,
                delta: (-2)
            },
            Entry {
                event: 0,
                key: 3,
                delta: 4
            },
            Entry {
                event: 2,
                key: 2,
                delta: 8
            },
            Entry {
                event: 3,
                key: 0,
                delta: 6
            },
            Entry {
                event: 0,
                key: 0,
                delta: 3
            }
        ]),
        Report {
            applied: 3,
            duplicates: 9,
            checksum: 32
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            event: 3,
            key: 0,
            delta: 1
        }]),
        Report {
            applied: 1,
            duplicates: 0,
            checksum: 1
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 3,
                key: 2,
                delta: 2
            },
            Entry {
                event: 0,
                key: 1,
                delta: 7
            }
        ]),
        Report {
            applied: 2,
            duplicates: 0,
            checksum: 20
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 4,
                key: 0,
                delta: 1
            },
            Entry {
                event: 4,
                key: 1,
                delta: 1
            },
            Entry {
                event: 1,
                key: 2,
                delta: (-2)
            }
        ]),
        Report {
            applied: 2,
            duplicates: 1,
            checksum: (-5)
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 4,
                key: 1,
                delta: 5
            },
            Entry {
                event: 4,
                key: 0,
                delta: 6
            },
            Entry {
                event: 1,
                key: 0,
                delta: 8
            },
            Entry {
                event: 0,
                key: 0,
                delta: 5
            }
        ]),
        Report {
            applied: 3,
            duplicates: 1,
            checksum: 23
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 2,
                key: 0,
                delta: 0
            },
            Entry {
                event: 1,
                key: 0,
                delta: 3
            },
            Entry {
                event: 1,
                key: 1,
                delta: (-1)
            },
            Entry {
                event: 2,
                key: 1,
                delta: 0
            },
            Entry {
                event: 1,
                key: 2,
                delta: 2
            }
        ]),
        Report {
            applied: 2,
            duplicates: 3,
            checksum: 3
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 4,
                key: 3,
                delta: (-2)
            },
            Entry {
                event: 1,
                key: 3,
                delta: 6
            },
            Entry {
                event: 3,
                key: 0,
                delta: 2
            },
            Entry {
                event: 4,
                key: 1,
                delta: (-2)
            },
            Entry {
                event: 4,
                key: 3,
                delta: (-2)
            },
            Entry {
                event: 3,
                key: 0,
                delta: 0
            }
        ]),
        Report {
            applied: 3,
            duplicates: 3,
            checksum: 18
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 1,
                key: 0,
                delta: 5
            },
            Entry {
                event: 3,
                key: 2,
                delta: (-1)
            },
            Entry {
                event: 3,
                key: 3,
                delta: 5
            },
            Entry {
                event: 0,
                key: 3,
                delta: (-2)
            },
            Entry {
                event: 4,
                key: 1,
                delta: (-1)
            },
            Entry {
                event: 1,
                key: 0,
                delta: 3
            },
            Entry {
                event: 2,
                key: 3,
                delta: (-1)
            }
        ]),
        Report {
            applied: 5,
            duplicates: 2,
            checksum: (-12)
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 2,
                key: 3,
                delta: (-1)
            },
            Entry {
                event: 2,
                key: 0,
                delta: (-1)
            },
            Entry {
                event: 2,
                key: 2,
                delta: 5
            },
            Entry {
                event: 1,
                key: 3,
                delta: 3
            },
            Entry {
                event: 4,
                key: 2,
                delta: 6
            },
            Entry {
                event: 3,
                key: 0,
                delta: 1
            },
            Entry {
                event: 3,
                key: 0,
                delta: 1
            },
            Entry {
                event: 3,
                key: 0,
                delta: 9
            }
        ]),
        Report {
            applied: 4,
            duplicates: 4,
            checksum: 27
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 0,
                key: 0,
                delta: 0
            },
            Entry {
                event: 0,
                key: 1,
                delta: 99
            },
            Entry {
                event: 1,
                key: 0,
                delta: (-1)
            }
        ]),
        Report {
            applied: 2,
            duplicates: 1,
            checksum: (-1)
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                event: 1,
                key: 0,
                delta: 5
            },
            Entry {
                event: 2,
                key: 1,
                delta: (-2)
            },
            Entry {
                event: 1,
                key: 9,
                delta: 9
            },
            Entry {
                event: 3,
                key: 0,
                delta: 1
            }
        ]),
        Report {
            applied: 3,
            duplicates: 1,
            checksum: 2
        },
        "fixture 22"
    );
}
