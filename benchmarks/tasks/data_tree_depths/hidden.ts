import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.roots !== 0n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 3n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 10n }, { node: 2n, parent: 1n, weight: 5n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 1n || actual.weighted_depth !== 5n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ node: 3n, parent: 0n, weight: 8n }, { node: 1n, parent: 0n, weight: (-8n) }, { node: 2n, parent: 0n, weight: (-6n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 3n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ node: 3n, parent: 0n, weight: 6n }, { node: 1n, parent: 0n, weight: 3n }, { node: 2n, parent: 1n, weight: 10n }, { node: 4n, parent: 0n, weight: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 3n || actual.max_depth !== 1n || actual.weighted_depth !== 10n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ node: 5n, parent: 4n, weight: (-1n) }, { node: 2n, parent: 1n, weight: (-7n) }, { node: 3n, parent: 0n, weight: 0n }, { node: 4n, parent: 0n, weight: 5n }, { node: 1n, parent: 0n, weight: (-4n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 3n || actual.max_depth !== 1n || actual.weighted_depth !== -8n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ node: 5n, parent: 2n, weight: (-5n) }, { node: 3n, parent: 2n, weight: (-2n) }, { node: 2n, parent: 1n, weight: 11n }, { node: 4n, parent: 0n, weight: (-1n) }, { node: 6n, parent: 3n, weight: (-4n) }, { node: 1n, parent: 0n, weight: (-6n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 3n || actual.weighted_depth !== -15n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ node: 6n, parent: 1n, weight: (-7n) }, { node: 7n, parent: 6n, weight: 7n }, { node: 4n, parent: 0n, weight: 1n }, { node: 2n, parent: 1n, weight: (-3n) }, { node: 5n, parent: 1n, weight: 5n }, { node: 3n, parent: 1n, weight: (-8n) }, { node: 1n, parent: 0n, weight: 9n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 2n || actual.weighted_depth !== 1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 11n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 4n }, { node: 2n, parent: 0n, weight: (-5n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 0n }, { node: 3n, parent: 1n, weight: (-3n) }, { node: 2n, parent: 0n, weight: 10n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 1n || actual.weighted_depth !== -3n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 1n, weight: (-7n) }, { node: 2n, parent: 1n, weight: 10n }, { node: 1n, parent: 0n, weight: 0n }, { node: 3n, parent: 1n, weight: 7n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 1n || actual.weighted_depth !== 10n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ node: 2n, parent: 1n, weight: (-2n) }, { node: 3n, parent: 0n, weight: (-5n) }, { node: 4n, parent: 3n, weight: (-8n) }, { node: 1n, parent: 0n, weight: 5n }, { node: 5n, parent: 0n, weight: (-4n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 3n || actual.max_depth !== 1n || actual.weighted_depth !== -10n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 4n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: (-5n) }, { node: 2n, parent: 1n, weight: 0n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 1n || actual.weighted_depth !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: (-3n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 0n || actual.weighted_depth !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: (-5n) }, { node: 2n, parent: 1n, weight: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 1n || actual.weighted_depth !== -2n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n, weight: 8n }, { node: 2n, parent: 1n, weight: 4n }, { node: 3n, parent: 0n, weight: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 1n || actual.weighted_depth !== 4n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 3n, weight: (-1n) }, { node: 1n, parent: 0n, weight: (-2n) }, { node: 3n, parent: 0n, weight: 8n }, { node: 2n, parent: 0n, weight: 0n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 3n || actual.max_depth !== 1n || actual.weighted_depth !== -1n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 3n, weight: 6n }, { node: 1n, parent: 0n, weight: (-2n) }, { node: 2n, parent: 1n, weight: (-2n) }, { node: 5n, parent: 2n, weight: 10n }, { node: 3n, parent: 0n, weight: (-3n) }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 2n || actual.weighted_depth !== 24n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ node: 3n, parent: 0n, weight: (-1n) }, { node: 2n, parent: 0n, weight: 3n }, { node: 6n, parent: 0n, weight: (-4n) }, { node: 5n, parent: 1n, weight: 11n }, { node: 4n, parent: 2n, weight: (-3n) }, { node: 1n, parent: 0n, weight: 9n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 4n || actual.max_depth !== 1n || actual.weighted_depth !== 8n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ node: 5n, parent: 4n, weight: (-1n) }, { node: 4n, parent: 3n, weight: 2n }, { node: 3n, parent: 2n, weight: 4n }, { node: 2n, parent: 1n, weight: 8n }, { node: 1n, parent: 0n, weight: 3n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 4n || actual.weighted_depth !== 18n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 2n, weight: 3n }, { node: 2n, parent: 1n, weight: 5n }, { node: 1n, parent: 0n, weight: 9n }, { node: 3n, parent: 0n, weight: 7n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 2n || actual.max_depth !== 2n || actual.weighted_depth !== 11n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ node: 30n, parent: 2n, weight: 7n }, { node: 12n, parent: 9n, weight: (-2n) }, { node: 2n, parent: 0n, weight: 3n }, { node: 9n, parent: 2n, weight: 5n }];
    const actual: Report = solve(rows);
    if (actual.roots !== 1n || actual.max_depth !== 2n || actual.weighted_depth !== 8n) throw new Error("fixture 23");
}
