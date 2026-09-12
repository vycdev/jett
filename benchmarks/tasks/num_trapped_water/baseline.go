package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func TrappedWater(heights []int64) int64 {
    var leftPeaks []int64 = []int64{}
    var peak int64 = 0
    var index int64 = 0
    for (index < int64(len(heights))) {
        var height int64 = nth(heights, index)
        if (height > peak) {
            peak = height
        }
        leftPeaks = append(leftPeaks, peak)
        index = (index + 1)
    }
    peak = 0
    var volume int64 = 0
    for (index > 0) {
        index = (index - 1)
        var height int64 = nth(heights, index)
        if (height > peak) {
            peak = height
        }
        var ceiling int64 = nth(leftPeaks, index)
        if (peak < ceiling) {
            ceiling = peak
        }
        volume = ((volume + ceiling) - height)
    }
    return volume
}
