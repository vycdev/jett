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

pub fn hex_digit(ch: String) -> i64 {
    let mut pos: i64 = 0;
    while (pos < 16) {
        if (at("0123456789abcdef".to_string(), pos) == lower(ch.clone())) {
            return pos;
        }
        pos = (pos + 1);
    }
    return (-1);
}

pub fn ascii_print(code: i64) -> String {
    return at(" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~".to_string(), (code - 32));
}

pub fn solve(text: String) -> TextOutcome {
    if ((length(text.clone()) % 2) != 0) {
        return TextOutcome::Rejected(TextError::Malformed);
    }
    let mut out: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        let mut high: i64 = hex_digit(at(text.clone(), pos));
        let mut low: i64 = hex_digit(at(text.clone(), (pos + 1)));
        if ((high < 0) || (low < 0)) {
            return TextOutcome::Rejected(TextError::Malformed);
        }
        let mut code: i64 = ((high * 16) + low);
        if ((code < 32) || (code > 126)) {
            return TextOutcome::Rejected(TextError::Range);
        }
        out = (out.clone() + &ascii_print(code));
        pos = (pos + 2);
    }
    return TextOutcome::Accepted(out.clone());
}
