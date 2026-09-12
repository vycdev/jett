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

pub fn alpha(ch: String) -> bool {
    return (contains(
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".to_string(),
        ch.clone(),
    ) && (length(ch.clone()) == 1));
}

pub fn alnum(ch: String) -> bool {
    return (alpha(ch.clone()) || (digit(ch.clone()) >= 0));
}

pub fn solve(text: String) -> bool {
    let mut clean: String = "".to_string();
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        let mut ch: String = at(text.clone(), pos);
        if alnum(ch.clone()) {
            clean = (clean.clone() + &lower(ch.clone()));
        }
        pos = (pos + 1);
    }
    pos = 0;
    while (pos < length(clean.clone())) {
        if (at(clean.clone(), pos) != at(clean.clone(), ((length(clean.clone()) - 1) - pos))) {
            return false;
        }
        pos = (pos + 1);
    }
    return true;
}
