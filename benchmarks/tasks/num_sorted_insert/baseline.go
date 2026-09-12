package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func SortedInsert(values []int64, value int64) []int64 {
    var output []int64 = []int64{}
    var inserted bool = false
    var index int64 = 0
    for (index < int64(len(values))) {
        var item int64 = nth(values, index)
        if !(inserted) {
            if (value <= item) {
                output = append(output, value)
                inserted = true
            }
        }
        output = append(output, item)
        index = (index + 1)
    }
    if !(inserted) {
        output = append(output, value)
    }
    return output
}
