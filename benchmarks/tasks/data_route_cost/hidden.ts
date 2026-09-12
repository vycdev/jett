import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.reachable !== 0n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.reachable !== 2n || actual.cost_sum !== 2n || actual.most_expensive !== 2n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 2n, cost: 6n }, { source: 1n, target: 2n, cost: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 3n, cost: 1n }, { source: 0n, target: 3n, cost: 4n }, { source: 0n, target: 2n, cost: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.reachable !== 3n || actual.cost_sum !== 4n || actual.most_expensive !== 3n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 3n, cost: 4n }, { source: 0n, target: 2n, cost: 4n }, { source: 2n, target: 3n, cost: 5n }, { source: 2n, target: 3n, cost: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 3n || actual.cost_sum !== 12n || actual.most_expensive !== 8n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 2n, cost: 3n }, { source: 0n, target: 4n, cost: 5n }, { source: 1n, target: 2n, cost: 7n }, { source: 4n, target: 5n, cost: 6n }, { source: 4n, target: 5n, cost: 4n }];
    const actual: Report = solve(rows, 6n);
    if (actual.reachable !== 4n || actual.cost_sum !== 17n || actual.most_expensive !== 9n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 6n, cost: 4n }, { source: 4n, target: 5n, cost: 5n }, { source: 0n, target: 2n, cost: 3n }, { source: 0n, target: 4n, cost: 2n }, { source: 0n, target: 1n, cost: 6n }, { source: 3n, target: 5n, cost: 6n }];
    const actual: Report = solve(rows, 7n);
    if (actual.reachable !== 6n || actual.cost_sum !== 22n || actual.most_expensive !== 7n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ source: 3n, target: 4n, cost: 5n }, { source: 3n, target: 5n, cost: 3n }, { source: 6n, target: 7n, cost: 1n }, { source: 2n, target: 4n, cost: 6n }, { source: 3n, target: 5n, cost: 6n }, { source: 6n, target: 7n, cost: 7n }, { source: 3n, target: 4n, cost: 7n }];
    const actual: Report = solve(rows, 8n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 7n }, { source: 0n, target: 1n, cost: 4n }, { source: 0n, target: 1n, cost: 7n }, { source: 0n, target: 1n, cost: 7n }, { source: 0n, target: 1n, cost: 6n }, { source: 0n, target: 1n, cost: 7n }, { source: 0n, target: 1n, cost: 4n }, { source: 0n, target: 1n, cost: 3n }, { source: 0n, target: 1n, cost: 4n }];
    const actual: Report = solve(rows, 2n);
    if (actual.reachable !== 2n || actual.cost_sum !== 3n || actual.most_expensive !== 3n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 3n }, { source: 0n, target: 2n, cost: 1n }, { source: 1n, target: 2n, cost: 2n }, { source: 0n, target: 1n, cost: 4n }, { source: 0n, target: 1n, cost: 2n }, { source: 0n, target: 1n, cost: 6n }, { source: 0n, target: 2n, cost: 4n }, { source: 1n, target: 2n, cost: 3n }, { source: 1n, target: 2n, cost: 3n }, { source: 0n, target: 1n, cost: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.reachable !== 3n || actual.cost_sum !== 2n || actual.most_expensive !== 1n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 2n, cost: 3n }, { source: 2n, target: 3n, cost: 5n }, { source: 1n, target: 3n, cost: 1n }, { source: 0n, target: 3n, cost: 6n }, { source: 1n, target: 3n, cost: 2n }, { source: 2n, target: 3n, cost: 3n }, { source: 0n, target: 3n, cost: 4n }, { source: 0n, target: 1n, cost: 5n }, { source: 2n, target: 3n, cost: 7n }, { source: 2n, target: 3n, cost: 2n }, { source: 1n, target: 2n, cost: 7n }];
    const actual: Report = solve(rows, 4n);
    if (actual.reachable !== 4n || actual.cost_sum !== 17n || actual.most_expensive !== 8n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 4n, cost: 2n }, { source: 3n, target: 4n, cost: 6n }, { source: 2n, target: 4n, cost: 3n }, { source: 2n, target: 3n, cost: 2n }, { source: 0n, target: 3n, cost: 2n }, { source: 1n, target: 3n, cost: 2n }, { source: 1n, target: 4n, cost: 1n }, { source: 1n, target: 2n, cost: 7n }, { source: 3n, target: 4n, cost: 1n }, { source: 3n, target: 4n, cost: 2n }, { source: 0n, target: 4n, cost: 1n }, { source: 3n, target: 4n, cost: 6n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 3n || actual.cost_sum !== 3n || actual.most_expensive !== 2n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 3n, cost: 2n }];
    const actual: Report = solve(rows, 7n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ source: 3n, target: 4n, cost: 4n }, { source: 3n, target: 5n, cost: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.reachable !== 1n || actual.cost_sum !== 0n || actual.most_expensive !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 4n }, { source: 0n, target: 1n, cost: 1n }, { source: 0n, target: 1n, cost: 2n }, { source: 0n, target: 1n, cost: 5n }];
    const actual: Report = solve(rows, 2n);
    if (actual.reachable !== 2n || actual.cost_sum !== 1n || actual.most_expensive !== 1n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 2n, cost: 3n }, { source: 1n, target: 2n, cost: 1n }, { source: 0n, target: 2n, cost: 4n }, { source: 1n, target: 2n, cost: 7n }, { source: 1n, target: 2n, cost: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.reachable !== 2n || actual.cost_sum !== 4n || actual.most_expensive !== 4n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 3n }, { source: 1n, target: 2n, cost: 2n }, { source: 1n, target: 3n, cost: 3n }, { source: 2n, target: 3n, cost: 5n }, { source: 2n, target: 3n, cost: 5n }, { source: 2n, target: 3n, cost: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.reachable !== 4n || actual.cost_sum !== 14n || actual.most_expensive !== 6n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ source: 1n, target: 2n, cost: 2n }, { source: 2n, target: 3n, cost: 6n }, { source: 1n, target: 3n, cost: 5n }, { source: 0n, target: 2n, cost: 1n }, { source: 0n, target: 1n, cost: 4n }, { source: 2n, target: 3n, cost: 2n }, { source: 1n, target: 4n, cost: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 5n || actual.cost_sum !== 13n || actual.most_expensive !== 5n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ source: 2n, target: 3n, cost: 2n }, { source: 0n, target: 3n, cost: 2n }, { source: 1n, target: 3n, cost: 3n }, { source: 2n, target: 5n, cost: 4n }, { source: 0n, target: 2n, cost: 5n }, { source: 3n, target: 5n, cost: 1n }, { source: 2n, target: 5n, cost: 2n }, { source: 0n, target: 5n, cost: 5n }];
    const actual: Report = solve(rows, 6n);
    if (actual.reachable !== 4n || actual.cost_sum !== 10n || actual.most_expensive !== 5n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 9n }, { source: 0n, target: 1n, cost: 3n }, { source: 1n, target: 3n, cost: 5n }, { source: 2n, target: 3n, cost: 1n }, { source: 0n, target: 3n, cost: 12n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 3n || actual.cost_sum !== 11n || actual.most_expensive !== 8n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ source: 0n, target: 1n, cost: 8n }, { source: 0n, target: 2n, cost: 2n }, { source: 1n, target: 3n, cost: 1n }, { source: 2n, target: 3n, cost: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.reachable !== 4n || actual.cost_sum !== 15n || actual.most_expensive !== 8n) throw new Error("fixture 23");
}
