package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func hasStep(blocked []int64, step int64) bool {
    var index int64 = 0
    for (index < int64(len(blocked))) {
        if (nth(blocked, index) == step) {
            return true
        }
        index = (index + 1)
    }
    return false
}
func StaircaseBlocked(height int64, blocked []int64) int64 {
    var previous int64 = 0
    var current int64 = 1
    var step int64 = 1
    for (step <= height) {
        var following int64 = (current + previous)
        if hasStep(blocked, step) {
            following = 0
        }
        previous = current
        current = following
        step = (step + 1)
    }
    return current
}
