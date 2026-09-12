export function fibonacci(n: bigint): bigint {
    let previous: bigint = 0n;
    let current: bigint = 1n;
    let index: bigint = 0n;
    while ((index < n)) {
        let following: bigint = (previous + current);
        previous = current;
        current = following;
        index = (index + 1n);
    }
    return previous;
}
