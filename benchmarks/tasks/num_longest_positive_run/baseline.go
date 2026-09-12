package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func LongestPositiveRun(values []int64) int64 {
    var current int64 = 0
    var best int64 = 0
    var index int64 = 0
    for (index < int64(len(values))) {
        if (nth(values, index) > 0) {
            current = (current + 1)
            if (current > best) {
                best = current
            }
        } else {
            current = 0
        }
        index = (index + 1)
    }
    return best
}
