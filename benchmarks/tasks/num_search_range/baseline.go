package benchmark
type SearchRange struct { first int64; last int64 }
func nth(values []int64, index int64) int64 { return values[index] }
func FindSearchRange(values []int64, target int64) SearchRange {
    var first int64 = -(1)
    var last int64 = -(1)
    var index int64 = 0
    for (index < int64(len(values))) {
        if (nth(values, index) == target) {
            if (first == -(1)) {
                first = index
            }
            last = index
        }
        index = (index + 1)
    }
    return SearchRange{first: first, last: last}
}
