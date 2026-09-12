function nth(values: readonly bigint[], index: bigint): bigint { return values[Number(index)]; }
function hasStep(blocked: readonly bigint[], step: bigint): boolean {
    let index: bigint = 0n;
    while ((index < BigInt(blocked.length))) {
        if ((nth(blocked, index) == step)) {
            return true;
        }
        index = (index + 1n);
    }
    return false;
}
export function staircaseBlocked(height: bigint, blocked: readonly bigint[]): bigint {
    let previous: bigint = 0n;
    let current: bigint = 1n;
    let step: bigint = 1n;
    while ((step <= height)) {
        let following: bigint = (current + previous);
        if (hasStep(blocked, step)) {
            following = 0n;
        }
        previous = current;
        current = following;
        step = (step + 1n);
    }
    return current;
}
