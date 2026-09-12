package benchmark
func safeDiv(left int64, right int64) int64 { return left / right }
func Binomial(n int64, k int64) int64 {
    if (k > n) {
        return 0
    }
    var answer int64 = 1
    var index int64 = 1
    for (index <= k) {
        answer = safeDiv((answer * ((n - index) + 1)), index)
        index = (index + 1)
    }
    return answer
}
