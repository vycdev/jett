import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.total !== 0n || actual.lowest_balance !== 0n || actual.first_overdraw !== -1n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 0n || actual.lowest_balance !== 5n || actual.first_overdraw !== -1n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.total !== 0n || actual.lowest_balance !== 1n || actual.first_overdraw !== -1n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ account: 3n, delta: 8n }];
    const actual: Report = solve(rows, 2n);
    if (actual.total !== 10n || actual.lowest_balance !== 10n || actual.first_overdraw !== -1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ account: 1n, delta: 6n }, { account: 3n, delta: 7n }];
    const actual: Report = solve(rows, 3n);
    if (actual.total !== 19n || actual.lowest_balance !== 9n || actual.first_overdraw !== -1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ account: 2n, delta: 11n }, { account: 0n, delta: 0n }, { account: 0n, delta: (-2n) }];
    const actual: Report = solve(rows, 4n);
    if (actual.total !== 17n || actual.lowest_balance !== 2n || actual.first_overdraw !== -1n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ account: 3n, delta: 10n }, { account: 2n, delta: 2n }, { account: 4n, delta: 0n }, { account: 0n, delta: 11n }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 43n || actual.lowest_balance !== 5n || actual.first_overdraw !== -1n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ account: 3n, delta: 10n }, { account: 0n, delta: 5n }, { account: 4n, delta: 2n }, { account: 1n, delta: 3n }, { account: 1n, delta: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.total !== 45n || actual.lowest_balance !== 8n || actual.first_overdraw !== -1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ account: 3n, delta: 5n }, { account: 1n, delta: (-2n) }, { account: 3n, delta: 11n }, { account: 4n, delta: 2n }, { account: 0n, delta: 10n }, { account: 3n, delta: 6n }];
    const actual: Report = solve(rows, 7n);
    if (actual.total !== 60n || actual.lowest_balance !== 5n || actual.first_overdraw !== -1n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ account: 1n, delta: 5n }, { account: 1n, delta: 1n }, { account: 3n, delta: (-1n) }, { account: 0n, delta: 8n }, { account: 3n, delta: 7n }, { account: 0n, delta: 5n }, { account: 1n, delta: 11n }];
    const actual: Report = solve(rows, 8n);
    if (actual.total !== 60n || actual.lowest_balance !== 7n || actual.first_overdraw !== -1n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: (-3n) }, { account: 1n, delta: 7n }, { account: 1n, delta: 7n }, { account: 3n, delta: (-2n) }, { account: 4n, delta: (-1n) }, { account: 0n, delta: 4n }, { account: 2n, delta: 8n }, { account: 1n, delta: (-2n) }];
    const actual: Report = solve(rows, 1n);
    if (actual.total !== 23n || actual.lowest_balance !== -2n || actual.first_overdraw !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ account: 2n, delta: 10n }, { account: 2n, delta: 11n }, { account: 4n, delta: 2n }, { account: 3n, delta: 1n }, { account: 2n, delta: 1n }, { account: 2n, delta: 4n }, { account: 0n, delta: 2n }, { account: 3n, delta: (-3n) }, { account: 4n, delta: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.total !== 38n || actual.lowest_balance !== 0n || actual.first_overdraw !== -1n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: 7n }, { account: 3n, delta: (-3n) }, { account: 1n, delta: 5n }, { account: 0n, delta: 10n }, { account: 1n, delta: 8n }, { account: 3n, delta: 6n }, { account: 0n, delta: 2n }, { account: 4n, delta: 5n }, { account: 2n, delta: (-1n) }, { account: 0n, delta: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.total !== 55n || actual.lowest_balance !== 0n || actual.first_overdraw !== -1n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ account: 2n, delta: 5n }, { account: 4n, delta: 3n }, { account: 3n, delta: 11n }, { account: 1n, delta: 8n }, { account: 3n, delta: 4n }, { account: 0n, delta: 1n }, { account: 4n, delta: 3n }, { account: 1n, delta: 5n }, { account: 4n, delta: 6n }, { account: 1n, delta: 9n }, { account: 4n, delta: 7n }];
    const actual: Report = solve(rows, 4n);
    if (actual.total !== 82n || actual.lowest_balance !== 5n || actual.first_overdraw !== -1n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ account: 1n, delta: 3n }, { account: 1n, delta: 7n }, { account: 3n, delta: 4n }, { account: 2n, delta: 6n }, { account: 2n, delta: 11n }, { account: 1n, delta: 0n }, { account: 0n, delta: 2n }, { account: 1n, delta: 10n }, { account: 1n, delta: (-1n) }, { account: 0n, delta: (-1n) }, { account: 3n, delta: 11n }, { account: 3n, delta: (-3n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 69n || actual.lowest_balance !== 6n || actual.first_overdraw !== -1n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.total !== 0n || actual.lowest_balance !== 6n || actual.first_overdraw !== -1n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ account: 3n, delta: (-1n) }];
    const actual: Report = solve(rows, 7n);
    if (actual.total !== 6n || actual.lowest_balance !== 6n || actual.first_overdraw !== -1n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: 4n }, { account: 4n, delta: 8n }];
    const actual: Report = solve(rows, 8n);
    if (actual.total !== 28n || actual.lowest_balance !== 12n || actual.first_overdraw !== -1n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ account: 3n, delta: 7n }, { account: 0n, delta: (-1n) }, { account: 0n, delta: 3n }];
    const actual: Report = solve(rows, 1n);
    if (actual.total !== 11n || actual.lowest_balance !== 0n || actual.first_overdraw !== -1n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ account: 1n, delta: 11n }, { account: 0n, delta: 5n }, { account: 3n, delta: 4n }, { account: 0n, delta: 4n }];
    const actual: Report = solve(rows, 2n);
    if (actual.total !== 30n || actual.lowest_balance !== 6n || actual.first_overdraw !== -1n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ account: 1n, delta: 1n }, { account: 2n, delta: 5n }, { account: 4n, delta: (-3n) }, { account: 2n, delta: 5n }, { account: 1n, delta: (-3n) }];
    const actual: Report = solve(rows, 3n);
    if (actual.total !== 14n || actual.lowest_balance !== 0n || actual.first_overdraw !== -1n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: 4n }, { account: 2n, delta: 1n }, { account: 3n, delta: 10n }, { account: 4n, delta: (-3n) }, { account: 2n, delta: 4n }, { account: 1n, delta: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.total !== 40n || actual.lowest_balance !== 1n || actual.first_overdraw !== -1n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ account: 2n, delta: 1n }, { account: 0n, delta: 6n }, { account: 1n, delta: 6n }, { account: 0n, delta: 0n }, { account: 1n, delta: 0n }, { account: 2n, delta: 11n }, { account: 4n, delta: (-1n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 43n || actual.lowest_balance !== 4n || actual.first_overdraw !== -1n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ account: 4n, delta: 6n }, { account: 4n, delta: (-1n) }, { account: 0n, delta: (-3n) }, { account: 2n, delta: 9n }, { account: 0n, delta: (-1n) }, { account: 0n, delta: 2n }, { account: 1n, delta: (-2n) }, { account: 1n, delta: (-1n) }];
    const actual: Report = solve(rows, 6n);
    if (actual.total !== 33n || actual.lowest_balance !== 2n || actual.first_overdraw !== -1n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: 3n }, { account: 1n, delta: 6n }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 19n || actual.lowest_balance !== 8n || actual.first_overdraw !== -1n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: (-5n) }, { account: 0n, delta: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 1n || actual.lowest_balance !== 0n || actual.first_overdraw !== -1n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ account: 0n, delta: (-6n) }, { account: 1n, delta: (-8n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== -4n || actual.lowest_balance !== -3n || actual.first_overdraw !== 0n) throw new Error("fixture 26");
}
{
    const rows: readonly Entry[] = [{ account: 2n, delta: (-7n) }, { account: 2n, delta: 9n }, { account: 3n, delta: (-1n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.total !== 11n || actual.lowest_balance !== -2n || actual.first_overdraw !== 0n) throw new Error("fixture 27");
}
