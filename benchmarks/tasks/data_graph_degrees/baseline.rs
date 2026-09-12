#[derive(Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: i64,
    pub target: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    pub sources: i64,
    pub sinks: i64,
    pub balanced: i64,
}

pub fn solve(rows: &[Entry], limit: i64) -> Report {
    let mut sources: i64 = 0;
    let mut sinks: i64 = 0;
    let mut balanced: i64 = 0;
    let mut vertex: i64 = 0;
    while (vertex < limit) {
        let mut incoming: i64 = 0;
        let mut outgoing: i64 = 0;
        for row in rows {
            if (row.source == vertex) {
                outgoing = (outgoing + 1);
            }
            if (row.target == vertex) {
                incoming = (incoming + 1);
            }
        }
        if ((incoming == 0) && (outgoing > 0)) {
            sources = (sources + 1);
        }
        if ((outgoing == 0) && (incoming > 0)) {
            sinks = (sinks + 1);
        }
        if (incoming == outgoing) {
            balanced = (balanced + 1);
        }
        vertex = (vertex + 1);
    }
    return Report {
        sources: sources,
        sinks: sinks,
        balanced: balanced,
    };
}
