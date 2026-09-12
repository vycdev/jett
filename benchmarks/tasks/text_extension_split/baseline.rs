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
pub struct TextPair {
    pub first: String,
    pub second: String,
}

pub fn solve(text: String) -> TextPair {
    let mut dot: i64 = (-1);
    let mut pos: i64 = 1;
    while (pos < length(text.clone())) {
        if (at(text.clone(), pos) == ".".to_string()) {
            dot = pos;
        }
        pos = (pos + 1);
    }
    if (dot < 0) {
        return TextPair {
            first: text.clone(),
            second: "".to_string(),
        };
    }
    return TextPair {
        first: part(text.clone(), 0, dot),
        second: part(text.clone(), (dot + 1), length(text.clone())),
    };
}
