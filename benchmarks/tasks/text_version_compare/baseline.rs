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

pub fn component_end(text: String, start: i64) -> i64 {
    let mut pos: i64 = start;
    while (pos < length(text.clone())) {
        if (at(text.clone(), pos) == ".".to_string()) {
            return pos;
        }
        pos = (pos + 1);
    }
    return pos;
}

pub fn component_value(text: String, start: i64, end: i64) -> i64 {
    if (start >= length(text.clone())) {
        return 0;
    }
    return uint_value(part(text.clone(), start, end), 999);
}

pub fn solve(left: String, right: String) -> i64 {
    let mut lp: i64 = 0;
    let mut rp: i64 = 0;
    while ((lp < length(left.clone())) || (rp < length(right.clone()))) {
        let mut le: i64 = component_end(left.clone(), lp);
        let mut re: i64 = component_end(right.clone(), rp);
        let mut lv: i64 = component_value(left.clone(), lp, le);
        let mut rv: i64 = component_value(right.clone(), rp, re);
        if (lv < rv) {
            return (-1);
        }
        if (lv > rv) {
            return 1;
        }
        lp = (le + 1);
        rp = (re + 1);
    }
    return 0;
}
