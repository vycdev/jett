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

pub fn uint_value(text: String, bound: i64) -> i64 {
    if (length(text.clone()) == 0) {
        return (-1);
    }
    let mut value: i64 = 0;
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        let mut num: i64 = digit(at(text.clone(), pos));
        if (num < 0) {
            return (-1);
        }
        value = ((value * 10) + num);
        if (value > bound) {
            return (-1);
        }
        pos = (pos + 1);
    }
    return value;
}

pub fn repeat_letter(ch: String, count: i64) -> String {
    let mut out: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos < count) {
        out = (out.clone() + &ch.clone());
        pos = (pos + 1);
    }
    return out.clone();
}

pub fn solve(text: String) -> TextOutcome {
    let mut out: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        let mut start: i64 = pos;
        while ((pos < length(text.clone())) && (digit(at(text.clone(), pos)) >= 0)) {
            pos = (pos + 1);
        }
        let mut count: i64 = uint_value(part(text.clone(), start, pos), 100);
        if ((count <= 0) || (at(text.clone(), start) == "0".to_string())) {
            return TextOutcome::Rejected(TextError::Malformed);
        }
        let mut ch: String = at(text.clone(), pos);
        if ((pos == length(text.clone()))
            || (!contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(), ch.clone())))
        {
            return TextOutcome::Rejected(TextError::Malformed);
        }
        if ((length(out.clone()) + count) > 100) {
            return TextOutcome::Rejected(TextError::Range);
        }
        out = (out.clone() + &repeat_letter(ch.clone(), count));
        pos = (pos + 1);
    }
    return TextOutcome::Accepted(out.clone());
}
