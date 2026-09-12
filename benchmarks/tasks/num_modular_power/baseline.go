package benchmark
func safeDiv(left int64, right int64) int64 { return left / right }
func safeMod(left int64, right int64) int64 { return left % right }
func ModularPower(base int64, exponent int64, modulus int64) int64 {
    var power int64 = safeMod(base, modulus)
    var remaining int64 = exponent
    var answer int64 = safeMod(1, modulus)
    for (remaining > 0) {
        if (safeMod(remaining, 2) == 1) {
            answer = safeMod((answer * power), modulus)
        }
        power = safeMod((power * power), modulus)
        remaining = safeDiv(remaining, 2)
    }
    return answer
}
