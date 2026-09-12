package benchmark
func safeMod(left int64, right int64) int64 { return left % right }
func PrimeStatus(n int64) bool {
    if (n < 2) {
        return false
    }
    var divisor int64 = 2
    for ((divisor * divisor) <= n) {
        if (safeMod(n, divisor) == 0) {
            return false
        }
        divisor = (divisor + 1)
    }
    return true
}
