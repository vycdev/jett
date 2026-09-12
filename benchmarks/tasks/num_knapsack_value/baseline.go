package benchmark
func nth(values []int64, index int64) int64 { return values[index] }
func KnapsackValue(weights []int64, values []int64, capacity int64) int64 {
    var costs []int64 = []int64{}
    var size int64 = 0
    for (size <= capacity) {
        costs = append(costs, 0)
        size = (size + 1)
    }
    var item int64 = 0
    for (item < int64(len(weights))) {
        var nextCosts []int64 = []int64{}
        var room int64 = 0
        var weight int64 = nth(weights, item)
        var value int64 = nth(values, item)
        for (room <= capacity) {
            var best int64 = nth(costs, room)
            if (weight <= room) {
                var candidate int64 = (nth(costs, (room - weight)) + value)
                if (candidate > best) {
                    best = candidate
                }
            }
            nextCosts = append(nextCosts, best)
            room = (room + 1)
        }
        costs = nextCosts
        item = (item + 1)
    }
    return nth(costs, capacity)
}
