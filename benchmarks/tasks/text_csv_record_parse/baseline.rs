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
fn new_words() -> Vec<String> {
    Vec::new()
}
fn push(mut values: Vec<String>, value: String) -> Vec<String> {
    values.push(value);
    values
}

#[derive(Debug, PartialEq, Eq)]
pub enum TextError {
    Empty,
    Malformed,
    Range,
}
#[derive(Debug, PartialEq, Eq)]
pub enum TextOutcome {
    Accepted(Vec<String>),
    Rejected(TextError),
}

pub fn quoted_end(text: String, start: i64) -> i64 {
    let mut pos: i64 = (start + 1);
    while (pos < length(text.clone())) {
        if (at(text.clone(), pos) == "\"".to_string()) {
            if (at(text.clone(), (pos + 1)) == "\"".to_string()) {
                pos = (pos + 2);
            } else {
                return (pos + 1);
            }
        } else {
            pos = (pos + 1);
        }
    }
    return (-1);
}

pub fn field_end(text: String, start: i64) -> i64 {
    if (at(text.clone(), start) == "\"".to_string()) {
        return quoted_end(text.clone(), start);
    }
    let mut pos: i64 = start;
    while ((pos < length(text.clone())) && (at(text.clone(), pos) != ",".to_string())) {
        if (at(text.clone(), pos) == "\"".to_string()) {
            return (-1);
        }
        pos = (pos + 1);
    }
    return pos;
}

pub fn decoded_field(text: String, start: i64, end: i64) -> String {
    if (at(text.clone(), start) != "\"".to_string()) {
        return part(text.clone(), start, end);
    }
    let mut out: String = "".to_string();
    let mut pos: i64 = (start + 1);
    while (pos < (end - 1)) {
        let mut ch: String = at(text.clone(), pos);
        out = (out.clone() + &ch.clone());
        if (ch.clone() == "\"".to_string()) {
            pos = (pos + 1);
        }
        pos = (pos + 1);
    }
    return out.clone();
}

pub fn solve(text: String) -> TextOutcome {
    let mut pos: i64 = 0;
    let mut fields: Vec<String> = new_words();
    while (pos <= length(text.clone())) {
        let mut end: i64 = field_end(text.clone(), pos);
        if (end < 0) {
            return TextOutcome::Rejected(TextError::Malformed);
        }
        fields = push(fields, decoded_field(text.clone(), pos, end));
        if (end == length(text.clone())) {
            return TextOutcome::Accepted(fields);
        }
        if (at(text.clone(), end) != ",".to_string()) {
            return TextOutcome::Rejected(TextError::Malformed);
        }
        pos = (end + 1);
    }
    return TextOutcome::Rejected(TextError::Malformed);
}
