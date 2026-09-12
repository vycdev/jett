import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.rooms !== 0n || actual.assignment_checksum !== 0n || actual.reuses !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ start: 11n, end: 15n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 1n || actual.assignment_checksum !== 1n || actual.reuses !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n }, { start: 6n, end: 12n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 2n || actual.assignment_checksum !== 5n || actual.reuses !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 2n }, { start: 1n, end: 6n }, { start: 10n, end: 13n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 2n || actual.assignment_checksum !== 8n || actual.reuses !== 1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 6n }, { start: 5n, end: 10n }, { start: 6n, end: 13n }, { start: 7n, end: 8n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 20n || actual.reuses !== 1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 5n }, { start: 4n, end: 5n }, { start: 6n, end: 7n }, { start: 8n, end: 15n }, { start: 9n, end: 12n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 2n || actual.assignment_checksum !== 22n || actual.reuses !== 3n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 2n }, { start: 1n, end: 8n }, { start: 5n, end: 9n }, { start: 7n, end: 13n }, { start: 9n, end: 11n }, { start: 10n, end: 14n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 37n || actual.reuses !== 3n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 1n }, { start: 0n, end: 3n }, { start: 1n, end: 5n }, { start: 5n, end: 12n }, { start: 6n, end: 12n }, { start: 7n, end: 14n }, { start: 11n, end: 12n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 4n || actual.assignment_checksum !== 68n || actual.reuses !== 3n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ start: 2n, end: 3n }, { start: 3n, end: 9n }, { start: 7n, end: 8n }, { start: 9n, end: 11n }, { start: 9n, end: 12n }, { start: 10n, end: 14n }, { start: 10n, end: 17n }, { start: 11n, end: 12n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 4n || actual.assignment_checksum !== 77n || actual.reuses !== 4n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 4n }, { start: 2n, end: 9n }, { start: 3n, end: 4n }, { start: 4n, end: 6n }, { start: 4n, end: 11n }, { start: 5n, end: 7n }, { start: 5n, end: 9n }, { start: 6n, end: 11n }, { start: 10n, end: 11n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 5n || actual.assignment_checksum !== 118n || actual.reuses !== 4n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 7n }, { start: 2n, end: 7n }, { start: 2n, end: 4n }, { start: 4n, end: 6n }, { start: 5n, end: 8n }, { start: 7n, end: 8n }, { start: 8n, end: 11n }, { start: 8n, end: 11n }, { start: 9n, end: 12n }, { start: 11n, end: 14n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 4n || actual.assignment_checksum !== 112n || actual.reuses !== 6n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 2n }, { start: 2n, end: 8n }, { start: 3n, end: 7n }, { start: 4n, end: 5n }, { start: 6n, end: 12n }, { start: 6n, end: 8n }, { start: 7n, end: 9n }, { start: 7n, end: 11n }, { start: 8n, end: 14n }, { start: 9n, end: 14n }, { start: 9n, end: 14n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 5n || actual.assignment_checksum !== 187n || actual.reuses !== 6n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 8n }, { start: 1n, end: 6n }, { start: 3n, end: 8n }, { start: 4n, end: 6n }, { start: 5n, end: 12n }, { start: 5n, end: 7n }, { start: 6n, end: 10n }, { start: 6n, end: 12n }, { start: 8n, end: 9n }, { start: 9n, end: 12n }, { start: 9n, end: 10n }, { start: 11n, end: 15n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 6n || actual.assignment_checksum !== 213n || actual.reuses !== 6n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 3n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 1n || actual.assignment_checksum !== 1n || actual.reuses !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 4n }, { start: 2n, end: 5n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 2n || actual.assignment_checksum !== 5n || actual.reuses !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 10n }, { start: 7n, end: 9n }, { start: 8n, end: 10n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 14n || actual.reuses !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 3n }, { start: 4n, end: 9n }, { start: 8n, end: 12n }, { start: 8n, end: 15n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 21n || actual.reuses !== 1n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ start: 6n, end: 9n }, { start: 8n, end: 9n }, { start: 10n, end: 12n }, { start: 10n, end: 11n }, { start: 10n, end: 14n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 31n || actual.reuses !== 2n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 7n }, { start: 4n, end: 10n }, { start: 5n, end: 9n }, { start: 9n, end: 16n }, { start: 9n, end: 16n }, { start: 10n, end: 12n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 45n || actual.reuses !== 3n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 6n }, { start: 2n, end: 9n }, { start: 3n, end: 8n }, { start: 3n, end: 5n }, { start: 8n, end: 13n }, { start: 8n, end: 10n }, { start: 10n, end: 11n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 4n || actual.assignment_checksum !== 67n || actual.reuses !== 3n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ start: 2n, end: 9n }, { start: 3n, end: 5n }, { start: 4n, end: 9n }, { start: 5n, end: 7n }, { start: 7n, end: 14n }, { start: 8n, end: 15n }, { start: 10n, end: 11n }, { start: 10n, end: 12n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 4n || actual.assignment_checksum !== 87n || actual.reuses !== 4n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 1n }, { start: 0n, end: 2n }, { start: 0n, end: 3n }, { start: 3n, end: 4n }, { start: 3n, end: 4n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 3n || actual.assignment_checksum !== 28n || actual.reuses !== 2n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 5n }, { start: 1n, end: 3n }, { start: 3n, end: 6n }, { start: 5n, end: 7n }];
    const actual: Report = solve(rows);
    if (actual.rooms !== 2n || actual.assignment_checksum !== 15n || actual.reuses !== 2n) throw new Error("fixture 22");
}
