
export interface Entry {
    readonly source: bigint;
    readonly target: bigint;
}

export interface Report {
    readonly sources: bigint;
    readonly sinks: bigint;
    readonly balanced: bigint;
}

export function solve(rows: readonly Entry[], limit: bigint): Report {
    let sources: bigint = 0n;
    let sinks: bigint = 0n;
    let balanced: bigint = 0n;
    let vertex: bigint = 0n;
    while ((vertex < limit)) {
        let incoming: bigint = 0n;
        let outgoing: bigint = 0n;
        for (const row of rows) {
            if ((row.source == vertex)) {
                outgoing = (outgoing + 1n);
            }
            if ((row.target == vertex)) {
                incoming = (incoming + 1n);
            }
        }
        if (((incoming == 0n) && (outgoing > 0n))) {
            sources = (sources + 1n);
        }
        if (((outgoing == 0n) && (incoming > 0n))) {
            sinks = (sinks + 1n);
        }
        if ((incoming == outgoing)) {
            balanced = (balanced + 1n);
        }
        vertex = (vertex + 1n);
    }
    return { sources: sources, sinks: sinks, balanced: balanced };
}
