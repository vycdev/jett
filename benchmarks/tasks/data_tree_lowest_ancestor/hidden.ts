import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 2n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ node: 3n, parent: 0n }, { node: 1n, parent: 0n }, { node: 2n, parent: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ node: 3n, parent: 0n }, { node: 1n, parent: 0n }, { node: 2n, parent: 1n }, { node: 4n, parent: 0n }];
    const actual: Report = solve(rows, 5n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ node: 5n, parent: 4n }, { node: 2n, parent: 1n }, { node: 3n, parent: 0n }, { node: 4n, parent: 0n }, { node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 6n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ node: 5n, parent: 2n }, { node: 3n, parent: 2n }, { node: 2n, parent: 1n }, { node: 4n, parent: 0n }, { node: 6n, parent: 3n }, { node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 7n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ node: 6n, parent: 1n }, { node: 7n, parent: 6n }, { node: 4n, parent: 0n }, { node: 2n, parent: 1n }, { node: 5n, parent: 1n }, { node: 3n, parent: 1n }, { node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 8n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.ancestor !== 1n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 0n }];
    const actual: Report = solve(rows, 2n);
    if (actual.ancestor !== 2n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 3n, parent: 1n }, { node: 2n, parent: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.ancestor !== 3n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 1n }, { node: 2n, parent: 1n }, { node: 1n, parent: 0n }, { node: 3n, parent: 1n }];
    const actual: Report = solve(rows, 4n);
    if (actual.ancestor !== 4n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ node: 2n, parent: 1n }, { node: 3n, parent: 0n }, { node: 4n, parent: 3n }, { node: 1n, parent: 0n }, { node: 5n, parent: 0n }];
    const actual: Report = solve(rows, 5n);
    if (actual.ancestor !== 5n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 7n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 1n }];
    const actual: Report = solve(rows, 8n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.ancestor !== 2n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 1n }, { node: 3n, parent: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.ancestor !== 3n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 3n }, { node: 1n, parent: 0n }, { node: 3n, parent: 0n }, { node: 2n, parent: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.ancestor !== 4n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ node: 4n, parent: 3n }, { node: 1n, parent: 0n }, { node: 2n, parent: 1n }, { node: 5n, parent: 2n }, { node: 3n, parent: 0n }];
    const actual: Report = solve(rows, 5n);
    if (actual.ancestor !== 5n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ node: 3n, parent: 0n }, { node: 2n, parent: 0n }, { node: 6n, parent: 0n }, { node: 5n, parent: 1n }, { node: 4n, parent: 2n }, { node: 1n, parent: 0n }];
    const actual: Report = solve(rows, 6n);
    if (actual.ancestor !== 6n || actual.selected_distance !== 0n || actual.largest_distance !== 0n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 1n }, { node: 3n, parent: 1n }, { node: 4n, parent: 2n }];
    const actual: Report = solve(rows, 1n);
    if (actual.ancestor !== 1n || actual.selected_distance !== 0n || actual.largest_distance !== 2n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.ancestor !== 0n || actual.selected_distance !== -1n || actual.largest_distance !== -1n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ node: 1n, parent: 0n }, { node: 2n, parent: 1n }, { node: 3n, parent: 1n }, { node: 4n, parent: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.ancestor !== 1n || actual.selected_distance !== 1n || actual.largest_distance !== 2n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ node: 30n, parent: 2n }, { node: 12n, parent: 9n }, { node: 2n, parent: 0n }, { node: 9n, parent: 2n }];
    const actual: Report = solve(rows, 12n);
    if (actual.ancestor !== 2n || actual.selected_distance !== 2n || actual.largest_distance !== 1n) throw new Error("fixture 26");
}
