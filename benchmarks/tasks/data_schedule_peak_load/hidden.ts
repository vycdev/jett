import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.peak !== 0n || actual.earliest !== -1n || actual.overloaded_starts !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.peak !== 0n || actual.earliest !== -1n || actual.overloaded_starts !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.peak !== 0n || actual.earliest !== -1n || actual.overloaded_starts !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ start: 11n, end: 15n, demand: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.peak !== 3n || actual.earliest !== 11n || actual.overloaded_starts !== 1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n, demand: 3n }, { start: 6n, end: 12n, demand: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.peak !== 8n || actual.earliest !== 6n || actual.overloaded_starts !== 1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 13n, demand: 1n }, { start: 0n, end: 2n, demand: 1n }, { start: 1n, end: 6n, demand: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.peak !== 5n || actual.earliest !== 1n || actual.overloaded_starts !== 1n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 6n, demand: 3n }, { start: 5n, end: 10n, demand: 2n }, { start: 7n, end: 8n, demand: 2n }, { start: 6n, end: 13n, demand: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.peak !== 7n || actual.earliest !== 7n || actual.overloaded_starts !== 1n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 5n, demand: 5n }, { start: 9n, end: 12n, demand: 2n }, { start: 6n, end: 7n, demand: 2n }, { start: 4n, end: 5n, demand: 4n }, { start: 8n, end: 15n, demand: 2n }];
    const actual: Report = solve(rows, 6n);
    if (actual.peak !== 9n || actual.earliest !== 4n || actual.overloaded_starts !== 1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 8n, demand: 5n }, { start: 7n, end: 13n, demand: 5n }, { start: 5n, end: 9n, demand: 1n }, { start: 10n, end: 14n, demand: 5n }, { start: 9n, end: 11n, demand: 5n }, { start: 0n, end: 2n, demand: 3n }];
    const actual: Report = solve(rows, 7n);
    if (actual.peak !== 15n || actual.earliest !== 10n || actual.overloaded_starts !== 4n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 5n, demand: 2n }, { start: 0n, end: 1n, demand: 4n }, { start: 6n, end: 12n, demand: 4n }, { start: 11n, end: 12n, demand: 5n }, { start: 7n, end: 14n, demand: 2n }, { start: 5n, end: 12n, demand: 1n }, { start: 0n, end: 3n, demand: 2n }];
    const actual: Report = solve(rows, 8n);
    if (actual.peak !== 12n || actual.earliest !== 11n || actual.overloaded_starts !== 1n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 14n, demand: 2n }, { start: 10n, end: 17n, demand: 1n }, { start: 7n, end: 8n, demand: 3n }, { start: 9n, end: 11n, demand: 4n }, { start: 11n, end: 12n, demand: 4n }, { start: 9n, end: 12n, demand: 4n }, { start: 2n, end: 3n, demand: 3n }, { start: 3n, end: 9n, demand: 3n }];
    const actual: Report = solve(rows, 1n);
    if (actual.peak !== 11n || actual.earliest !== 10n || actual.overloaded_starts !== 6n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ start: 2n, end: 9n, demand: 5n }, { start: 5n, end: 7n, demand: 4n }, { start: 4n, end: 6n, demand: 3n }, { start: 4n, end: 11n, demand: 5n }, { start: 5n, end: 9n, demand: 2n }, { start: 10n, end: 11n, demand: 3n }, { start: 0n, end: 4n, demand: 1n }, { start: 6n, end: 11n, demand: 3n }, { start: 3n, end: 4n, demand: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.peak !== 19n || actual.earliest !== 5n || actual.overloaded_starts !== 6n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 8n, demand: 1n }, { start: 2n, end: 7n, demand: 1n }, { start: 1n, end: 7n, demand: 2n }, { start: 11n, end: 14n, demand: 4n }, { start: 9n, end: 12n, demand: 1n }, { start: 5n, end: 8n, demand: 5n }, { start: 8n, end: 11n, demand: 3n }, { start: 2n, end: 4n, demand: 1n }, { start: 4n, end: 6n, demand: 3n }, { start: 8n, end: 11n, demand: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.peak !== 11n || actual.earliest !== 5n || actual.overloaded_starts !== 7n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ start: 6n, end: 12n, demand: 4n }, { start: 0n, end: 2n, demand: 4n }, { start: 7n, end: 9n, demand: 1n }, { start: 4n, end: 5n, demand: 5n }, { start: 6n, end: 8n, demand: 2n }, { start: 8n, end: 14n, demand: 5n }, { start: 9n, end: 14n, demand: 2n }, { start: 9n, end: 14n, demand: 2n }, { start: 3n, end: 7n, demand: 2n }, { start: 2n, end: 8n, demand: 2n }, { start: 7n, end: 11n, demand: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.peak !== 16n || actual.earliest !== 9n || actual.overloaded_starts !== 5n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ start: 9n, end: 12n, demand: 3n }, { start: 5n, end: 12n, demand: 2n }, { start: 3n, end: 8n, demand: 1n }, { start: 5n, end: 7n, demand: 2n }, { start: 4n, end: 6n, demand: 2n }, { start: 9n, end: 10n, demand: 2n }, { start: 1n, end: 8n, demand: 4n }, { start: 11n, end: 15n, demand: 1n }, { start: 6n, end: 10n, demand: 2n }, { start: 8n, end: 9n, demand: 4n }, { start: 1n, end: 6n, demand: 4n }, { start: 6n, end: 12n, demand: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.peak !== 15n || actual.earliest !== 5n || actual.overloaded_starts !== 8n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.peak !== 0n || actual.earliest !== -1n || actual.overloaded_starts !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 3n, demand: 4n }];
    const actual: Report = solve(rows, 7n);
    if (actual.peak !== 4n || actual.earliest !== 1n || actual.overloaded_starts !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 4n, demand: 4n }, { start: 2n, end: 5n, demand: 1n }];
    const actual: Report = solve(rows, 8n);
    if (actual.peak !== 5n || actual.earliest !== 2n || actual.overloaded_starts !== 0n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ start: 8n, end: 10n, demand: 4n }, { start: 7n, end: 10n, demand: 1n }, { start: 7n, end: 9n, demand: 2n }];
    const actual: Report = solve(rows, 1n);
    if (actual.peak !== 7n || actual.earliest !== 8n || actual.overloaded_starts !== 2n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ start: 4n, end: 9n, demand: 3n }, { start: 8n, end: 12n, demand: 5n }, { start: 0n, end: 3n, demand: 3n }, { start: 8n, end: 15n, demand: 5n }];
    const actual: Report = solve(rows, 2n);
    if (actual.peak !== 13n || actual.earliest !== 8n || actual.overloaded_starts !== 3n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 12n, demand: 1n }, { start: 10n, end: 11n, demand: 4n }, { start: 6n, end: 9n, demand: 3n }, { start: 10n, end: 14n, demand: 1n }, { start: 8n, end: 9n, demand: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.peak !== 6n || actual.earliest !== 10n || actual.overloaded_starts !== 2n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ start: 5n, end: 9n, demand: 1n }, { start: 3n, end: 7n, demand: 3n }, { start: 4n, end: 10n, demand: 1n }, { start: 9n, end: 16n, demand: 5n }, { start: 10n, end: 12n, demand: 5n }, { start: 9n, end: 16n, demand: 1n }];
    const actual: Report = solve(rows, 4n);
    if (actual.peak !== 11n || actual.earliest !== 10n || actual.overloaded_starts !== 3n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n, demand: 2n }, { start: 3n, end: 5n, demand: 3n }, { start: 0n, end: 6n, demand: 5n }, { start: 2n, end: 9n, demand: 4n }, { start: 8n, end: 13n, demand: 1n }, { start: 8n, end: 10n, demand: 1n }, { start: 10n, end: 11n, demand: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.peak !== 14n || actual.earliest !== 3n || actual.overloaded_starts !== 3n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 14n, demand: 3n }, { start: 8n, end: 15n, demand: 1n }, { start: 2n, end: 9n, demand: 2n }, { start: 10n, end: 11n, demand: 3n }, { start: 3n, end: 5n, demand: 1n }, { start: 5n, end: 7n, demand: 2n }, { start: 10n, end: 12n, demand: 3n }, { start: 4n, end: 9n, demand: 4n }];
    const actual: Report = solve(rows, 6n);
    if (actual.peak !== 10n || actual.earliest !== 8n || actual.overloaded_starts !== 5n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 2n, demand: 3n }, { start: 2n, end: 4n, demand: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.peak !== 3n || actual.earliest !== 0n || actual.overloaded_starts !== 0n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ start: 2n, end: 4n, demand: 5n }, { start: 0n, end: 2n, demand: 5n }, { start: 0n, end: 2n, demand: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.peak !== 6n || actual.earliest !== 0n || actual.overloaded_starts !== 1n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ start: 4n, end: 8n, demand: 2n }, { start: 0n, end: 4n, demand: 3n }, { start: 2n, end: 6n, demand: 4n }, { start: 2n, end: 3n, demand: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.peak !== 8n || actual.earliest !== 2n || actual.overloaded_starts !== 2n) throw new Error("fixture 26");
}
