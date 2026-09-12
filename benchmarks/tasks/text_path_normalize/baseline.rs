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

pub fn parent_path(text: String) -> String {
    let mut pos: i64 = (length(text.clone()) - 1);
    while (pos >= 0) {
        if (at(text.clone(), pos) == "/".to_string()) {
            return part(text.clone(), 0, pos);
        }
        pos = (pos - 1);
    }
    return "".to_string();
}

pub fn add_segment(path: String, segment: String) -> String {
    if ((segment.clone() == "".to_string()) || (segment.clone() == ".".to_string())) {
        return path.clone();
    }
    if (segment.clone() == "..".to_string()) {
        return parent_path(path.clone());
    }
    return ((path.clone() + &"/".to_string()) + &segment.clone());
}

pub fn solve(text: String) -> String {
    let mut path: String = "".to_string();
    let mut start: i64 = 1;
    let mut pos: i64 = 1;
    while (pos <= length(text.clone())) {
        if ((pos == length(text.clone())) || (at(text.clone(), pos) == "/".to_string())) {
            path = add_segment(path.clone(), part(text.clone(), start, pos));
            start = (pos + 1);
        }
        pos = (pos + 1);
    }
    if (path.clone() == "".to_string()) {
        return "/".to_string();
    }
    return path.clone();
}
