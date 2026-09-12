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

pub fn trim_space(text: String) -> String {
    let mut start: i64 = 0;
    let mut end: i64 = length(text.clone());
    while ((start < end) && (at(text.clone(), start) == " ".to_string())) {
        start = (start + 1);
    }
    while ((end > start) && (at(text.clone(), (end - 1)) == " ".to_string())) {
        end = (end - 1);
    }
    return part(text.clone(), start, end);
}

pub fn solve(text: String) -> TextPair {
    let mut pos: i64 = 0;
    while (at(text.clone(), pos) != "=".to_string()) {
        pos = (pos + 1);
    }
    return TextPair {
        first: trim_space(part(text.clone(), 0, pos)),
        second: trim_space(part(text.clone(), (pos + 1), length(text.clone()))),
    };
}
