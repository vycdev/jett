import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.selected !== 0n || actual.rank_sum !== 0n || actual.tie_groups !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 0n || actual.rank_sum !== 0n || actual.tie_groups !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 0n || actual.rank_sum !== 0n || actual.tie_groups !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ score: 8n, penalty: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 1n || actual.rank_sum !== 1n || actual.tie_groups !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ score: 6n, penalty: 3n }, { score: 7n, penalty: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 2n || actual.rank_sum !== 3n || actual.tie_groups !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ score: 11n, penalty: 1n }, { score: 0n, penalty: 6n }, { score: (-2n), penalty: 5n }];
    const actual: Report = solve(rows, 4n);
    if (actual.selected !== 3n || actual.rank_sum !== 6n || actual.tie_groups !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ score: 10n, penalty: 2n }, { score: 2n, penalty: 3n }, { score: 0n, penalty: 4n }, { score: 11n, penalty: 2n }];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 4n || actual.rank_sum !== 10n || actual.tie_groups !== 0n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ score: 10n, penalty: 3n }, { score: 5n, penalty: 5n }, { score: 2n, penalty: 6n }, { score: 3n, penalty: 1n }, { score: 1n, penalty: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.selected !== 5n || actual.rank_sum !== 15n || actual.tie_groups !== 0n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ score: 5n, penalty: 6n }, { score: (-2n), penalty: 5n }, { score: 11n, penalty: 6n }, { score: 2n, penalty: 4n }, { score: 10n, penalty: 6n }, { score: 6n, penalty: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.selected !== 6n || actual.rank_sum !== 21n || actual.tie_groups !== 0n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ score: 5n, penalty: 1n }, { score: 1n, penalty: 1n }, { score: (-1n), penalty: 1n }, { score: 8n, penalty: 4n }, { score: 7n, penalty: 4n }, { score: 5n, penalty: 4n }, { score: 11n, penalty: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.selected !== 7n || actual.rank_sum !== 28n || actual.tie_groups !== 0n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ score: (-3n), penalty: 3n }, { score: 7n, penalty: 4n }, { score: 7n, penalty: 1n }, { score: (-2n), penalty: 3n }, { score: (-1n), penalty: 4n }, { score: 4n, penalty: 5n }, { score: 8n, penalty: 4n }, { score: (-2n), penalty: 6n }];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 1n || actual.rank_sum !== 1n || actual.tie_groups !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ score: 10n, penalty: 2n }, { score: 11n, penalty: 2n }, { score: 2n, penalty: 2n }, { score: 1n, penalty: 2n }, { score: 1n, penalty: 5n }, { score: 4n, penalty: 2n }, { score: 2n, penalty: 1n }, { score: (-3n), penalty: 4n }, { score: 2n, penalty: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.rank_sum !== 3n || actual.tie_groups !== 1n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ score: 7n, penalty: 1n }, { score: (-3n), penalty: 1n }, { score: 5n, penalty: 6n }, { score: 10n, penalty: 1n }, { score: 8n, penalty: 3n }, { score: 6n, penalty: 3n }, { score: 2n, penalty: 3n }, { score: 5n, penalty: 3n }, { score: (-1n), penalty: 2n }, { score: 1n, penalty: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 3n || actual.rank_sum !== 6n || actual.tie_groups !== 0n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ score: 5n, penalty: 3n }, { score: 3n, penalty: 6n }, { score: 11n, penalty: 1n }, { score: 8n, penalty: 6n }, { score: 4n, penalty: 2n }, { score: 1n, penalty: 1n }, { score: 3n, penalty: 2n }, { score: 5n, penalty: 6n }, { score: 6n, penalty: 5n }, { score: 9n, penalty: 5n }, { score: 7n, penalty: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.selected !== 4n || actual.rank_sum !== 10n || actual.tie_groups !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ score: 3n, penalty: 2n }, { score: 7n, penalty: 2n }, { score: 4n, penalty: 6n }, { score: 6n, penalty: 3n }, { score: 11n, penalty: 3n }, { score: 0n, penalty: 5n }, { score: 2n, penalty: 2n }, { score: 10n, penalty: 3n }, { score: (-1n), penalty: 5n }, { score: (-1n), penalty: 1n }, { score: 11n, penalty: 6n }, { score: (-3n), penalty: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 5n || actual.rank_sum !== 15n || actual.tie_groups !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.selected !== 0n || actual.rank_sum !== 0n || actual.tie_groups !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ score: (-1n), penalty: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.selected !== 1n || actual.rank_sum !== 1n || actual.tie_groups !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ score: 4n, penalty: 1n }, { score: 8n, penalty: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.selected !== 2n || actual.rank_sum !== 3n || actual.tie_groups !== 0n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ score: 7n, penalty: 3n }, { score: (-1n), penalty: 4n }, { score: 3n, penalty: 4n }];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 1n || actual.rank_sum !== 1n || actual.tie_groups !== 0n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ score: 11n, penalty: 3n }, { score: 5n, penalty: 2n }, { score: 4n, penalty: 3n }, { score: 4n, penalty: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.rank_sum !== 3n || actual.tie_groups !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ score: 1n, penalty: 5n }, { score: 5n, penalty: 4n }, { score: (-3n), penalty: 3n }, { score: 5n, penalty: 5n }, { score: (-3n), penalty: 6n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 3n || actual.rank_sum !== 6n || actual.tie_groups !== 0n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ score: 4n, penalty: 4n }, { score: 1n, penalty: 6n }, { score: 10n, penalty: 1n }, { score: (-3n), penalty: 1n }, { score: 4n, penalty: 1n }, { score: 4n, penalty: 6n }];
    const actual: Report = solve(rows, 4n);
    if (actual.selected !== 4n || actual.rank_sum !== 10n || actual.tie_groups !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ score: 1n, penalty: 6n }, { score: 6n, penalty: 5n }, { score: 6n, penalty: 5n }, { score: 0n, penalty: 5n }, { score: 0n, penalty: 2n }, { score: 11n, penalty: 1n }, { score: (-1n), penalty: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 5n || actual.rank_sum !== 14n || actual.tie_groups !== 1n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ score: 6n, penalty: 1n }, { score: (-1n), penalty: 1n }, { score: (-3n), penalty: 4n }, { score: 9n, penalty: 5n }, { score: (-1n), penalty: 2n }, { score: 2n, penalty: 2n }, { score: (-2n), penalty: 3n }, { score: (-1n), penalty: 6n }];
    const actual: Report = solve(rows, 6n);
    if (actual.selected !== 6n || actual.rank_sum !== 21n || actual.tie_groups !== 0n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ score: 9n, penalty: 1n }, { score: 9n, penalty: 1n }, { score: 8n, penalty: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.rank_sum !== 2n || actual.tie_groups !== 1n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ score: 0n, penalty: 2n }, { score: 0n, penalty: 1n }];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 1n || actual.rank_sum !== 1n || actual.tie_groups !== 0n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ score: 9n, penalty: 1n }, { score: 9n, penalty: 1n }, { score: 9n, penalty: 2n }, { score: 8n, penalty: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 3n || actual.rank_sum !== 5n || actual.tie_groups !== 1n) throw new Error("fixture 26");
}
