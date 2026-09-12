include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[]),
        Report {
            settled: 0,
            outstanding: 0,
            credit: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[Entry {
            invoice: 3,
            amount: 8
        }]),
        Report {
            settled: 0,
            outstanding: 8,
            credit: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: 6
            },
            Entry {
                invoice: 3,
                amount: 7
            }
        ]),
        Report {
            settled: 0,
            outstanding: 13,
            credit: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 2,
                amount: 11
            },
            Entry {
                invoice: 0,
                amount: 0
            },
            Entry {
                invoice: 0,
                amount: (-2)
            }
        ]),
        Report {
            settled: 0,
            outstanding: 11,
            credit: 2
        },
        "fixture 3"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 3,
                amount: 10
            },
            Entry {
                invoice: 2,
                amount: 2
            },
            Entry {
                invoice: 4,
                amount: 0
            },
            Entry {
                invoice: 0,
                amount: 11
            }
        ]),
        Report {
            settled: 1,
            outstanding: 23,
            credit: 0
        },
        "fixture 4"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 3,
                amount: 10
            },
            Entry {
                invoice: 0,
                amount: 5
            },
            Entry {
                invoice: 4,
                amount: 2
            },
            Entry {
                invoice: 1,
                amount: 3
            },
            Entry {
                invoice: 1,
                amount: 1
            }
        ]),
        Report {
            settled: 0,
            outstanding: 21,
            credit: 0
        },
        "fixture 5"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 3,
                amount: 5
            },
            Entry {
                invoice: 1,
                amount: (-2)
            },
            Entry {
                invoice: 3,
                amount: 11
            },
            Entry {
                invoice: 4,
                amount: 2
            },
            Entry {
                invoice: 0,
                amount: 10
            },
            Entry {
                invoice: 3,
                amount: 6
            }
        ]),
        Report {
            settled: 0,
            outstanding: 34,
            credit: 2
        },
        "fixture 6"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: 5
            },
            Entry {
                invoice: 1,
                amount: 1
            },
            Entry {
                invoice: 3,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: 8
            },
            Entry {
                invoice: 3,
                amount: 7
            },
            Entry {
                invoice: 0,
                amount: 5
            },
            Entry {
                invoice: 1,
                amount: 11
            }
        ]),
        Report {
            settled: 0,
            outstanding: 36,
            credit: 0
        },
        "fixture 7"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 0,
                amount: (-3)
            },
            Entry {
                invoice: 1,
                amount: 7
            },
            Entry {
                invoice: 1,
                amount: 7
            },
            Entry {
                invoice: 3,
                amount: (-2)
            },
            Entry {
                invoice: 4,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: 4
            },
            Entry {
                invoice: 2,
                amount: 8
            },
            Entry {
                invoice: 1,
                amount: (-2)
            }
        ]),
        Report {
            settled: 0,
            outstanding: 21,
            credit: 3
        },
        "fixture 8"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 2,
                amount: 10
            },
            Entry {
                invoice: 2,
                amount: 11
            },
            Entry {
                invoice: 4,
                amount: 2
            },
            Entry {
                invoice: 3,
                amount: 1
            },
            Entry {
                invoice: 2,
                amount: 1
            },
            Entry {
                invoice: 2,
                amount: 4
            },
            Entry {
                invoice: 0,
                amount: 2
            },
            Entry {
                invoice: 3,
                amount: (-3)
            },
            Entry {
                invoice: 4,
                amount: 2
            }
        ]),
        Report {
            settled: 0,
            outstanding: 32,
            credit: 2
        },
        "fixture 9"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 0,
                amount: 7
            },
            Entry {
                invoice: 3,
                amount: (-3)
            },
            Entry {
                invoice: 1,
                amount: 5
            },
            Entry {
                invoice: 0,
                amount: 10
            },
            Entry {
                invoice: 1,
                amount: 8
            },
            Entry {
                invoice: 3,
                amount: 6
            },
            Entry {
                invoice: 0,
                amount: 2
            },
            Entry {
                invoice: 4,
                amount: 5
            },
            Entry {
                invoice: 2,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: 1
            }
        ]),
        Report {
            settled: 0,
            outstanding: 41,
            credit: 1
        },
        "fixture 10"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 2,
                amount: 5
            },
            Entry {
                invoice: 4,
                amount: 3
            },
            Entry {
                invoice: 3,
                amount: 11
            },
            Entry {
                invoice: 1,
                amount: 8
            },
            Entry {
                invoice: 3,
                amount: 4
            },
            Entry {
                invoice: 0,
                amount: 1
            },
            Entry {
                invoice: 4,
                amount: 3
            },
            Entry {
                invoice: 1,
                amount: 5
            },
            Entry {
                invoice: 4,
                amount: 6
            },
            Entry {
                invoice: 1,
                amount: 9
            },
            Entry {
                invoice: 4,
                amount: 7
            }
        ]),
        Report {
            settled: 0,
            outstanding: 62,
            credit: 0
        },
        "fixture 11"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: 3
            },
            Entry {
                invoice: 1,
                amount: 7
            },
            Entry {
                invoice: 3,
                amount: 4
            },
            Entry {
                invoice: 2,
                amount: 6
            },
            Entry {
                invoice: 2,
                amount: 11
            },
            Entry {
                invoice: 1,
                amount: 0
            },
            Entry {
                invoice: 0,
                amount: 2
            },
            Entry {
                invoice: 1,
                amount: 10
            },
            Entry {
                invoice: 1,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: (-1)
            },
            Entry {
                invoice: 3,
                amount: 11
            },
            Entry {
                invoice: 3,
                amount: (-3)
            }
        ]),
        Report {
            settled: 0,
            outstanding: 49,
            credit: 0
        },
        "fixture 12"
    );
    assert_eq!(
        solve(&[Entry {
            invoice: 3,
            amount: (-1)
        }]),
        Report {
            settled: 0,
            outstanding: 0,
            credit: 1
        },
        "fixture 13"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 0,
                amount: 4
            },
            Entry {
                invoice: 4,
                amount: 8
            }
        ]),
        Report {
            settled: 0,
            outstanding: 12,
            credit: 0
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 3,
                amount: 7
            },
            Entry {
                invoice: 0,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: 3
            }
        ]),
        Report {
            settled: 0,
            outstanding: 9,
            credit: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: 11
            },
            Entry {
                invoice: 0,
                amount: 5
            },
            Entry {
                invoice: 3,
                amount: 4
            },
            Entry {
                invoice: 0,
                amount: 4
            }
        ]),
        Report {
            settled: 0,
            outstanding: 24,
            credit: 0
        },
        "fixture 16"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: 1
            },
            Entry {
                invoice: 2,
                amount: 5
            },
            Entry {
                invoice: 4,
                amount: (-3)
            },
            Entry {
                invoice: 2,
                amount: 5
            },
            Entry {
                invoice: 1,
                amount: (-3)
            }
        ]),
        Report {
            settled: 0,
            outstanding: 10,
            credit: 5
        },
        "fixture 17"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 0,
                amount: 4
            },
            Entry {
                invoice: 2,
                amount: 1
            },
            Entry {
                invoice: 3,
                amount: 10
            },
            Entry {
                invoice: 4,
                amount: (-3)
            },
            Entry {
                invoice: 2,
                amount: 4
            },
            Entry {
                invoice: 1,
                amount: 4
            }
        ]),
        Report {
            settled: 0,
            outstanding: 23,
            credit: 3
        },
        "fixture 18"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 2,
                amount: 1
            },
            Entry {
                invoice: 0,
                amount: 6
            },
            Entry {
                invoice: 1,
                amount: 6
            },
            Entry {
                invoice: 0,
                amount: 0
            },
            Entry {
                invoice: 1,
                amount: 0
            },
            Entry {
                invoice: 2,
                amount: 11
            },
            Entry {
                invoice: 4,
                amount: (-1)
            }
        ]),
        Report {
            settled: 0,
            outstanding: 24,
            credit: 1
        },
        "fixture 19"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 4,
                amount: 6
            },
            Entry {
                invoice: 4,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: (-3)
            },
            Entry {
                invoice: 2,
                amount: 9
            },
            Entry {
                invoice: 0,
                amount: (-1)
            },
            Entry {
                invoice: 0,
                amount: 2
            },
            Entry {
                invoice: 1,
                amount: (-2)
            },
            Entry {
                invoice: 1,
                amount: (-1)
            }
        ]),
        Report {
            settled: 0,
            outstanding: 14,
            credit: 5
        },
        "fixture 20"
    );
    assert_eq!(
        solve(&[Entry {
            invoice: 1,
            amount: 0
        }]),
        Report {
            settled: 1,
            outstanding: 0,
            credit: 0
        },
        "fixture 21"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: (-3)
            },
            Entry {
                invoice: 1,
                amount: 3
            },
            Entry {
                invoice: 2,
                amount: (-1)
            },
            Entry {
                invoice: 3,
                amount: 2
            }
        ]),
        Report {
            settled: 1,
            outstanding: 2,
            credit: 1
        },
        "fixture 22"
    );
    assert_eq!(
        solve(&[
            Entry {
                invoice: 1,
                amount: 5
            },
            Entry {
                invoice: 2,
                amount: (-4)
            },
            Entry {
                invoice: 1,
                amount: (-5)
            },
            Entry {
                invoice: 3,
                amount: 7
            }
        ]),
        Report {
            settled: 1,
            outstanding: 7,
            credit: 4
        },
        "fixture 23"
    );
}
