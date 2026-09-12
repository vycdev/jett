package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func CoinChange(coins []int64, amount int64) int64 {
    var costs []int64 = []int64{0}
    var total int64 = 1
    for (total <= amount) {
        var best int64 = 1000
        var index int64 = 0
        for (index < int64(len(coins))) {
            var coin int64 = nth(coins, index)
            if (coin <= total) {
                var prior int64 = nth(costs, (total - coin))
                if ((prior + 1) < best) {
                    best = (prior + 1)
                }
            }
            index = (index + 1)
        }
        costs = append(costs, best)
        total = (total + 1)
    }
    var answer int64 = nth(costs, amount)
    if (answer == 1000) {
        return -(1)
    }
    return answer
}
