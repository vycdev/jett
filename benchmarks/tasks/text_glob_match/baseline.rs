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

pub fn solve(text: String, pattern: String) -> bool {
    let mut ti: i64 = 0;
    let mut pi: i64 = 0;
    let mut star: i64 = (-1);
    let mut retry: i64 = 0;
    while (ti < length(text.clone())) {
        let mut ch: String = at(pattern.clone(), pi);
        if ((pi < length(pattern.clone()))
            && ((ch.clone() == "?".to_string()) || (ch.clone() == at(text.clone(), ti))))
        {
            ti = (ti + 1);
            pi = (pi + 1);
        } else {
            if (ch.clone() == "*".to_string()) {
                star = pi;
                retry = ti;
                pi = (pi + 1);
            } else {
                if (star < 0) {
                    return false;
                }
                retry = (retry + 1);
                ti = retry;
                pi = (star + 1);
            }
        }
    }
    while (at(pattern.clone(), pi) == "*".to_string()) {
        pi = (pi + 1);
    }
    return (pi == length(pattern.clone()));
}
