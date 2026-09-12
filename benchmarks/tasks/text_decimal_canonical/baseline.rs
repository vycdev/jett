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

pub fn digit(ch: String) -> i64 {
    let mut pos: i64 = 0;
    while (pos < 10) {
        if (at("0123456789".to_string(), pos) == ch.clone()) {
            return pos;
        }
        pos = (pos + 1);
    }
    return (-1);
}

pub fn signed_digits(negative: bool, digits: String) -> TextOutcome {
    if (digits.clone() == "".to_string()) {
        return TextOutcome::Accepted("0".to_string());
    }
    if negative {
        return TextOutcome::Accepted(("-".to_string() + &digits.clone()));
    }
    return TextOutcome::Accepted(digits.clone());
}

pub fn solve(text: String) -> TextOutcome {
    if (length(text.clone()) == 0) {
        return TextOutcome::Rejected(TextError::Empty);
    }
    let mut pos: i64 = 0;
    let mut negative: bool = (at(text.clone(), 0) == "-".to_string());
    if (negative || (at(text.clone(), 0) == "+".to_string())) {
        pos = 1;
    }
    if (pos == length(text.clone())) {
        return TextOutcome::Rejected(TextError::Malformed);
    }
    let mut out: String = "".to_string();
    while (pos < length(text.clone())) {
        let mut ch: String = at(text.clone(), pos);
        if (digit(ch.clone()) < 0) {
            return TextOutcome::Rejected(TextError::Malformed);
        }
        if ((out.clone() != "".to_string()) || (ch.clone() != "0".to_string())) {
            out = (out.clone() + &ch.clone());
        }
        pos = (pos + 1);
    }
    return signed_digits(negative, out.clone());
}
