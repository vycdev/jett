package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func MatrixTrace(values []int64, rows int64) int64 {
    var total int64 = 0
    var row int64 = 0
    for (row < rows) {
        total = (total + nth(values, ((row * rows) + row)))
        row = (row + 1)
    }
    return total
}
