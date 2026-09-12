import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.ready !== 0n || actual.blocked !== 0n || actual.total_cost !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 3n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 1n || actual.blocked !== 0n || actual.total_cost !== 3n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 10n }, { job: 2n, prerequisite: 1n, cost: 5n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 2n || actual.blocked !== 0n || actual.total_cost !== 15n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ job: 3n, prerequisite: 0n, cost: 8n }, { job: 1n, prerequisite: 0n, cost: (-8n) }, { job: 2n, prerequisite: 0n, cost: (-6n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 3n || actual.blocked !== 0n || actual.total_cost !== -6n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ job: 3n, prerequisite: 0n, cost: 6n }, { job: 1n, prerequisite: 0n, cost: 3n }, { job: 2n, prerequisite: 1n, cost: 10n }, { job: 4n, prerequisite: 0n, cost: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 4n || actual.blocked !== 0n || actual.total_cost !== 17n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ job: 5n, prerequisite: 4n, cost: (-1n) }, { job: 2n, prerequisite: 1n, cost: (-7n) }, { job: 3n, prerequisite: 0n, cost: 0n }, { job: 4n, prerequisite: 0n, cost: 5n }, { job: 1n, prerequisite: 0n, cost: (-4n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 5n || actual.blocked !== 0n || actual.total_cost !== -7n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ job: 5n, prerequisite: 2n, cost: (-5n) }, { job: 3n, prerequisite: 2n, cost: (-2n) }, { job: 2n, prerequisite: 1n, cost: 11n }, { job: 4n, prerequisite: 0n, cost: (-1n) }, { job: 6n, prerequisite: 3n, cost: (-4n) }, { job: 1n, prerequisite: 0n, cost: (-6n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 6n || actual.blocked !== 0n || actual.total_cost !== -7n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ job: 6n, prerequisite: 1n, cost: (-7n) }, { job: 7n, prerequisite: 6n, cost: 7n }, { job: 4n, prerequisite: 0n, cost: 1n }, { job: 2n, prerequisite: 1n, cost: (-3n) }, { job: 5n, prerequisite: 1n, cost: 5n }, { job: 3n, prerequisite: 1n, cost: (-8n) }, { job: 1n, prerequisite: 0n, cost: 9n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 7n || actual.blocked !== 0n || actual.total_cost !== 4n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 11n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 1n || actual.blocked !== 0n || actual.total_cost !== 11n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 4n }, { job: 2n, prerequisite: 0n, cost: (-5n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 2n || actual.blocked !== 0n || actual.total_cost !== -1n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 0n }, { job: 3n, prerequisite: 1n, cost: (-3n) }, { job: 2n, prerequisite: 0n, cost: 10n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 3n || actual.blocked !== 0n || actual.total_cost !== 7n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ job: 4n, prerequisite: 1n, cost: (-7n) }, { job: 2n, prerequisite: 1n, cost: 10n }, { job: 1n, prerequisite: 0n, cost: 0n }, { job: 3n, prerequisite: 1n, cost: 7n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 4n || actual.blocked !== 0n || actual.total_cost !== 10n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ job: 2n, prerequisite: 1n, cost: (-2n) }, { job: 3n, prerequisite: 0n, cost: (-5n) }, { job: 4n, prerequisite: 3n, cost: (-8n) }, { job: 1n, prerequisite: 0n, cost: 5n }, { job: 5n, prerequisite: 0n, cost: (-4n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 5n || actual.blocked !== 0n || actual.total_cost !== -14n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 4n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 1n || actual.blocked !== 0n || actual.total_cost !== 4n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: (-5n) }, { job: 2n, prerequisite: 1n, cost: 0n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 2n || actual.blocked !== 0n || actual.total_cost !== -5n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: (-3n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 1n || actual.blocked !== 0n || actual.total_cost !== -3n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: (-5n) }, { job: 2n, prerequisite: 1n, cost: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 2n || actual.blocked !== 0n || actual.total_cost !== -7n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ job: 1n, prerequisite: 0n, cost: 8n }, { job: 2n, prerequisite: 1n, cost: 4n }, { job: 3n, prerequisite: 0n, cost: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 3n || actual.blocked !== 0n || actual.total_cost !== 10n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ job: 4n, prerequisite: 3n, cost: (-1n) }, { job: 1n, prerequisite: 0n, cost: (-2n) }, { job: 3n, prerequisite: 0n, cost: 8n }, { job: 2n, prerequisite: 0n, cost: 0n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 4n || actual.blocked !== 0n || actual.total_cost !== 5n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ job: 4n, prerequisite: 3n, cost: 6n }, { job: 1n, prerequisite: 0n, cost: (-2n) }, { job: 2n, prerequisite: 1n, cost: (-2n) }, { job: 5n, prerequisite: 2n, cost: 10n }, { job: 3n, prerequisite: 0n, cost: (-3n) }];
    const actual: Report = solve(rows);
    if (actual.ready !== 5n || actual.blocked !== 0n || actual.total_cost !== 9n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ job: 3n, prerequisite: 0n, cost: (-1n) }, { job: 2n, prerequisite: 0n, cost: 3n }, { job: 6n, prerequisite: 0n, cost: (-4n) }, { job: 5n, prerequisite: 1n, cost: 11n }, { job: 4n, prerequisite: 2n, cost: (-3n) }, { job: 1n, prerequisite: 0n, cost: 9n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 6n || actual.blocked !== 0n || actual.total_cost !== 15n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ job: 9n, prerequisite: 8n, cost: 5n }, { job: 3n, prerequisite: 2n, cost: 1n }, { job: 2n, prerequisite: 1n, cost: 8n }, { job: 1n, prerequisite: 0n, cost: (-2n) }, { job: 10n, prerequisite: 9n, cost: 3n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 3n || actual.blocked !== 2n || actual.total_cost !== 7n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ job: 3n, prerequisite: 2n, cost: 5n }, { job: 1n, prerequisite: 0n, cost: 2n }, { job: 5n, prerequisite: 3n, cost: 9n }];
    const actual: Report = solve(rows);
    if (actual.ready !== 1n || actual.blocked !== 2n || actual.total_cost !== 2n) throw new Error("fixture 22");
}
