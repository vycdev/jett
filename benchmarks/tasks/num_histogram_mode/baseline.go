package benchmark
type Mode struct { value int64; count int64 }
func nth(values []int64, index int64) int64 { return values[index] }
func occurrenceCount(values []int64, target int64) int64 {
    var count int64 = 0
    var index int64 = 0
    for (index < int64(len(values))) {
        if (nth(values, index) == target) {
            count = (count + 1)
        }
        index = (index + 1)
    }
    return count
}
func HistogramMode(values []int64) Mode {
    var bestValue int64 = 0
    var bestCount int64 = 0
    var index int64 = 0
    for (index < int64(len(values))) {
        var value int64 = nth(values, index)
        var count int64 = occurrenceCount(values, value)
        var replace bool = (count > bestCount)
        if (count == bestCount) {
            if (value < bestValue) {
                replace = true
            }
        }
        if replace {
            bestValue = value
            bestCount = count
        }
        index = (index + 1)
    }
    return Mode{value: bestValue, count: bestCount}
}
