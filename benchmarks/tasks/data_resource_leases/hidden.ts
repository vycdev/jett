import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.accepted !== 0n || actual.rejected !== 0n || actual.expires_sum !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ resource_id: 3n, timestamp: 8n, duration: 3n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 1n || actual.rejected !== 0n || actual.expires_sum !== 11n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ resource_id: 1n, timestamp: 6n, duration: 3n }, { resource_id: 3n, timestamp: 7n, duration: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 0n || actual.expires_sum !== 21n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ resource_id: 2n, timestamp: 11n, duration: 1n }, { resource_id: 0n, timestamp: 0n, duration: 6n }, { resource_id: 0n, timestamp: (-2n), duration: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 1n || actual.expires_sum !== 18n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ resource_id: 3n, timestamp: 10n, duration: 2n }, { resource_id: 2n, timestamp: 2n, duration: 3n }, { resource_id: 4n, timestamp: 0n, duration: 4n }, { resource_id: 0n, timestamp: 11n, duration: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 0n || actual.expires_sum !== 34n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ resource_id: 3n, timestamp: 10n, duration: 3n }, { resource_id: 0n, timestamp: 5n, duration: 5n }, { resource_id: 4n, timestamp: 2n, duration: 6n }, { resource_id: 1n, timestamp: 3n, duration: 1n }, { resource_id: 1n, timestamp: 1n, duration: 1n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 1n || actual.expires_sum !== 35n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ resource_id: 3n, timestamp: 5n, duration: 6n }, { resource_id: 1n, timestamp: (-2n), duration: 5n }, { resource_id: 3n, timestamp: 11n, duration: 6n }, { resource_id: 4n, timestamp: 2n, duration: 4n }, { resource_id: 0n, timestamp: 10n, duration: 6n }, { resource_id: 3n, timestamp: 6n, duration: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 2n || actual.expires_sum !== 39n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ resource_id: 1n, timestamp: 5n, duration: 1n }, { resource_id: 1n, timestamp: 1n, duration: 1n }, { resource_id: 3n, timestamp: (-1n), duration: 1n }, { resource_id: 0n, timestamp: 8n, duration: 4n }, { resource_id: 3n, timestamp: 7n, duration: 4n }, { resource_id: 0n, timestamp: 5n, duration: 4n }, { resource_id: 1n, timestamp: 11n, duration: 3n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 3n || actual.expires_sum !== 37n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ resource_id: 0n, timestamp: (-3n), duration: 3n }, { resource_id: 1n, timestamp: 7n, duration: 4n }, { resource_id: 1n, timestamp: 7n, duration: 1n }, { resource_id: 3n, timestamp: (-2n), duration: 3n }, { resource_id: 4n, timestamp: (-1n), duration: 4n }, { resource_id: 0n, timestamp: 4n, duration: 5n }, { resource_id: 2n, timestamp: 8n, duration: 4n }, { resource_id: 1n, timestamp: (-2n), duration: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 3n || actual.rejected !== 5n || actual.expires_sum !== 32n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ resource_id: 2n, timestamp: 10n, duration: 2n }, { resource_id: 2n, timestamp: 11n, duration: 2n }, { resource_id: 4n, timestamp: 2n, duration: 2n }, { resource_id: 3n, timestamp: 1n, duration: 2n }, { resource_id: 2n, timestamp: 1n, duration: 5n }, { resource_id: 2n, timestamp: 4n, duration: 2n }, { resource_id: 0n, timestamp: 2n, duration: 1n }, { resource_id: 3n, timestamp: (-3n), duration: 4n }, { resource_id: 4n, timestamp: 2n, duration: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 5n || actual.expires_sum !== 22n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ resource_id: 0n, timestamp: 7n, duration: 1n }, { resource_id: 3n, timestamp: (-3n), duration: 1n }, { resource_id: 1n, timestamp: 5n, duration: 6n }, { resource_id: 0n, timestamp: 10n, duration: 1n }, { resource_id: 1n, timestamp: 8n, duration: 3n }, { resource_id: 3n, timestamp: 6n, duration: 3n }, { resource_id: 0n, timestamp: 2n, duration: 3n }, { resource_id: 4n, timestamp: 5n, duration: 3n }, { resource_id: 2n, timestamp: (-1n), duration: 2n }, { resource_id: 0n, timestamp: 1n, duration: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 5n || actual.rejected !== 5n || actual.expires_sum !== 39n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ resource_id: 2n, timestamp: 5n, duration: 3n }, { resource_id: 4n, timestamp: 3n, duration: 6n }, { resource_id: 3n, timestamp: 11n, duration: 1n }, { resource_id: 1n, timestamp: 8n, duration: 6n }, { resource_id: 3n, timestamp: 4n, duration: 2n }, { resource_id: 0n, timestamp: 1n, duration: 1n }, { resource_id: 4n, timestamp: 3n, duration: 2n }, { resource_id: 1n, timestamp: 5n, duration: 6n }, { resource_id: 4n, timestamp: 6n, duration: 5n }, { resource_id: 1n, timestamp: 9n, duration: 5n }, { resource_id: 4n, timestamp: 7n, duration: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 5n || actual.rejected !== 6n || actual.expires_sum !== 45n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ resource_id: 1n, timestamp: 3n, duration: 2n }, { resource_id: 1n, timestamp: 7n, duration: 2n }, { resource_id: 3n, timestamp: 4n, duration: 6n }, { resource_id: 2n, timestamp: 6n, duration: 3n }, { resource_id: 2n, timestamp: 11n, duration: 3n }, { resource_id: 1n, timestamp: 0n, duration: 5n }, { resource_id: 0n, timestamp: 2n, duration: 2n }, { resource_id: 1n, timestamp: 10n, duration: 3n }, { resource_id: 1n, timestamp: (-1n), duration: 5n }, { resource_id: 0n, timestamp: (-1n), duration: 1n }, { resource_id: 3n, timestamp: 11n, duration: 6n }, { resource_id: 3n, timestamp: (-3n), duration: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 8n || actual.rejected !== 4n || actual.expires_sum !== 48n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ resource_id: 3n, timestamp: (-1n), duration: 5n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 0n || actual.rejected !== 1n || actual.expires_sum !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ resource_id: 0n, timestamp: 4n, duration: 1n }, { resource_id: 4n, timestamp: 8n, duration: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 0n || actual.expires_sum !== 17n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ resource_id: 3n, timestamp: 7n, duration: 3n }, { resource_id: 0n, timestamp: (-1n), duration: 4n }, { resource_id: 0n, timestamp: 3n, duration: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 1n || actual.expires_sum !== 17n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ resource_id: 1n, timestamp: 11n, duration: 3n }, { resource_id: 0n, timestamp: 5n, duration: 2n }, { resource_id: 3n, timestamp: 4n, duration: 3n }, { resource_id: 0n, timestamp: 4n, duration: 2n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 3n || actual.rejected !== 1n || actual.expires_sum !== 28n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ resource_id: 1n, timestamp: 1n, duration: 5n }, { resource_id: 2n, timestamp: 5n, duration: 4n }, { resource_id: 4n, timestamp: (-3n), duration: 3n }, { resource_id: 2n, timestamp: 5n, duration: 5n }, { resource_id: 1n, timestamp: (-3n), duration: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 3n || actual.expires_sum !== 15n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ resource_id: 0n, timestamp: 4n, duration: 4n }, { resource_id: 2n, timestamp: 1n, duration: 6n }, { resource_id: 3n, timestamp: 10n, duration: 1n }, { resource_id: 4n, timestamp: (-3n), duration: 1n }, { resource_id: 2n, timestamp: 4n, duration: 1n }, { resource_id: 1n, timestamp: 4n, duration: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 2n || actual.expires_sum !== 36n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ resource_id: 2n, timestamp: 1n, duration: 6n }, { resource_id: 0n, timestamp: 6n, duration: 5n }, { resource_id: 1n, timestamp: 6n, duration: 5n }, { resource_id: 0n, timestamp: 0n, duration: 5n }, { resource_id: 1n, timestamp: 0n, duration: 2n }, { resource_id: 2n, timestamp: 11n, duration: 1n }, { resource_id: 4n, timestamp: (-1n), duration: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 4n || actual.rejected !== 3n || actual.expires_sum !== 34n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ resource_id: 4n, timestamp: 6n, duration: 1n }, { resource_id: 4n, timestamp: (-1n), duration: 1n }, { resource_id: 0n, timestamp: (-3n), duration: 4n }, { resource_id: 2n, timestamp: 9n, duration: 5n }, { resource_id: 0n, timestamp: (-1n), duration: 2n }, { resource_id: 0n, timestamp: 2n, duration: 2n }, { resource_id: 1n, timestamp: (-2n), duration: 3n }, { resource_id: 1n, timestamp: (-1n), duration: 6n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 3n || actual.rejected !== 5n || actual.expires_sum !== 25n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ resource_id: 0n, timestamp: 0n, duration: 0n }, { resource_id: 0n, timestamp: 0n, duration: 2n }, { resource_id: 0n, timestamp: 2n, duration: 3n }, { resource_id: 0n, timestamp: 1n, duration: 9n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 2n || actual.expires_sum !== 5n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ resource_id: 1n, timestamp: 0n, duration: 3n }, { resource_id: 1n, timestamp: 2n, duration: 7n }, { resource_id: 1n, timestamp: 3n, duration: 2n }, { resource_id: 2n, timestamp: (-1n), duration: 4n }];
    const actual: Report = solve(rows);
    if (actual.accepted !== 2n || actual.rejected !== 2n || actual.expires_sum !== 5n) throw new Error("fixture 22");
}
