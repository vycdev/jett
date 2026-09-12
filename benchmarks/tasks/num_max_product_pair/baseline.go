package benchmark
type MaybeInt struct { Found bool; Value int64 }
func nth(values []int64, index int64) int64 { return values[index] }
func MaxProductPair(values []int64) MaybeInt {
    if (int64(len(values)) < 2) {
        return MaybeInt{}
    }
    var best int64 = (nth(values, 0) * nth(values, 1))
    var first int64 = 0
    for (first < int64(len(values))) {
        var second int64 = (first + 1)
        for (second < int64(len(values))) {
            var candidate int64 = (nth(values, first) * nth(values, second))
            if (candidate > best) {
                best = candidate
            }
            second = (second + 1)
        }
        first = (first + 1)
    }
    return MaybeInt{Found: true, Value: best}
}
