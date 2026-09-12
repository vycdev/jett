package benchmark
func Fibonacci(n int64) int64 {
    var previous int64 = 0
    var current int64 = 1
    var index int64 = 0
    for (index < n) {
        var following int64 = (previous + current)
        previous = current
        current = following
        index = (index + 1)
    }
    return previous
}
