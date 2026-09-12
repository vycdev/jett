

def nth(values: list[int], index: int) -> int:
    return values[index]

def knapsack_value(weights: list[int], values: list[int], capacity: int) -> int:
    costs: list[int] = []
    size: int = 0
    while size <= capacity:
        costs.append(0)
        size = size + 1
    item: int = 0
    while item < len(weights):
        next_costs: list[int] = []
        room: int = 0
        weight: int = nth(weights, item)
        value: int = nth(values, item)
        while room <= capacity:
            best: int = nth(costs, room)
            if weight <= room:
                candidate: int = nth(costs, room - weight) + value
                if candidate > best:
                    best = candidate
            next_costs.append(best)
            room = room + 1
        costs = next_costs
        item = item + 1
    return nth(costs, capacity)
