import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ key: 3n, side: 8n, quantity: 3n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ key: 1n, side: 6n, quantity: 3n }, { key: 3n, side: 7n, quantity: 5n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ key: 2n, side: 11n, quantity: 1n }, { key: 0n, side: 0n, quantity: 6n }, { key: 0n, side: (-2n), quantity: 5n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 6n || actual.right_only !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ key: 3n, side: 10n, quantity: 2n }, { key: 2n, side: 2n, quantity: 3n }, { key: 4n, side: 0n, quantity: 4n }, { key: 0n, side: 11n, quantity: 2n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 4n || actual.right_only !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ key: 3n, side: 10n, quantity: 3n }, { key: 0n, side: 5n, quantity: 5n }, { key: 4n, side: 2n, quantity: 6n }, { key: 1n, side: 3n, quantity: 1n }, { key: 1n, side: 1n, quantity: 1n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 1n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ key: 3n, side: 5n, quantity: 6n }, { key: 1n, side: (-2n), quantity: 5n }, { key: 3n, side: 11n, quantity: 6n }, { key: 4n, side: 2n, quantity: 4n }, { key: 0n, side: 10n, quantity: 6n }, { key: 3n, side: 6n, quantity: 5n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ key: 1n, side: 5n, quantity: 1n }, { key: 1n, side: 1n, quantity: 1n }, { key: 3n, side: (-1n), quantity: 1n }, { key: 0n, side: 8n, quantity: 4n }, { key: 3n, side: 7n, quantity: 4n }, { key: 0n, side: 5n, quantity: 4n }, { key: 1n, side: 11n, quantity: 3n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ key: 0n, side: (-3n), quantity: 3n }, { key: 1n, side: 7n, quantity: 4n }, { key: 1n, side: 7n, quantity: 1n }, { key: 3n, side: (-2n), quantity: 3n }, { key: 4n, side: (-1n), quantity: 4n }, { key: 0n, side: 4n, quantity: 5n }, { key: 2n, side: 8n, quantity: 4n }, { key: 1n, side: (-2n), quantity: 6n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ key: 2n, side: 10n, quantity: 2n }, { key: 2n, side: 11n, quantity: 2n }, { key: 4n, side: 2n, quantity: 2n }, { key: 3n, side: 1n, quantity: 2n }, { key: 2n, side: 1n, quantity: 5n }, { key: 2n, side: 4n, quantity: 2n }, { key: 0n, side: 2n, quantity: 1n }, { key: 3n, side: (-3n), quantity: 4n }, { key: 4n, side: 2n, quantity: 2n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 7n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ key: 0n, side: 7n, quantity: 1n }, { key: 3n, side: (-3n), quantity: 1n }, { key: 1n, side: 5n, quantity: 6n }, { key: 0n, side: 10n, quantity: 1n }, { key: 1n, side: 8n, quantity: 3n }, { key: 3n, side: 6n, quantity: 3n }, { key: 0n, side: 2n, quantity: 3n }, { key: 4n, side: 5n, quantity: 3n }, { key: 2n, side: (-1n), quantity: 2n }, { key: 0n, side: 1n, quantity: 2n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 2n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ key: 2n, side: 5n, quantity: 3n }, { key: 4n, side: 3n, quantity: 6n }, { key: 3n, side: 11n, quantity: 1n }, { key: 1n, side: 8n, quantity: 6n }, { key: 3n, side: 4n, quantity: 2n }, { key: 0n, side: 1n, quantity: 1n }, { key: 4n, side: 3n, quantity: 2n }, { key: 1n, side: 5n, quantity: 6n }, { key: 4n, side: 6n, quantity: 5n }, { key: 1n, side: 9n, quantity: 5n }, { key: 4n, side: 7n, quantity: 2n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 1n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ key: 1n, side: 3n, quantity: 2n }, { key: 1n, side: 7n, quantity: 2n }, { key: 3n, side: 4n, quantity: 6n }, { key: 2n, side: 6n, quantity: 3n }, { key: 2n, side: 11n, quantity: 3n }, { key: 1n, side: 0n, quantity: 5n }, { key: 0n, side: 2n, quantity: 2n }, { key: 1n, side: 10n, quantity: 3n }, { key: 1n, side: (-1n), quantity: 5n }, { key: 0n, side: (-1n), quantity: 1n }, { key: 3n, side: 11n, quantity: 6n }, { key: 3n, side: (-3n), quantity: 4n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 5n || actual.right_only !== 0n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ key: 3n, side: (-1n), quantity: 5n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ key: 0n, side: 4n, quantity: 1n }, { key: 4n, side: 8n, quantity: 4n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ key: 3n, side: 7n, quantity: 3n }, { key: 0n, side: (-1n), quantity: 4n }, { key: 0n, side: 3n, quantity: 4n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ key: 1n, side: 11n, quantity: 3n }, { key: 0n, side: 5n, quantity: 2n }, { key: 3n, side: 4n, quantity: 3n }, { key: 0n, side: 4n, quantity: 2n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ key: 1n, side: 1n, quantity: 5n }, { key: 2n, side: 5n, quantity: 4n }, { key: 4n, side: (-3n), quantity: 3n }, { key: 2n, side: 5n, quantity: 5n }, { key: 1n, side: (-3n), quantity: 6n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 5n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ key: 0n, side: 4n, quantity: 4n }, { key: 2n, side: 1n, quantity: 6n }, { key: 3n, side: 10n, quantity: 1n }, { key: 4n, side: (-3n), quantity: 1n }, { key: 2n, side: 4n, quantity: 1n }, { key: 1n, side: 4n, quantity: 6n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 6n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ key: 2n, side: 1n, quantity: 6n }, { key: 0n, side: 6n, quantity: 5n }, { key: 1n, side: 6n, quantity: 5n }, { key: 0n, side: 0n, quantity: 5n }, { key: 1n, side: 0n, quantity: 2n }, { key: 2n, side: 11n, quantity: 1n }, { key: 4n, side: (-1n), quantity: 4n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 7n || actual.right_only !== 6n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ key: 4n, side: 6n, quantity: 1n }, { key: 4n, side: (-1n), quantity: 1n }, { key: 0n, side: (-3n), quantity: 4n }, { key: 2n, side: 9n, quantity: 5n }, { key: 0n, side: (-1n), quantity: 2n }, { key: 0n, side: 2n, quantity: 2n }, { key: 1n, side: (-2n), quantity: 3n }, { key: 1n, side: (-1n), quantity: 6n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 0n || actual.left_only !== 0n || actual.right_only !== 0n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ key: 0n, side: 0n, quantity: 2n }, { key: 0n, side: 0n, quantity: 3n }, { key: 0n, side: 1n, quantity: 4n }, { key: 1n, side: 2n, quantity: 9n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 4n || actual.left_only !== 1n || actual.right_only !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ key: 1n, side: 0n, quantity: 4n }, { key: 1n, side: 1n, quantity: 2n }, { key: 2n, side: 1n, quantity: 5n }, { key: 1n, side: 0n, quantity: 1n }];
    const actual: Report = solve(rows);
    if (actual.matched !== 2n || actual.left_only !== 3n || actual.right_only !== 5n) throw new Error("fixture 22");
}
