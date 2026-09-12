package benchmark
func safeDiv(left int64, right int64) int64 { return left / right }
func safeMod(left int64, right int64) int64 { return left % right }
func DigitChecksum(n int64) int64 {
    var remaining int64 = n
    var sign int64 = 1
    var checksum int64 = 0
    for (remaining > 0) {
        checksum = (checksum + (sign * safeMod(remaining, 10)))
        sign = -(sign)
        remaining = safeDiv(remaining, 10)
    }
    return checksum
}
