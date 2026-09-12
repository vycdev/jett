package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func PairSumCount(values []int64, target int64) int64 {
    var count int64 = 0
    var first int64 = 0
    for (first < int64(len(values))) {
        var second int64 = (first + 1)
        for (second < int64(len(values))) {
            if ((nth(values, first) + nth(values, second)) == target) {
                count = (count + 1)
            }
            second = (second + 1)
        }
        first = (first + 1)
    }
    return count
}
