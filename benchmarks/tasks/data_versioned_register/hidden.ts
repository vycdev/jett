import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.accepted !== 0n || actual.stale !== 0n || actual.checksum !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ key: 3n, version: 8n, value: 3n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 1n || actual.stale !== 0n || actual.checksum !== 12n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ key: 1n, version: 6n, value: 3n }, { key: 3n, version: 7n, value: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.stale !== 0n || actual.checksum !== 26n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ key: 2n, version: 11n, value: 1n }, { key: 0n, version: 0n, value: 6n }, { key: 0n, version: (-2n), value: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.stale !== 1n || actual.checksum !== 9n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ key: 3n, version: 10n, value: 2n }, { key: 2n, version: 2n, value: 3n }, { key: 4n, version: 0n, value: 4n }, { key: 0n, version: 11n, value: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.stale !== 0n || actual.checksum !== 39n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ key: 3n, version: 10n, value: 3n }, { key: 0n, version: 5n, value: 5n }, { key: 4n, version: 2n, value: 6n }, { key: 1n, version: 3n, value: 1n }, { key: 1n, version: 1n, value: 1n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.stale !== 1n || actual.checksum !== 49n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ key: 3n, version: 5n, value: 6n }, { key: 1n, version: (-2n), value: 5n }, { key: 3n, version: 11n, value: 6n }, { key: 4n, version: 2n, value: 4n }, { key: 0n, version: 10n, value: 6n }, { key: 3n, version: 6n, value: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.stale !== 2n || actual.checksum !== 50n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ key: 1n, version: 5n, value: 1n }, { key: 1n, version: 1n, value: 1n }, { key: 3n, version: (-1n), value: 1n }, { key: 0n, version: 8n, value: 4n }, { key: 3n, version: 7n, value: 4n }, { key: 0n, version: 5n, value: 4n }, { key: 1n, version: 11n, value: 3n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.stale !== 3n || actual.checksum !== 26n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ key: 0n, version: (-3n), value: 3n }, { key: 1n, version: 7n, value: 4n }, { key: 1n, version: 7n, value: 1n }, { key: 3n, version: (-2n), value: 3n }, { key: 4n, version: (-1n), value: 4n }, { key: 0n, version: 4n, value: 5n }, { key: 2n, version: 8n, value: 4n }, { key: 1n, version: (-2n), value: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 3n || actual.stale !== 5n || actual.checksum !== 25n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ key: 2n, version: 10n, value: 2n }, { key: 2n, version: 11n, value: 2n }, { key: 4n, version: 2n, value: 2n }, { key: 3n, version: 1n, value: 2n }, { key: 2n, version: 1n, value: 5n }, { key: 2n, version: 4n, value: 2n }, { key: 0n, version: 2n, value: 1n }, { key: 3n, version: (-3n), value: 4n }, { key: 4n, version: 2n, value: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 5n || actual.stale !== 4n || actual.checksum !== 25n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ key: 0n, version: 7n, value: 1n }, { key: 3n, version: (-3n), value: 1n }, { key: 1n, version: 5n, value: 6n }, { key: 0n, version: 10n, value: 1n }, { key: 1n, version: 8n, value: 3n }, { key: 3n, version: 6n, value: 3n }, { key: 0n, version: 2n, value: 3n }, { key: 4n, version: 5n, value: 3n }, { key: 2n, version: (-1n), value: 2n }, { key: 0n, version: 1n, value: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 6n || actual.stale !== 4n || actual.checksum !== 34n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ key: 2n, version: 5n, value: 3n }, { key: 4n, version: 3n, value: 6n }, { key: 3n, version: 11n, value: 1n }, { key: 1n, version: 8n, value: 6n }, { key: 3n, version: 4n, value: 2n }, { key: 0n, version: 1n, value: 1n }, { key: 4n, version: 3n, value: 2n }, { key: 1n, version: 5n, value: 6n }, { key: 4n, version: 6n, value: 5n }, { key: 1n, version: 9n, value: 5n }, { key: 4n, version: 7n, value: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 8n || actual.stale !== 3n || actual.checksum !== 34n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ key: 1n, version: 3n, value: 2n }, { key: 1n, version: 7n, value: 2n }, { key: 3n, version: 4n, value: 6n }, { key: 2n, version: 6n, value: 3n }, { key: 2n, version: 11n, value: 3n }, { key: 1n, version: 0n, value: 5n }, { key: 0n, version: 2n, value: 2n }, { key: 1n, version: 10n, value: 3n }, { key: 1n, version: (-1n), value: 5n }, { key: 0n, version: (-1n), value: 1n }, { key: 3n, version: 11n, value: 6n }, { key: 3n, version: (-3n), value: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 8n || actual.stale !== 4n || actual.checksum !== 41n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ key: 3n, version: (-1n), value: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 0n || actual.stale !== 1n || actual.checksum !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ key: 0n, version: 4n, value: 1n }, { key: 4n, version: 8n, value: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.stale !== 0n || actual.checksum !== 21n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ key: 3n, version: 7n, value: 3n }, { key: 0n, version: (-1n), value: 4n }, { key: 0n, version: 3n, value: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.stale !== 1n || actual.checksum !== 16n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ key: 1n, version: 11n, value: 3n }, { key: 0n, version: 5n, value: 2n }, { key: 3n, version: 4n, value: 3n }, { key: 0n, version: 4n, value: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 3n || actual.stale !== 1n || actual.checksum !== 20n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ key: 1n, version: 1n, value: 5n }, { key: 2n, version: 5n, value: 4n }, { key: 4n, version: (-3n), value: 3n }, { key: 2n, version: 5n, value: 5n }, { key: 1n, version: (-3n), value: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.stale !== 3n || actual.checksum !== 22n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ key: 0n, version: 4n, value: 4n }, { key: 2n, version: 1n, value: 6n }, { key: 3n, version: 10n, value: 1n }, { key: 4n, version: (-3n), value: 1n }, { key: 2n, version: 4n, value: 1n }, { key: 1n, version: 4n, value: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 5n || actual.stale !== 1n || actual.checksum !== 23n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ key: 2n, version: 1n, value: 6n }, { key: 0n, version: 6n, value: 5n }, { key: 1n, version: 6n, value: 5n }, { key: 0n, version: 0n, value: 5n }, { key: 1n, version: 0n, value: 2n }, { key: 2n, version: 11n, value: 1n }, { key: 4n, version: (-1n), value: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.stale !== 3n || actual.checksum !== 18n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ key: 4n, version: 6n, value: 1n }, { key: 4n, version: (-1n), value: 1n }, { key: 0n, version: (-3n), value: 4n }, { key: 2n, version: 9n, value: 5n }, { key: 0n, version: (-1n), value: 2n }, { key: 0n, version: 2n, value: 2n }, { key: 1n, version: (-2n), value: 3n }, { key: 1n, version: (-1n), value: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 3n || actual.stale !== 5n || actual.checksum !== 22n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ key: 0n, version: 0n, value: 3n }, { key: 0n, version: 0n, value: 9n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 1n || actual.stale !== 1n || actual.checksum !== 3n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ key: 0n, version: (-1n), value: 8n }, { key: 0n, version: (-2n), value: 9n }, { key: 0n, version: 0n, value: 0n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 1n || actual.stale !== 2n || actual.checksum !== 0n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ key: 1n, version: 0n, value: 8n }, { key: 1n, version: 0n, value: 9n }, { key: 1n, version: 2n, value: (-3n) }, { key: 2n, version: (-1n), value: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.stale !== 2n || actual.checksum !== -6n) throw new Error("fixture 23");
}
