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

pub fn solve(text: String, needle: String) -> i64 {
    let mut count: i64 = 0;
    let mut pos: i64 = 0;
    while ((pos + length(needle.clone())) <= length(text.clone())) {
        if (part(text.clone(), pos, (pos + length(needle.clone()))) == needle.clone()) {
            count = (count + 1);
        }
        pos = (pos + 1);
    }
    return count;
}
