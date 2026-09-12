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

pub fn escape_char(ch: String) -> String {
    if (ch.clone() == "&".to_string()) {
        return "&amp;".to_string();
    }
    if (ch.clone() == "<".to_string()) {
        return "&lt;".to_string();
    }
    if (ch.clone() == ">".to_string()) {
        return "&gt;".to_string();
    }
    if (ch.clone() == "\"".to_string()) {
        return "&quot;".to_string();
    }
    if (ch.clone() == "'".to_string()) {
        return "&#39;".to_string();
    }
    return ch.clone();
}

pub fn solve(text: String) -> String {
    let mut out: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        out = (out.clone() + &escape_char(at(text.clone(), pos)));
        pos = (pos + 1);
    }
    return out.clone();
}
