fn length(text: String) -> i64 {
    i64::try_from(text.len()).unwrap_or(0)
}
fn at(text: String, pos: i64) -> String {
    usize::try_from(pos)
        .ok()
        .and_then(|p| text.get(p..p + 1))
        .unwrap_or("")
        .to_string()
}
fn part(text: String, start: i64, end: i64) -> String {
    match (usize::try_from(start), usize::try_from(end)) {
        (Ok(a), Ok(b)) => text.get(a..b).unwrap_or("").to_string(),
        _ => String::new(),
    }
}
fn contains(text: String, needle: String) -> bool {
    text.contains(&needle)
}
fn lower(text: String) -> String {
    text.to_ascii_lowercase()
}
fn upper(text: String) -> String {
    text.to_ascii_uppercase()
}
fn render(value: i64) -> String {
    value.to_string()
}

#[derive(Debug, PartialEq, Eq)]
pub enum TextError {
    Empty,
    Malformed,
    Range,
}
#[derive(Debug, PartialEq, Eq)]
pub enum TextOutcome {
    Accepted(String),
    Rejected(TextError),
}

pub fn solve(text: String) -> TextOutcome {
    let mut best: String = "".to_string();
    let mut word: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos <= length(text.clone())) {
        let mut ch: String = at(text.clone(), pos);
        if ((ch.clone() == " ".to_string()) || (pos == length(text.clone()))) {
            if (length(word.clone()) > length(best.clone())) {
                best = word.clone();
            }
            word = "".to_string();
        } else {
            word = (word.clone() + &ch.clone());
        }
        pos = (pos + 1);
    }
    if (best.clone() == "".to_string()) {
        return TextOutcome::Rejected(TextError::Empty);
    }
    return TextOutcome::Accepted(best.clone());
}
