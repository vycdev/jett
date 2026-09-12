fn nth(values: &[i64], index: i64) -> i64 { values[usize::try_from(index).unwrap_or(0)] }
pub fn trapped_water(heights: &[i64]) -> i64 {
    let mut left_peaks: Vec<i64> = vec![];
    let mut peak: i64 = 0;
    let mut index: i64 = 0;
    while (index < i64::try_from(heights.len()).unwrap_or(0)) {
        let mut height: i64 = nth(&heights, index);
        if (height > peak) {
            peak = height;
        }
        left_peaks.push(peak);
        index = (index + 1);
    }
    peak = 0;
    let mut volume: i64 = 0;
    while (index > 0) {
        index = (index - 1);
        let mut height: i64 = nth(&heights, index);
        if (height > peak) {
            peak = height;
        }
        let mut ceiling: i64 = nth(&left_peaks, index);
        if (peak < ceiling) {
            ceiling = peak;
        }
        volume = ((volume + ceiling) - height);
    }
    return volume;
}
