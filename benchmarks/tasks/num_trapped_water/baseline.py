

def nth(values: list[int], index: int) -> int:
    return values[index]

def trapped_water(heights: list[int]) -> int:
    left_peaks: list[int] = []
    peak: int = 0
    index: int = 0
    while index < len(heights):
        height: int = nth(heights, index)
        if height > peak:
            peak = height
        left_peaks.append(peak)
        index = index + 1
    peak = 0
    volume: int = 0
    while index > 0:
        index = index - 1
        height: int = nth(heights, index)
        if height > peak:
            peak = height
        ceiling: int = nth(left_peaks, index)
        if peak < ceiling:
            ceiling = peak
        volume = volume + ceiling - height
    return volume
