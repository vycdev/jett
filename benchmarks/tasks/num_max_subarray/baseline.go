package benchmark
type MaybeInt struct { Found bool; Value int64 }
func nth(values []int64, index int64) int64 { return values[index] }
func MaxSubarray(values []int64) MaybeInt {
    if (int64(len(values)) == 0) {
        return MaybeInt{}
    }
    var ending int64 = nth(values, 0)
    var best int64 = ending
    var index int64 = 1
    for (index < int64(len(values))) {
        var value int64 = nth(values, index)
        if (ending > 0) {
            ending = (ending + value)
        } else {
            ending = value
        }
        if (ending > best) {
            best = ending
        }
        index = (index + 1)
    }
    return MaybeInt{Found: true, Value: best}
}
