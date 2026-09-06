include!("solution.rs");

fn lines(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn scores(values: &[(&str, i64)]) -> std::collections::BTreeMap<String, i64> {
    values
        .iter()
        .map(|(name, score)| ((*name).to_string(), *score))
        .collect()
}

#[test]
fn hidden_score_lines() {
    assert_eq!(parse_scores(lines(&[])), ScoreResult::Parsed(scores(&[])));
    assert_eq!(
        parse_scores(lines(&["ada=10", "bob=-2"])),
        ScoreResult::Parsed(scores(&[("ada", 10), ("bob", -2)]))
    );
    assert_eq!(
        parse_scores(lines(&[
            "ada=9223372036854775807",
            "bob=-9223372036854775808"
        ])),
        ScoreResult::Parsed(scores(&[
            ("ada", 9223372036854775807),
            ("bob", -9223372036854775808)
        ]))
    );
    assert_eq!(
        parse_scores(lines(&["ada=1", "ada=2"])),
        ScoreResult::Failure {
            line: 1,
            error: ScoreError::DuplicateName
        }
    );
    assert_eq!(
        parse_scores(lines(&["missing"])),
        ScoreResult::Failure {
            line: 0,
            error: ScoreError::Malformed
        }
    );
    assert_eq!(
        parse_scores(lines(&["=3"])),
        ScoreResult::Failure {
            line: 0,
            error: ScoreError::Malformed
        }
    );
    assert_eq!(
        parse_scores(lines(&["a=1=2"])),
        ScoreResult::Failure {
            line: 0,
            error: ScoreError::Malformed
        }
    );
    assert_eq!(
        parse_scores(lines(&["ada=01"])),
        ScoreResult::Failure {
            line: 0,
            error: ScoreError::InvalidScore
        }
    );
    assert_eq!(
        parse_scores(lines(&["ada=-0"])),
        ScoreResult::Failure {
            line: 0,
            error: ScoreError::InvalidScore
        }
    );
    assert_eq!(
        parse_scores(lines(&["ada=9223372036854775808"])),
        ScoreResult::Failure {
            line: 0,
            error: ScoreError::InvalidScore
        }
    );
    assert_eq!(
        parse_scores(lines(&["ada=1", "ada=bad"])),
        ScoreResult::Failure {
            line: 1,
            error: ScoreError::InvalidScore
        }
    );
}
