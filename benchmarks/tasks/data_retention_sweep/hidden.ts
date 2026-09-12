import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ key: 3n, modified: 8n, size: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ key: 1n, modified: 6n, size: 3n }, { key: 3n, modified: 7n, size: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 2n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ key: 2n, modified: 11n, size: 1n }, { key: 0n, modified: 0n, size: 6n }, { key: 0n, modified: (-2n), size: 5n }];
    const actual: Report = solve(rows, 4n);
    if (actual.removed !== 1n || actual.reclaimed !== 5n || actual.retained !== 2n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ key: 3n, modified: 10n, size: 2n }, { key: 2n, modified: 2n, size: 3n }, { key: 4n, modified: 0n, size: 4n }, { key: 0n, modified: 11n, size: 2n }];
    const actual: Report = solve(rows, 5n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 4n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ key: 3n, modified: 10n, size: 3n }, { key: 0n, modified: 5n, size: 5n }, { key: 4n, modified: 2n, size: 6n }, { key: 1n, modified: 3n, size: 1n }, { key: 1n, modified: 1n, size: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.removed !== 1n || actual.reclaimed !== 1n || actual.retained !== 4n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ key: 3n, modified: 5n, size: 6n }, { key: 1n, modified: (-2n), size: 5n }, { key: 3n, modified: 11n, size: 6n }, { key: 4n, modified: 2n, size: 4n }, { key: 0n, modified: 10n, size: 6n }, { key: 3n, modified: 6n, size: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.removed !== 2n || actual.reclaimed !== 11n || actual.retained !== 4n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ key: 1n, modified: 5n, size: 1n }, { key: 1n, modified: 1n, size: 1n }, { key: 3n, modified: (-1n), size: 1n }, { key: 0n, modified: 8n, size: 4n }, { key: 3n, modified: 7n, size: 4n }, { key: 0n, modified: 5n, size: 4n }, { key: 1n, modified: 11n, size: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.removed !== 4n || actual.reclaimed !== 7n || actual.retained !== 3n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ key: 0n, modified: (-3n), size: 3n }, { key: 1n, modified: 7n, size: 4n }, { key: 1n, modified: 7n, size: 1n }, { key: 3n, modified: (-2n), size: 3n }, { key: 4n, modified: (-1n), size: 4n }, { key: 0n, modified: 4n, size: 5n }, { key: 2n, modified: 8n, size: 4n }, { key: 1n, modified: (-2n), size: 6n }];
    const actual: Report = solve(rows, 1n);
    if (actual.removed !== 2n || actual.reclaimed !== 9n || actual.retained !== 6n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ key: 2n, modified: 10n, size: 2n }, { key: 2n, modified: 11n, size: 2n }, { key: 4n, modified: 2n, size: 2n }, { key: 3n, modified: 1n, size: 2n }, { key: 2n, modified: 1n, size: 5n }, { key: 2n, modified: 4n, size: 2n }, { key: 0n, modified: 2n, size: 1n }, { key: 3n, modified: (-3n), size: 4n }, { key: 4n, modified: 2n, size: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.removed !== 2n || actual.reclaimed !== 9n || actual.retained !== 7n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ key: 0n, modified: 7n, size: 1n }, { key: 3n, modified: (-3n), size: 1n }, { key: 1n, modified: 5n, size: 6n }, { key: 0n, modified: 10n, size: 1n }, { key: 1n, modified: 8n, size: 3n }, { key: 3n, modified: 6n, size: 3n }, { key: 0n, modified: 2n, size: 3n }, { key: 4n, modified: 5n, size: 3n }, { key: 2n, modified: (-1n), size: 2n }, { key: 0n, modified: 1n, size: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.removed !== 3n || actual.reclaimed !== 6n || actual.retained !== 7n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ key: 2n, modified: 5n, size: 3n }, { key: 4n, modified: 3n, size: 6n }, { key: 3n, modified: 11n, size: 1n }, { key: 1n, modified: 8n, size: 6n }, { key: 3n, modified: 4n, size: 2n }, { key: 0n, modified: 1n, size: 1n }, { key: 4n, modified: 3n, size: 2n }, { key: 1n, modified: 5n, size: 6n }, { key: 4n, modified: 6n, size: 5n }, { key: 1n, modified: 9n, size: 5n }, { key: 4n, modified: 7n, size: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.removed !== 2n || actual.reclaimed !== 8n || actual.retained !== 9n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ key: 1n, modified: 3n, size: 2n }, { key: 1n, modified: 7n, size: 2n }, { key: 3n, modified: 4n, size: 6n }, { key: 2n, modified: 6n, size: 3n }, { key: 2n, modified: 11n, size: 3n }, { key: 1n, modified: 0n, size: 5n }, { key: 0n, modified: 2n, size: 2n }, { key: 1n, modified: 10n, size: 3n }, { key: 1n, modified: (-1n), size: 5n }, { key: 0n, modified: (-1n), size: 1n }, { key: 3n, modified: 11n, size: 6n }, { key: 3n, modified: (-3n), size: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.removed !== 6n || actual.reclaimed !== 23n || actual.retained !== 6n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ key: 3n, modified: (-1n), size: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 1n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ key: 0n, modified: 4n, size: 1n }, { key: 4n, modified: 8n, size: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 2n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ key: 3n, modified: 7n, size: 3n }, { key: 0n, modified: (-1n), size: 4n }, { key: 0n, modified: 3n, size: 4n }];
    const actual: Report = solve(rows, 1n);
    if (actual.removed !== 1n || actual.reclaimed !== 4n || actual.retained !== 2n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ key: 1n, modified: 11n, size: 3n }, { key: 0n, modified: 5n, size: 2n }, { key: 3n, modified: 4n, size: 3n }, { key: 0n, modified: 4n, size: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 4n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ key: 1n, modified: 1n, size: 5n }, { key: 2n, modified: 5n, size: 4n }, { key: 4n, modified: (-3n), size: 3n }, { key: 2n, modified: 5n, size: 5n }, { key: 1n, modified: (-3n), size: 6n }];
    const actual: Report = solve(rows, 3n);
    if (actual.removed !== 1n || actual.reclaimed !== 6n || actual.retained !== 4n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ key: 0n, modified: 4n, size: 4n }, { key: 2n, modified: 1n, size: 6n }, { key: 3n, modified: 10n, size: 1n }, { key: 4n, modified: (-3n), size: 1n }, { key: 2n, modified: 4n, size: 1n }, { key: 1n, modified: 4n, size: 6n }];
    const actual: Report = solve(rows, 4n);
    if (actual.removed !== 1n || actual.reclaimed !== 6n || actual.retained !== 5n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ key: 2n, modified: 1n, size: 6n }, { key: 0n, modified: 6n, size: 5n }, { key: 1n, modified: 6n, size: 5n }, { key: 0n, modified: 0n, size: 5n }, { key: 1n, modified: 0n, size: 2n }, { key: 2n, modified: 11n, size: 1n }, { key: 4n, modified: (-1n), size: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.removed !== 3n || actual.reclaimed !== 13n || actual.retained !== 4n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ key: 4n, modified: 6n, size: 1n }, { key: 4n, modified: (-1n), size: 1n }, { key: 0n, modified: (-3n), size: 4n }, { key: 2n, modified: 9n, size: 5n }, { key: 0n, modified: (-1n), size: 2n }, { key: 0n, modified: 2n, size: 2n }, { key: 1n, modified: (-2n), size: 3n }, { key: 1n, modified: (-1n), size: 6n }];
    const actual: Report = solve(rows, 6n);
    if (actual.removed !== 4n || actual.reclaimed !== 10n || actual.retained !== 4n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ key: 0n, modified: 2n, size: 3n }, { key: 0n, modified: 2n, size: 7n }];
    const actual: Report = solve(rows, 3n);
    if (actual.removed !== 1n || actual.reclaimed !== 3n || actual.retained !== 1n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ key: 0n, modified: 3n, size: 5n }, { key: 0n, modified: 4n, size: 2n }, { key: 1n, modified: 1n, size: 9n }];
    const actual: Report = solve(rows, 3n);
    if (actual.removed !== 0n || actual.reclaimed !== 0n || actual.retained !== 3n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ key: 1n, modified: 2n, size: 5n }, { key: 1n, modified: 2n, size: 7n }, { key: 2n, modified: 1n, size: 9n }, { key: 1n, modified: 5n, size: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.removed !== 2n || actual.reclaimed !== 12n || actual.retained !== 2n) throw new Error("fixture 26");
}
