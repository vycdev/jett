import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.applied !== 0n || actual.duplicates !== 0n || actual.checksum !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ event: 3n, key: 2n, delta: 1n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 1n || actual.duplicates !== 0n || actual.checksum !== 3n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ event: 4n, key: 2n, delta: 4n }, { event: 4n, key: 2n, delta: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.applied !== 1n || actual.duplicates !== 1n || actual.checksum !== 12n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ event: 0n, key: 1n, delta: 8n }, { event: 0n, key: 0n, delta: 6n }, { event: 3n, key: 1n, delta: 3n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 2n || actual.duplicates !== 1n || actual.checksum !== 22n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ event: 2n, key: 2n, delta: 7n }, { event: 1n, key: 3n, delta: (-1n) }, { event: 1n, key: 3n, delta: 3n }, { event: 0n, key: 2n, delta: 9n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 1n || actual.checksum !== 44n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ event: 1n, key: 3n, delta: (-2n) }, { event: 1n, key: 2n, delta: (-1n) }, { event: 3n, key: 1n, delta: (-1n) }, { event: 4n, key: 3n, delta: 9n }, { event: 4n, key: 2n, delta: 5n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 2n || actual.checksum !== 26n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ event: 0n, key: 3n, delta: 7n }, { event: 4n, key: 1n, delta: 6n }, { event: 0n, key: 1n, delta: 2n }, { event: 0n, key: 3n, delta: 0n }, { event: 0n, key: 0n, delta: 9n }, { event: 3n, key: 3n, delta: 8n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 3n || actual.checksum !== 72n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ event: 3n, key: 0n, delta: 6n }, { event: 3n, key: 1n, delta: 3n }, { event: 0n, key: 0n, delta: 2n }, { event: 1n, key: 3n, delta: 0n }, { event: 0n, key: 3n, delta: (-1n) }, { event: 2n, key: 1n, delta: 4n }, { event: 0n, key: 3n, delta: 7n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 4n || actual.duplicates !== 3n || actual.checksum !== 16n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ event: 2n, key: 3n, delta: 0n }, { event: 0n, key: 2n, delta: 1n }, { event: 2n, key: 1n, delta: 7n }, { event: 2n, key: 1n, delta: 4n }, { event: 2n, key: 1n, delta: 2n }, { event: 2n, key: 2n, delta: 5n }, { event: 1n, key: 0n, delta: 3n }, { event: 0n, key: 3n, delta: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 5n || actual.checksum !== 6n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ event: 3n, key: 2n, delta: 1n }, { event: 0n, key: 0n, delta: 5n }, { event: 0n, key: 0n, delta: 0n }, { event: 4n, key: 0n, delta: (-1n) }, { event: 1n, key: 2n, delta: 4n }, { event: 4n, key: 2n, delta: 9n }, { event: 0n, key: 2n, delta: 2n }, { event: 4n, key: 2n, delta: 3n }, { event: 1n, key: 1n, delta: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.applied !== 4n || actual.duplicates !== 5n || actual.checksum !== 19n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ event: 2n, key: 1n, delta: 3n }, { event: 4n, key: 2n, delta: 6n }, { event: 3n, key: 3n, delta: (-2n) }, { event: 1n, key: 3n, delta: 5n }, { event: 1n, key: 0n, delta: 2n }, { event: 0n, key: 3n, delta: 1n }, { event: 1n, key: 1n, delta: 7n }, { event: 4n, key: 1n, delta: 1n }, { event: 3n, key: 1n, delta: 0n }, { event: 1n, key: 3n, delta: 5n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 5n || actual.duplicates !== 5n || actual.checksum !== 40n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ event: 2n, key: 2n, delta: 2n }, { event: 2n, key: 1n, delta: 1n }, { event: 4n, key: 0n, delta: 3n }, { event: 1n, key: 1n, delta: 2n }, { event: 1n, key: 1n, delta: 7n }, { event: 0n, key: 1n, delta: (-1n) }, { event: 3n, key: 3n, delta: (-2n) }, { event: 3n, key: 3n, delta: 0n }, { event: 4n, key: 0n, delta: 5n }, { event: 0n, key: 3n, delta: 4n }, { event: 2n, key: 0n, delta: 0n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 5n || actual.duplicates !== 6n || actual.checksum !== 3n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ event: 3n, key: 0n, delta: 4n }, { event: 3n, key: 1n, delta: 3n }, { event: 0n, key: 1n, delta: 5n }, { event: 3n, key: 2n, delta: (-2n) }, { event: 3n, key: 1n, delta: 1n }, { event: 2n, key: 2n, delta: 6n }, { event: 3n, key: 0n, delta: 2n }, { event: 2n, key: 1n, delta: (-2n) }, { event: 0n, key: 3n, delta: 4n }, { event: 2n, key: 2n, delta: 8n }, { event: 3n, key: 0n, delta: 6n }, { event: 0n, key: 0n, delta: 3n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 9n || actual.checksum !== 32n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ event: 3n, key: 0n, delta: 1n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 1n || actual.duplicates !== 0n || actual.checksum !== 1n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ event: 3n, key: 2n, delta: 2n }, { event: 0n, key: 1n, delta: 7n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 2n || actual.duplicates !== 0n || actual.checksum !== 20n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ event: 4n, key: 0n, delta: 1n }, { event: 4n, key: 1n, delta: 1n }, { event: 1n, key: 2n, delta: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.applied !== 2n || actual.duplicates !== 1n || actual.checksum !== -5n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ event: 4n, key: 1n, delta: 5n }, { event: 4n, key: 0n, delta: 6n }, { event: 1n, key: 0n, delta: 8n }, { event: 0n, key: 0n, delta: 5n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 1n || actual.checksum !== 23n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ event: 2n, key: 0n, delta: 0n }, { event: 1n, key: 0n, delta: 3n }, { event: 1n, key: 1n, delta: (-1n) }, { event: 2n, key: 1n, delta: 0n }, { event: 1n, key: 2n, delta: 2n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 2n || actual.duplicates !== 3n || actual.checksum !== 3n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ event: 4n, key: 3n, delta: (-2n) }, { event: 1n, key: 3n, delta: 6n }, { event: 3n, key: 0n, delta: 2n }, { event: 4n, key: 1n, delta: (-2n) }, { event: 4n, key: 3n, delta: (-2n) }, { event: 3n, key: 0n, delta: 0n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 3n || actual.checksum !== 18n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ event: 1n, key: 0n, delta: 5n }, { event: 3n, key: 2n, delta: (-1n) }, { event: 3n, key: 3n, delta: 5n }, { event: 0n, key: 3n, delta: (-2n) }, { event: 4n, key: 1n, delta: (-1n) }, { event: 1n, key: 0n, delta: 3n }, { event: 2n, key: 3n, delta: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.applied !== 5n || actual.duplicates !== 2n || actual.checksum !== -12n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ event: 2n, key: 3n, delta: (-1n) }, { event: 2n, key: 0n, delta: (-1n) }, { event: 2n, key: 2n, delta: 5n }, { event: 1n, key: 3n, delta: 3n }, { event: 4n, key: 2n, delta: 6n }, { event: 3n, key: 0n, delta: 1n }, { event: 3n, key: 0n, delta: 1n }, { event: 3n, key: 0n, delta: 9n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 4n || actual.duplicates !== 4n || actual.checksum !== 27n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ event: 0n, key: 0n, delta: 0n }, { event: 0n, key: 1n, delta: 99n }, { event: 1n, key: 0n, delta: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.applied !== 2n || actual.duplicates !== 1n || actual.checksum !== -1n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ event: 1n, key: 0n, delta: 5n }, { event: 2n, key: 1n, delta: (-2n) }, { event: 1n, key: 9n, delta: 9n }, { event: 3n, key: 0n, delta: 1n }];
    const actual: Report = solve(rows);
    if (actual.applied !== 3n || actual.duplicates !== 1n || actual.checksum !== 2n) throw new Error("fixture 22");
}
