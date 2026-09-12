package benchmark
type MaybeInt struct { Found bool; Value int64 }
func nth(values []int64, index int64) int64 { return values[index] }
func EquilibriumIndex(values []int64) MaybeInt {
    var remaining int64 = 0
    var index int64 = 0
    for (index < int64(len(values))) {
        remaining = (remaining + nth(values, index))
        index = (index + 1)
    }
    var before int64 = 0
    index = 0
    for (index < int64(len(values))) {
        var value int64 = nth(values, index)
        remaining = (remaining - value)
        if (before == remaining) {
            return MaybeInt{Found: true, Value: index}
        }
        before = (before + value)
        index = (index + 1)
    }
    return MaybeInt{}
}
