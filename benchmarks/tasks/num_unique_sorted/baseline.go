package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func UniqueSorted(values []int64) []int64 {
    var output []int64 = []int64{}
    var index int64 = 0
    for (index < int64(len(values))) {
        var value int64 = nth(values, index)
        if (index == 0) {
            output = append(output, value)
        } else {
            if (value != nth(values, (index - 1))) {
                output = append(output, value)
            }
        }
        index = (index + 1)
    }
    return output
}
