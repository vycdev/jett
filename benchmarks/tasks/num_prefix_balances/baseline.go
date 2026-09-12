package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func PrefixBalances(values []int64, opening int64) []int64 {
    var balances []int64 = []int64{opening}
    var balance int64 = opening
    var index int64 = 0
    for (index < int64(len(values))) {
        balance = (balance + nth(values, index))
        balances = append(balances, balance)
        index = (index + 1)
    }
    return balances
}
