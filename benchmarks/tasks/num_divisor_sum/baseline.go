package benchmark
func safeDiv(left int64, right int64) int64 { return left / right }
func safeMod(left int64, right int64) int64 { return left % right }
func DivisorSum(n int64) int64 {
    var total int64 = 0
    var divisor int64 = 1
    for ((divisor * divisor) <= n) {
        if (safeMod(n, divisor) == 0) {
            total = (total + divisor)
            var partner int64 = safeDiv(n, divisor)
            if (partner != divisor) {
                total = (total + partner)
            }
        }
        divisor = (divisor + 1)
    }
    return total
}
