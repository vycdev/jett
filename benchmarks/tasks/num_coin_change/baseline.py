

def nth(values: list[int], index: int) -> int:
    return values[index]

def coin_change(coins: list[int], amount: int) -> int:
    costs: list[int] = [0]
    total: int = 1
    while total <= amount:
        best: int = 1000
        index: int = 0
        while index < len(coins):
            coin: int = nth(coins, index)
            if coin <= total:
                prior: int = nth(costs, total - coin)
                if prior + 1 < best:
                    best = prior + 1
            index = index + 1
        costs.append(best)
        total = total + 1
    answer: int = nth(costs, amount)
    if answer == 1000:
        return -1
    return answer
