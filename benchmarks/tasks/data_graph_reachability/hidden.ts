import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.reachable !== 0n || actual.unreachable !== 0n || actual.reachable_edge_count !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 1n || actual.unreachable !== 4n || actual.reachable_edge_count !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.reachable !== 1n || actual.unreachable !== 0n || actual.reachable_edge_count !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.reachable !== 1n || actual.unreachable !== 1n || actual.reachable_edge_count !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 1n }, { source: 2n, target: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.reachable !== 1n || actual.unreachable !== 2n || actual.reachable_edge_count !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 0n }, { source: 1n, target: 0n }, { source: 3n, target: 1n }];
    const actual: Report = solve(rows, 4n);
    if (actual.reachable !== 1n || actual.unreachable !== 3n || actual.reachable_edge_count !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 2n }, { source: 1n, target: 3n }, { source: 1n, target: 3n }, { source: 2n, target: 0n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 1n || actual.unreachable !== 4n || actual.reachable_edge_count !== 0n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ source: 4n, target: 4n }, { source: 5n, target: 1n }, { source: 0n, target: 1n }, { source: 0n, target: 3n }, { source: 5n, target: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.reachable !== 3n || actual.unreachable !== 3n || actual.reachable_edge_count !== 2n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ source: 6n, target: 4n }, { source: 3n, target: 5n }, { source: 2n, target: 3n }, { source: 6n, target: 5n }, { source: 4n, target: 4n }, { source: 4n, target: 0n }];
    const actual: Report = solve(rows, 7n);
    if (actual.reachable !== 1n || actual.unreachable !== 6n || actual.reachable_edge_count !== 0n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ source: 4n, target: 1n }, { source: 2n, target: 0n }, { source: 6n, target: 6n }, { source: 6n, target: 0n }, { source: 7n, target: 2n }, { source: 0n, target: 0n }, { source: 3n, target: 6n }];
    const actual: Report = solve(rows, 8n);
    if (actual.reachable !== 1n || actual.unreachable !== 7n || actual.reachable_edge_count !== 1n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.reachable !== 1n || actual.unreachable !== 0n || actual.reachable_edge_count !== 8n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 0n }, { source: 1n, target: 1n }, { source: 0n, target: 0n }, { source: 0n, target: 1n }, { source: 1n, target: 1n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.reachable !== 2n || actual.unreachable !== 0n || actual.reachable_edge_count !== 9n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 1n }, { source: 0n, target: 1n }, { source: 2n, target: 2n }, { source: 1n, target: 0n }, { source: 0n, target: 1n }, { source: 1n, target: 2n }, { source: 2n, target: 1n }, { source: 2n, target: 1n }, { source: 0n, target: 2n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.reachable !== 3n || actual.unreachable !== 0n || actual.reachable_edge_count !== 10n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 2n }, { source: 3n, target: 1n }, { source: 1n, target: 1n }, { source: 1n, target: 1n }, { source: 1n, target: 1n }, { source: 1n, target: 3n }, { source: 2n, target: 2n }, { source: 2n, target: 1n }, { source: 0n, target: 2n }, { source: 1n, target: 2n }, { source: 1n, target: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.reachable !== 4n || actual.unreachable !== 0n || actual.reachable_edge_count !== 11n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 3n }, { source: 3n, target: 0n }, { source: 3n, target: 1n }, { source: 0n, target: 3n }, { source: 4n, target: 3n }, { source: 3n, target: 2n }, { source: 1n, target: 3n }, { source: 3n, target: 3n }, { source: 2n, target: 0n }, { source: 1n, target: 3n }, { source: 2n, target: 0n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 4n || actual.unreachable !== 1n || actual.reachable_edge_count !== 11n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.reachable !== 1n || actual.unreachable !== 5n || actual.reachable_edge_count !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ source: 4n, target: 6n }];
    const actual: Report = solve(rows, 7n);
    if (actual.reachable !== 1n || actual.unreachable !== 6n || actual.reachable_edge_count !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ source: 5n, target: 6n }, { source: 0n, target: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.reachable !== 2n || actual.unreachable !== 6n || actual.reachable_edge_count !== 1n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.reachable !== 1n || actual.unreachable !== 0n || actual.reachable_edge_count !== 3n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 0n }, { source: 0n, target: 0n }, { source: 1n, target: 0n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.reachable !== 1n || actual.unreachable !== 1n || actual.reachable_edge_count !== 1n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 0n }, { source: 2n, target: 2n }, { source: 2n, target: 2n }, { source: 2n, target: 0n }, { source: 2n, target: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.reachable !== 1n || actual.unreachable !== 2n || actual.reachable_edge_count !== 0n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 2n }, { source: 0n, target: 1n }, { source: 3n, target: 0n }, { source: 1n, target: 0n }, { source: 0n, target: 0n }, { source: 2n, target: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.reachable !== 3n || actual.unreachable !== 1n || actual.reachable_edge_count !== 5n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 0n }, { source: 1n, target: 1n }, { source: 2n, target: 1n }, { source: 1n, target: 2n }, { source: 4n, target: 3n }, { source: 1n, target: 4n }, { source: 4n, target: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 1n || actual.unreachable !== 4n || actual.reachable_edge_count !== 0n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 4n }, { source: 0n, target: 4n }, { source: 3n, target: 0n }, { source: 3n, target: 0n }, { source: 1n, target: 5n }, { source: 0n, target: 3n }, { source: 5n, target: 3n }, { source: 0n, target: 3n }];
    const actual: Report = solve(rows, 6n);
    if (actual.reachable !== 3n || actual.unreachable !== 3n || actual.reachable_edge_count !== 5n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 3n }, { source: 1n, target: 2n }, { source: 0n, target: 1n }, { source: 3n, target: 1n }, { source: 0n, target: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 4n || actual.unreachable !== 1n || actual.reachable_edge_count !== 5n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n }, { source: 1n, target: 0n }, { source: 3n, target: 4n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 2n || actual.unreachable !== 3n || actual.reachable_edge_count !== 3n) throw new Error("fixture 25");
}
