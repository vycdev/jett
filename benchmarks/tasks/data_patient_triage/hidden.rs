include!("solution.rs");

#[test]
fn fixtures() {
    assert_eq!(
        solve(&[], 0),
        Report {
            selected: 0,
            patient_checksum: 0,
            severity_sum: 0
        },
        "fixture 0"
    );
    assert_eq!(
        solve(&[], 5),
        Report {
            selected: 0,
            patient_checksum: 0,
            severity_sum: 0
        },
        "fixture 1"
    );
    assert_eq!(
        solve(&[], 1),
        Report {
            selected: 0,
            patient_checksum: 0,
            severity_sum: 0
        },
        "fixture 2"
    );
    assert_eq!(
        solve(
            &[Entry {
                patient: 3,
                severity: 8,
                arrival: 3
            }],
            2
        ),
        Report {
            selected: 1,
            patient_checksum: 4,
            severity_sum: 8
        },
        "fixture 3"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 1,
                    severity: 6,
                    arrival: 3
                },
                Entry {
                    patient: 3,
                    severity: 7,
                    arrival: 5
                }
            ],
            3
        ),
        Report {
            selected: 2,
            patient_checksum: 8,
            severity_sum: 13
        },
        "fixture 4"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 2,
                    severity: 11,
                    arrival: 1
                },
                Entry {
                    patient: 0,
                    severity: 0,
                    arrival: 6
                },
                Entry {
                    patient: 0,
                    severity: (-2),
                    arrival: 5
                }
            ],
            4
        ),
        Report {
            selected: 3,
            patient_checksum: 8,
            severity_sum: 9
        },
        "fixture 5"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 3,
                    severity: 10,
                    arrival: 2
                },
                Entry {
                    patient: 2,
                    severity: 2,
                    arrival: 3
                },
                Entry {
                    patient: 4,
                    severity: 0,
                    arrival: 4
                },
                Entry {
                    patient: 0,
                    severity: 11,
                    arrival: 2
                }
            ],
            5
        ),
        Report {
            selected: 4,
            patient_checksum: 38,
            severity_sum: 23
        },
        "fixture 6"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 3,
                    severity: 10,
                    arrival: 3
                },
                Entry {
                    patient: 0,
                    severity: 5,
                    arrival: 5
                },
                Entry {
                    patient: 4,
                    severity: 2,
                    arrival: 6
                },
                Entry {
                    patient: 1,
                    severity: 3,
                    arrival: 1
                },
                Entry {
                    patient: 1,
                    severity: 1,
                    arrival: 1
                }
            ],
            6
        ),
        Report {
            selected: 5,
            patient_checksum: 42,
            severity_sum: 21
        },
        "fixture 7"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 3,
                    severity: 5,
                    arrival: 6
                },
                Entry {
                    patient: 1,
                    severity: (-2),
                    arrival: 5
                },
                Entry {
                    patient: 3,
                    severity: 11,
                    arrival: 6
                },
                Entry {
                    patient: 4,
                    severity: 2,
                    arrival: 4
                },
                Entry {
                    patient: 0,
                    severity: 10,
                    arrival: 6
                },
                Entry {
                    patient: 3,
                    severity: 6,
                    arrival: 5
                }
            ],
            7
        ),
        Report {
            selected: 6,
            patient_checksum: 71,
            severity_sum: 32
        },
        "fixture 8"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 1,
                    severity: 5,
                    arrival: 1
                },
                Entry {
                    patient: 1,
                    severity: 1,
                    arrival: 1
                },
                Entry {
                    patient: 3,
                    severity: (-1),
                    arrival: 1
                },
                Entry {
                    patient: 0,
                    severity: 8,
                    arrival: 4
                },
                Entry {
                    patient: 3,
                    severity: 7,
                    arrival: 4
                },
                Entry {
                    patient: 0,
                    severity: 5,
                    arrival: 4
                },
                Entry {
                    patient: 1,
                    severity: 11,
                    arrival: 3
                }
            ],
            8
        ),
        Report {
            selected: 7,
            patient_checksum: 69,
            severity_sum: 36
        },
        "fixture 9"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 0,
                    severity: (-3),
                    arrival: 3
                },
                Entry {
                    patient: 1,
                    severity: 7,
                    arrival: 4
                },
                Entry {
                    patient: 1,
                    severity: 7,
                    arrival: 1
                },
                Entry {
                    patient: 3,
                    severity: (-2),
                    arrival: 3
                },
                Entry {
                    patient: 4,
                    severity: (-1),
                    arrival: 4
                },
                Entry {
                    patient: 0,
                    severity: 4,
                    arrival: 5
                },
                Entry {
                    patient: 2,
                    severity: 8,
                    arrival: 4
                },
                Entry {
                    patient: 1,
                    severity: (-2),
                    arrival: 6
                }
            ],
            1
        ),
        Report {
            selected: 1,
            patient_checksum: 3,
            severity_sum: 8
        },
        "fixture 10"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 2,
                    severity: 10,
                    arrival: 2
                },
                Entry {
                    patient: 2,
                    severity: 11,
                    arrival: 2
                },
                Entry {
                    patient: 4,
                    severity: 2,
                    arrival: 2
                },
                Entry {
                    patient: 3,
                    severity: 1,
                    arrival: 2
                },
                Entry {
                    patient: 2,
                    severity: 1,
                    arrival: 5
                },
                Entry {
                    patient: 2,
                    severity: 4,
                    arrival: 2
                },
                Entry {
                    patient: 0,
                    severity: 2,
                    arrival: 1
                },
                Entry {
                    patient: 3,
                    severity: (-3),
                    arrival: 4
                },
                Entry {
                    patient: 4,
                    severity: 2,
                    arrival: 2
                }
            ],
            2
        ),
        Report {
            selected: 2,
            patient_checksum: 9,
            severity_sum: 21
        },
        "fixture 11"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 0,
                    severity: 7,
                    arrival: 1
                },
                Entry {
                    patient: 3,
                    severity: (-3),
                    arrival: 1
                },
                Entry {
                    patient: 1,
                    severity: 5,
                    arrival: 6
                },
                Entry {
                    patient: 0,
                    severity: 10,
                    arrival: 1
                },
                Entry {
                    patient: 1,
                    severity: 8,
                    arrival: 3
                },
                Entry {
                    patient: 3,
                    severity: 6,
                    arrival: 3
                },
                Entry {
                    patient: 0,
                    severity: 2,
                    arrival: 3
                },
                Entry {
                    patient: 4,
                    severity: 5,
                    arrival: 3
                },
                Entry {
                    patient: 2,
                    severity: (-1),
                    arrival: 2
                },
                Entry {
                    patient: 0,
                    severity: 1,
                    arrival: 2
                }
            ],
            3
        ),
        Report {
            selected: 3,
            patient_checksum: 8,
            severity_sum: 25
        },
        "fixture 12"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 2,
                    severity: 5,
                    arrival: 3
                },
                Entry {
                    patient: 4,
                    severity: 3,
                    arrival: 6
                },
                Entry {
                    patient: 3,
                    severity: 11,
                    arrival: 1
                },
                Entry {
                    patient: 1,
                    severity: 8,
                    arrival: 6
                },
                Entry {
                    patient: 3,
                    severity: 4,
                    arrival: 2
                },
                Entry {
                    patient: 0,
                    severity: 1,
                    arrival: 1
                },
                Entry {
                    patient: 4,
                    severity: 3,
                    arrival: 2
                },
                Entry {
                    patient: 1,
                    severity: 5,
                    arrival: 6
                },
                Entry {
                    patient: 4,
                    severity: 6,
                    arrival: 5
                },
                Entry {
                    patient: 1,
                    severity: 9,
                    arrival: 5
                },
                Entry {
                    patient: 4,
                    severity: 7,
                    arrival: 2
                }
            ],
            4
        ),
        Report {
            selected: 4,
            patient_checksum: 34,
            severity_sum: 35
        },
        "fixture 13"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 1,
                    severity: 3,
                    arrival: 2
                },
                Entry {
                    patient: 1,
                    severity: 7,
                    arrival: 2
                },
                Entry {
                    patient: 3,
                    severity: 4,
                    arrival: 6
                },
                Entry {
                    patient: 2,
                    severity: 6,
                    arrival: 3
                },
                Entry {
                    patient: 2,
                    severity: 11,
                    arrival: 3
                },
                Entry {
                    patient: 1,
                    severity: 0,
                    arrival: 5
                },
                Entry {
                    patient: 0,
                    severity: 2,
                    arrival: 2
                },
                Entry {
                    patient: 1,
                    severity: 10,
                    arrival: 3
                },
                Entry {
                    patient: 1,
                    severity: (-1),
                    arrival: 5
                },
                Entry {
                    patient: 0,
                    severity: (-1),
                    arrival: 1
                },
                Entry {
                    patient: 3,
                    severity: 11,
                    arrival: 6
                },
                Entry {
                    patient: 3,
                    severity: (-3),
                    arrival: 4
                }
            ],
            5
        ),
        Report {
            selected: 5,
            patient_checksum: 40,
            severity_sum: 45
        },
        "fixture 14"
    );
    assert_eq!(
        solve(&[], 6),
        Report {
            selected: 0,
            patient_checksum: 0,
            severity_sum: 0
        },
        "fixture 15"
    );
    assert_eq!(
        solve(
            &[Entry {
                patient: 3,
                severity: (-1),
                arrival: 5
            }],
            7
        ),
        Report {
            selected: 1,
            patient_checksum: 4,
            severity_sum: (-1)
        },
        "fixture 16"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 0,
                    severity: 4,
                    arrival: 1
                },
                Entry {
                    patient: 4,
                    severity: 8,
                    arrival: 4
                }
            ],
            8
        ),
        Report {
            selected: 2,
            patient_checksum: 7,
            severity_sum: 12
        },
        "fixture 17"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 3,
                    severity: 7,
                    arrival: 3
                },
                Entry {
                    patient: 0,
                    severity: (-1),
                    arrival: 4
                },
                Entry {
                    patient: 0,
                    severity: 3,
                    arrival: 4
                }
            ],
            1
        ),
        Report {
            selected: 1,
            patient_checksum: 4,
            severity_sum: 7
        },
        "fixture 18"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 1,
                    severity: 11,
                    arrival: 3
                },
                Entry {
                    patient: 0,
                    severity: 5,
                    arrival: 2
                },
                Entry {
                    patient: 3,
                    severity: 4,
                    arrival: 3
                },
                Entry {
                    patient: 0,
                    severity: 4,
                    arrival: 2
                }
            ],
            2
        ),
        Report {
            selected: 2,
            patient_checksum: 4,
            severity_sum: 16
        },
        "fixture 19"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 1,
                    severity: 1,
                    arrival: 5
                },
                Entry {
                    patient: 2,
                    severity: 5,
                    arrival: 4
                },
                Entry {
                    patient: 4,
                    severity: (-3),
                    arrival: 3
                },
                Entry {
                    patient: 2,
                    severity: 5,
                    arrival: 5
                },
                Entry {
                    patient: 1,
                    severity: (-3),
                    arrival: 6
                }
            ],
            3
        ),
        Report {
            selected: 3,
            patient_checksum: 15,
            severity_sum: 11
        },
        "fixture 20"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 0,
                    severity: 4,
                    arrival: 4
                },
                Entry {
                    patient: 2,
                    severity: 1,
                    arrival: 6
                },
                Entry {
                    patient: 3,
                    severity: 10,
                    arrival: 1
                },
                Entry {
                    patient: 4,
                    severity: (-3),
                    arrival: 1
                },
                Entry {
                    patient: 2,
                    severity: 4,
                    arrival: 1
                },
                Entry {
                    patient: 1,
                    severity: 4,
                    arrival: 6
                }
            ],
            4
        ),
        Report {
            selected: 4,
            patient_checksum: 21,
            severity_sum: 22
        },
        "fixture 21"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 2,
                    severity: 1,
                    arrival: 6
                },
                Entry {
                    patient: 0,
                    severity: 6,
                    arrival: 5
                },
                Entry {
                    patient: 1,
                    severity: 6,
                    arrival: 5
                },
                Entry {
                    patient: 0,
                    severity: 0,
                    arrival: 5
                },
                Entry {
                    patient: 1,
                    severity: 0,
                    arrival: 2
                },
                Entry {
                    patient: 2,
                    severity: 11,
                    arrival: 1
                },
                Entry {
                    patient: 4,
                    severity: (-1),
                    arrival: 4
                }
            ],
            5
        ),
        Report {
            selected: 5,
            patient_checksum: 33,
            severity_sum: 24
        },
        "fixture 22"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 4,
                    severity: 6,
                    arrival: 1
                },
                Entry {
                    patient: 4,
                    severity: (-1),
                    arrival: 1
                },
                Entry {
                    patient: 0,
                    severity: (-3),
                    arrival: 4
                },
                Entry {
                    patient: 2,
                    severity: 9,
                    arrival: 5
                },
                Entry {
                    patient: 0,
                    severity: (-1),
                    arrival: 2
                },
                Entry {
                    patient: 0,
                    severity: 2,
                    arrival: 2
                },
                Entry {
                    patient: 1,
                    severity: (-2),
                    arrival: 3
                },
                Entry {
                    patient: 1,
                    severity: (-1),
                    arrival: 6
                }
            ],
            6
        ),
        Report {
            selected: 6,
            patient_checksum: 53,
            severity_sum: 14
        },
        "fixture 23"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 8,
                    severity: 5,
                    arrival: 2
                },
                Entry {
                    patient: 1,
                    severity: 5,
                    arrival: 2
                },
                Entry {
                    patient: 3,
                    severity: 5,
                    arrival: 1
                }
            ],
            2
        ),
        Report {
            selected: 2,
            patient_checksum: 22,
            severity_sum: 10
        },
        "fixture 24"
    );
    assert_eq!(
        solve(
            &[
                Entry {
                    patient: 9,
                    severity: 2,
                    arrival: 1
                },
                Entry {
                    patient: 3,
                    severity: 5,
                    arrival: 4
                },
                Entry {
                    patient: 4,
                    severity: 5,
                    arrival: 4
                },
                Entry {
                    patient: 8,
                    severity: 5,
                    arrival: 2
                }
            ],
            2
        ),
        Report {
            selected: 2,
            patient_checksum: 17,
            severity_sum: 10
        },
        "fixture 25"
    );
}
