package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func LisLength(values []int64) int64 {
    var lengths []int64 = []int64{}
    var best int64 = 0
    var index int64 = 0
    for (index < int64(len(values))) {
        var ending int64 = 1
        var prior int64 = 0
        for (prior < index) {
            if (nth(values, prior) < nth(values, index)) {
                var candidate int64 = (nth(lengths, prior) + 1)
                if (candidate > ending) {
                    ending = candidate
                }
            }
            prior = (prior + 1)
        }
        lengths = append(lengths, ending)
        if (ending > best) {
            best = ending
        }
        index = (index + 1)
    }
    return best
}
