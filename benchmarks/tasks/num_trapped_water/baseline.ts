function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
export function trappedWater(heights: readonly bigint[]): bigint {
    let leftPeaks: bigint[] = [];
    let peak: bigint = 0n;
    let index: bigint = 0n;
    while ((index < BigInt(heights.length))) {
        let height: bigint = nth(heights, index);
        if ((height > peak)) {
            peak = height;
        }
        leftPeaks.push(peak);
        index = (index + 1n);
    }
    peak = 0n;
    let volume: bigint = 0n;
    while ((index > 0n)) {
        index = (index - 1n);
        let height: bigint = nth(heights, index);
        if ((height > peak)) {
            peak = height;
        }
        let ceiling: bigint = nth(leftPeaks, index);
        if ((peak < ceiling)) {
            ceiling = peak;
        }
        volume = ((volume + ceiling) - height);
    }
    return volume;
}
