pub fn fibonacci(n: i64) -> i64 {
    let mut previous: i64 = 0;
    let mut current: i64 = 1;
    let mut index: i64 = 0;
    while (index < n) {
        let mut following: i64 = (previous + current);
        previous = current;
        current = following;
        index = (index + 1);
    }
    return previous;
}
