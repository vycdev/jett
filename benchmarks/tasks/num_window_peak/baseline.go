package benchmark
type MaybeInt struct { Found bool; Value int64 }
func nth(values []int64, index int64) int64 { return values[index] }
func WindowPeak(values []int64, width int64) MaybeInt {
    if (width == 0) {
        return MaybeInt{}
    }
    if (width > int64(len(values))) {
        return MaybeInt{}
    }
    var total int64 = 0
    var index int64 = 0
    for (index < width) {
        total = (total + nth(values, index))
        index = (index + 1)
    }
    var best int64 = total
    for (index < int64(len(values))) {
        total = ((total + nth(values, index)) - nth(values, (index - width)))
        if (total > best) {
            best = total
        }
        index = (index + 1)
    }
    return MaybeInt{Found: true, Value: best}
}
