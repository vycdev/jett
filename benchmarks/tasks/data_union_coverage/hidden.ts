import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.covered !== 0n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 5n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 1n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ start: 11n, end: 15n }];
    const actual: Report = solve(rows, 2n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 2n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n }, { start: 6n, end: 12n }];
    const actual: Report = solve(rows, 3n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 3n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 13n }, { start: 0n, end: 2n }, { start: 1n, end: 6n }];
    const actual: Report = solve(rows, 4n);
    if (actual.covered !== 4n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 6n }, { start: 5n, end: 10n }, { start: 7n, end: 8n }, { start: 6n, end: 13n }];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 2n || actual.gaps !== 1n || actual.longest_gap !== 3n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 5n }, { start: 9n, end: 12n }, { start: 6n, end: 7n }, { start: 4n, end: 5n }, { start: 8n, end: 15n }];
    const actual: Report = solve(rows, 6n);
    if (actual.covered !== 5n || actual.gaps !== 1n || actual.longest_gap !== 1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 8n }, { start: 7n, end: 13n }, { start: 5n, end: 9n }, { start: 10n, end: 14n }, { start: 9n, end: 11n }, { start: 0n, end: 2n }];
    const actual: Report = solve(rows, 7n);
    if (actual.covered !== 7n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 5n }, { start: 0n, end: 1n }, { start: 6n, end: 12n }, { start: 11n, end: 12n }, { start: 7n, end: 14n }, { start: 5n, end: 12n }, { start: 0n, end: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.covered !== 8n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 14n }, { start: 10n, end: 17n }, { start: 7n, end: 8n }, { start: 9n, end: 11n }, { start: 11n, end: 12n }, { start: 9n, end: 12n }, { start: 2n, end: 3n }, { start: 3n, end: 9n }];
    const actual: Report = solve(rows, 1n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 1n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ start: 2n, end: 9n }, { start: 5n, end: 7n }, { start: 4n, end: 6n }, { start: 4n, end: 11n }, { start: 5n, end: 9n }, { start: 10n, end: 11n }, { start: 0n, end: 4n }, { start: 6n, end: 11n }, { start: 3n, end: 4n }];
    const actual: Report = solve(rows, 2n);
    if (actual.covered !== 2n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 8n }, { start: 2n, end: 7n }, { start: 1n, end: 7n }, { start: 11n, end: 14n }, { start: 9n, end: 12n }, { start: 5n, end: 8n }, { start: 8n, end: 11n }, { start: 2n, end: 4n }, { start: 4n, end: 6n }, { start: 8n, end: 11n }];
    const actual: Report = solve(rows, 3n);
    if (actual.covered !== 2n || actual.gaps !== 1n || actual.longest_gap !== 1n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ start: 6n, end: 12n }, { start: 0n, end: 2n }, { start: 7n, end: 9n }, { start: 4n, end: 5n }, { start: 6n, end: 8n }, { start: 8n, end: 14n }, { start: 9n, end: 14n }, { start: 9n, end: 14n }, { start: 3n, end: 7n }, { start: 2n, end: 8n }, { start: 7n, end: 11n }];
    const actual: Report = solve(rows, 4n);
    if (actual.covered !== 4n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ start: 9n, end: 12n }, { start: 5n, end: 12n }, { start: 3n, end: 8n }, { start: 5n, end: 7n }, { start: 4n, end: 6n }, { start: 9n, end: 10n }, { start: 1n, end: 8n }, { start: 11n, end: 15n }, { start: 6n, end: 10n }, { start: 8n, end: 9n }, { start: 1n, end: 6n }, { start: 6n, end: 12n }];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 4n || actual.gaps !== 1n || actual.longest_gap !== 1n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 6n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 3n }];
    const actual: Report = solve(rows, 7n);
    if (actual.covered !== 2n || actual.gaps !== 2n || actual.longest_gap !== 4n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 4n }, { start: 2n, end: 5n }];
    const actual: Report = solve(rows, 8n);
    if (actual.covered !== 5n || actual.gaps !== 1n || actual.longest_gap !== 3n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ start: 8n, end: 10n }, { start: 7n, end: 10n }, { start: 7n, end: 9n }];
    const actual: Report = solve(rows, 1n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 1n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ start: 4n, end: 9n }, { start: 8n, end: 12n }, { start: 0n, end: 3n }, { start: 8n, end: 15n }];
    const actual: Report = solve(rows, 2n);
    if (actual.covered !== 2n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 12n }, { start: 10n, end: 11n }, { start: 6n, end: 9n }, { start: 10n, end: 14n }, { start: 8n, end: 9n }];
    const actual: Report = solve(rows, 3n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 3n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ start: 5n, end: 9n }, { start: 3n, end: 7n }, { start: 4n, end: 10n }, { start: 9n, end: 16n }, { start: 10n, end: 12n }, { start: 9n, end: 16n }];
    const actual: Report = solve(rows, 4n);
    if (actual.covered !== 1n || actual.gaps !== 1n || actual.longest_gap !== 3n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n }, { start: 3n, end: 5n }, { start: 0n, end: 6n }, { start: 2n, end: 9n }, { start: 8n, end: 13n }, { start: 8n, end: 10n }, { start: 10n, end: 11n }];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 5n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 14n }, { start: 8n, end: 15n }, { start: 2n, end: 9n }, { start: 10n, end: 11n }, { start: 3n, end: 5n }, { start: 5n, end: 7n }, { start: 10n, end: 12n }, { start: 4n, end: 9n }];
    const actual: Report = solve(rows, 6n);
    if (actual.covered !== 4n || actual.gaps !== 1n || actual.longest_gap !== 2n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 3n }, { start: 3n, end: 5n }];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 5n || actual.gaps !== 0n || actual.longest_gap !== 0n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ start: 9n, end: 12n }];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 0n || actual.gaps !== 1n || actual.longest_gap !== 5n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 1n }, { start: 4n, end: 5n }];
    const actual: Report = solve(rows, 5n);
    if (actual.covered !== 2n || actual.gaps !== 1n || actual.longest_gap !== 3n) throw new Error("fixture 26");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 3n }, { start: 2n, end: 4n }, { start: 6n, end: 9n }];
    const actual: Report = solve(rows, 8n);
    if (actual.covered !== 5n || actual.gaps !== 2n || actual.longest_gap !== 2n) throw new Error("fixture 27");
}
