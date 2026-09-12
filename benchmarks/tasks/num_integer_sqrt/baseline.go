package benchmark
func safeDiv(left int64, right int64) int64 { return left / right }
func IntegerSqrt(n int64) int64 {
    var lower int64 = 0
    var upper int64 = 1000001
    for ((lower + 1) < upper) {
        var middle int64 = safeDiv((lower + upper), 2)
        if ((middle * middle) <= n) {
            lower = middle
        } else {
            upper = middle
        }
    }
    return lower
}
