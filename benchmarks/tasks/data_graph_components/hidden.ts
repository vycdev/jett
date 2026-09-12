import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.components !== 0n || actual.largest !== 0n || actual.isolated !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.components !== 5n || actual.largest !== 1n || actual.isolated !== 5n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.components !== 1n || actual.largest !== 1n || actual.isolated !== 1n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.components !== 2n || actual.largest !== 1n || actual.isolated !== 1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 1n }, { source: 2n, target: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.components !== 2n || actual.largest !== 2n || actual.isolated !== 1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 0n }, { source: 1n, target: 0n }, { source: 3n, target: 1n }];
    const actual: Report = solve(rows, 4n);
    if (actual.components !== 1n || actual.largest !== 4n || actual.isolated !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 2n }, { source: 1n, target: 3n }, { source: 1n, target: 3n }, { source: 2n, target: 0n }];
    const actual: Report = solve(rows, 5n);
    if (actual.components !== 3n || actual.largest !== 2n || actual.isolated !== 1n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ source: 4n, target: 4n }, { source: 5n, target: 1n }, { source: 0n, target: 1n }, { source: 0n, target: 3n }, { source: 5n, target: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.components !== 3n || actual.largest !== 4n || actual.isolated !== 1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ source: 6n, target: 4n }, { source: 3n, target: 5n }, { source: 2n, target: 3n }, { source: 6n, target: 5n }, { source: 4n, target: 4n }, { source: 4n, target: 0n }];
    const actual: Report = solve(rows, 7n);
    if (actual.components !== 2n || actual.largest !== 6n || actual.isolated !== 1n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ source: 4n, target: 1n }, { source: 2n, target: 0n }, { source: 6n, target: 6n }, { source: 6n, target: 0n }, { source: 7n, target: 2n }, { source: 0n, target: 0n }, { source: 3n, target: 6n }];
    const actual: Report = solve(rows, 8n);
    if (actual.components !== 3n || actual.largest !== 5n || actual.isolated !== 1n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.components !== 1n || actual.largest !== 1n || actual.isolated !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 0n }, { source: 1n, target: 1n }, { source: 0n, target: 0n }, { source: 0n, target: 1n }, { source: 1n, target: 1n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.components !== 1n || actual.largest !== 2n || actual.isolated !== 0n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 1n }, { source: 0n, target: 1n }, { source: 2n, target: 2n }, { source: 1n, target: 0n }, { source: 0n, target: 1n }, { source: 1n, target: 2n }, { source: 2n, target: 1n }, { source: 2n, target: 1n }, { source: 0n, target: 2n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.components !== 1n || actual.largest !== 3n || actual.isolated !== 0n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 2n }, { source: 3n, target: 1n }, { source: 1n, target: 1n }, { source: 1n, target: 1n }, { source: 1n, target: 1n }, { source: 1n, target: 3n }, { source: 2n, target: 2n }, { source: 2n, target: 1n }, { source: 0n, target: 2n }, { source: 1n, target: 2n }, { source: 1n, target: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.components !== 1n || actual.largest !== 4n || actual.isolated !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 3n }, { source: 3n, target: 0n }, { source: 3n, target: 1n }, { source: 0n, target: 3n }, { source: 4n, target: 3n }, { source: 3n, target: 2n }, { source: 1n, target: 3n }, { source: 3n, target: 3n }, { source: 2n, target: 0n }, { source: 1n, target: 3n }, { source: 2n, target: 0n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.components !== 1n || actual.largest !== 5n || actual.isolated !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.components !== 6n || actual.largest !== 1n || actual.isolated !== 6n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ source: 4n, target: 6n }];
    const actual: Report = solve(rows, 7n);
    if (actual.components !== 6n || actual.largest !== 2n || actual.isolated !== 5n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ source: 5n, target: 6n }, { source: 0n, target: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.components !== 6n || actual.largest !== 2n || actual.isolated !== 4n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 0n }, { source: 0n, target: 0n }, { source: 0n, target: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.components !== 1n || actual.largest !== 1n || actual.isolated !== 0n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 0n }, { source: 0n, target: 0n }, { source: 1n, target: 0n }, { source: 1n, target: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.components !== 1n || actual.largest !== 2n || actual.isolated !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 0n }, { source: 2n, target: 2n }, { source: 2n, target: 2n }, { source: 2n, target: 0n }, { source: 2n, target: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.components !== 2n || actual.largest !== 2n || actual.isolated !== 1n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 2n }, { source: 0n, target: 1n }, { source: 3n, target: 0n }, { source: 1n, target: 0n }, { source: 0n, target: 0n }, { source: 2n, target: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.components !== 1n || actual.largest !== 4n || actual.isolated !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 0n }, { source: 1n, target: 1n }, { source: 2n, target: 1n }, { source: 1n, target: 2n }, { source: 4n, target: 3n }, { source: 1n, target: 4n }, { source: 4n, target: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.components !== 1n || actual.largest !== 5n || actual.isolated !== 0n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 4n }, { source: 0n, target: 4n }, { source: 3n, target: 0n }, { source: 3n, target: 0n }, { source: 1n, target: 5n }, { source: 0n, target: 3n }, { source: 5n, target: 3n }, { source: 0n, target: 3n }];
    const actual: Report = solve(rows, 6n);
    if (actual.components !== 1n || actual.largest !== 6n || actual.isolated !== 0n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 3n }, { source: 0n, target: 1n }, { source: 1n, target: 2n }, { source: 4n, target: 4n }];
    const actual: Report = solve(rows, 6n);
    if (actual.components !== 3n || actual.largest !== 4n || actual.isolated !== 1n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n }, { source: 1n, target: 2n }, { source: 3n, target: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.components !== 3n || actual.largest !== 3n || actual.isolated !== 1n) throw new Error("fixture 25");
}
