package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func PolynomialValue(coefficients []int64, x int64) int64 {
    var answer int64 = 0
    var index int64 = int64(len(coefficients))
    for (index > 0) {
        index = (index - 1)
        answer = ((answer * x) + nth(coefficients, index))
    }
    return answer
}
