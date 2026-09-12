package benchmark
type MaybeInt struct { Found bool; Value int64 }
func safeDiv(left int64, right int64) int64 { return left / right }
func safeMod(left int64, right int64) int64 { return left % right }
func CollatzSteps(n int64, limit int64) MaybeInt {
    var current int64 = n
    var steps int64 = 0
    for (current != 1) {
        if (steps == limit) {
            return MaybeInt{}
        }
        if (safeMod(current, 2) == 0) {
            current = safeDiv(current, 2)
        } else {
            current = ((3 * current) + 1)
        }
        steps = (steps + 1)
    }
    return MaybeInt{Found: true, Value: steps}
}
