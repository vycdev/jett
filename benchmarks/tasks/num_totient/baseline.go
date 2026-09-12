package benchmark
func safeDiv(left int64, right int64) int64 { return left / right }
func safeMod(left int64, right int64) int64 { return left % right }
func Totient(n int64) int64 {
    var remaining int64 = n
    var count int64 = n
    var factor int64 = 2
    for ((factor * factor) <= remaining) {
        if (safeMod(remaining, factor) == 0) {
            count = (count - safeDiv(count, factor))
            for (safeMod(remaining, factor) == 0) {
                remaining = safeDiv(remaining, factor)
            }
        }
        factor = (factor + 1)
    }
    if (remaining > 1) {
        count = (count - safeDiv(count, remaining))
    }
    return count
}
