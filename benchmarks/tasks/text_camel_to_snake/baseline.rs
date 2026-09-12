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

pub fn boundary(text: String, pos: i64) -> bool {
    if (pos == 0) {
        return false;
    }
    let mut ch: String = at(text.clone(), pos);
    if (!contains("ABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(), ch.clone())) {
        return false;
    }
    let mut prev: String = at(text.clone(), (pos - 1));
    if contains(
        "abcdefghijklmnopqrstuvwxyz0123456789".to_string(),
        prev.clone(),
    ) {
        return true;
    }
    return (contains(
        "abcdefghijklmnopqrstuvwxyz".to_string(),
        at(text.clone(), (pos + 1)),
    ) && ((pos + 1) < length(text.clone())));
}

pub fn solve(text: String) -> String {
    let mut out: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        if boundary(text.clone(), pos) {
            out = (out.clone() + &"_".to_string());
        }
        out = (out.clone() + &lower(at(text.clone(), pos)));
        pos = (pos + 1);
    }
    return out.clone();
}
