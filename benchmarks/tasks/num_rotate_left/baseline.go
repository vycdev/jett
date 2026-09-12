package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func safeMod(left int64, right int64) int64 { return left % right }
func RotateLeft(values []int64, distance int64) []int64 {
    var rotated []int64 = []int64{}
    var count int64 = int64(len(values))
    if (count == 0) {
        return rotated
    }
    var shift int64 = safeMod(distance, count)
    var index int64 = 0
    for (index < count) {
        var position int64 = safeMod((index + shift), count)
        rotated = append(rotated, nth(values, position))
        index = (index + 1)
    }
    return rotated
}
