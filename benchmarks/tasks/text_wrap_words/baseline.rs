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

pub fn compact(text: String) -> String {
    let mut out: String = "".to_string();
    let mut pending: bool = false;
    let mut pos: i64 = 0;
    while (pos < length(text.clone())) {
        let mut ch: String = at(text.clone(), pos);
        if (ch.clone() == " ".to_string()) {
            pending = (length(out.clone()) > 0);
        } else {
            if pending {
                out = (out.clone() + &" ".to_string());
            }
            out = (out.clone() + &ch.clone());
            pending = false;
        }
        pos = (pos + 1);
    }
    return out.clone();
}

pub fn append_word(out: String, word: String, used: i64, width: i64) -> String {
    if (out.clone() == "".to_string()) {
        return word.clone();
    }
    if (((used + 1) + length(word.clone())) <= width) {
        return ((out.clone() + &" ".to_string()) + &word.clone());
    }
    return ((out.clone() + &"\n".to_string()) + &word.clone());
}

pub fn solve(text: String, width: i64) -> String {
    let mut clean: String = compact(text.clone());
    let mut out: String = "".to_string();
    let mut used: i64 = 0;
    let mut start: i64 = 0;
    let mut pos: i64 = 0;
    while (pos <= length(clean.clone())) {
        if ((at(clean.clone(), pos) == " ".to_string()) || (pos == length(clean.clone()))) {
            let mut word: String = part(clean.clone(), start, pos);
            out = append_word(out.clone(), word.clone(), used, width);
            if ((used == 0) || (((used + 1) + length(word.clone())) > width)) {
                used = length(word.clone());
            } else {
                used = ((used + 1) + length(word.clone()));
            }
            start = (pos + 1);
        }
        pos = (pos + 1);
    }
    return out.clone();
}
