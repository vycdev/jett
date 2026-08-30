use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum ScoreError {
    Malformed,
    InvalidScore,
    DuplicateName,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ScoreResult {
    Parsed(BTreeMap<String, i64>),
    Failure { line: i64, error: ScoreError },
}

pub fn parse_scores(lines: Vec<String>) -> ScoreResult {
    let mut scores = BTreeMap::new();
    for (line, text) in lines.into_iter().enumerate() {
        let mut parts = text.split('=');
        let name = parts.next();
        let raw_score = parts.next();
        if name.is_none() || name == Some("") || raw_score.is_none() || parts.next().is_some() {
            return ScoreResult::Failure {
                line: line as i64,
                error: ScoreError::Malformed,
            };
        }
        let name = name.unwrap_or("");
        let raw_score = raw_score.unwrap_or("");
        let score = match raw_score.parse::<i64>() {
            Ok(value) if value.to_string() == raw_score => value,
            Ok(_) => {
                return ScoreResult::Failure {
                    line: line as i64,
                    error: ScoreError::InvalidScore,
                };
            }
            Err(_) => {
                return ScoreResult::Failure {
                    line: line as i64,
                    error: ScoreError::InvalidScore,
                };
            }
        };
        if scores.contains_key(name) {
            return ScoreResult::Failure {
                line: line as i64,
                error: ScoreError::DuplicateName,
            };
        }
        scores.insert(name.to_string(), score);
    }
    ScoreResult::Parsed(scores)
}
