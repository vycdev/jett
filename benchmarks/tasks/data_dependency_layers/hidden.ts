import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.layers !== 0n || actual.last_layer_count !== 0n || actual.layer_sum !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.layers !== 1n || actual.last_layer_count !== 5n || actual.layer_sum !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.layers !== 1n || actual.last_layer_count !== 1n || actual.layer_sum !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.layers !== 2n || actual.last_layer_count !== 1n || actual.layer_sum !== 1n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 1n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.layers !== 2n || actual.last_layer_count !== 1n || actual.layer_sum !== 1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 3n }, { prerequisite: 0n, dependent: 3n }, { prerequisite: 0n, dependent: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.layers !== 2n || actual.last_layer_count !== 2n || actual.layer_sum !== 2n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 2n, dependent: 3n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 2n, dependent: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.layers !== 3n || actual.last_layer_count !== 1n || actual.layer_sum !== 3n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 2n }, { prerequisite: 0n, dependent: 4n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 4n, dependent: 5n }, { prerequisite: 4n, dependent: 5n }];
    const actual: Report = solve(rows, 6n);
    if (actual.layers !== 3n || actual.last_layer_count !== 1n || actual.layer_sum !== 4n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 6n }, { prerequisite: 4n, dependent: 5n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 0n, dependent: 4n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 3n, dependent: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.layers !== 3n || actual.last_layer_count !== 1n || actual.layer_sum !== 6n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 3n, dependent: 4n }, { prerequisite: 3n, dependent: 5n }, { prerequisite: 6n, dependent: 7n }, { prerequisite: 2n, dependent: 4n }, { prerequisite: 3n, dependent: 5n }, { prerequisite: 6n, dependent: 7n }, { prerequisite: 3n, dependent: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.layers !== 2n || actual.last_layer_count !== 3n || actual.layer_sum !== 3n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.layers !== 2n || actual.last_layer_count !== 1n || actual.layer_sum !== 1n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 0n, dependent: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.layers !== 3n || actual.last_layer_count !== 1n || actual.layer_sum !== 3n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 1n, dependent: 2n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 1n, dependent: 3n }, { prerequisite: 0n, dependent: 3n }, { prerequisite: 1n, dependent: 3n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 0n, dependent: 3n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 1n, dependent: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.layers !== 4n || actual.last_layer_count !== 1n || actual.layer_sum !== 6n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 1n, dependent: 4n }, { prerequisite: 3n, dependent: 4n }, { prerequisite: 2n, dependent: 4n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 0n, dependent: 3n }, { prerequisite: 1n, dependent: 3n }, { prerequisite: 1n, dependent: 4n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 3n, dependent: 4n }, { prerequisite: 3n, dependent: 4n }, { prerequisite: 0n, dependent: 4n }, { prerequisite: 3n, dependent: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.layers !== 4n || actual.last_layer_count !== 1n || actual.layer_sum !== 6n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.layers !== 1n || actual.last_layer_count !== 6n || actual.layer_sum !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 2n, dependent: 3n }];
    const actual: Report = solve(rows, 7n);
    if (actual.layers !== 2n || actual.last_layer_count !== 1n || actual.layer_sum !== 1n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 3n, dependent: 4n }, { prerequisite: 3n, dependent: 5n }];
    const actual: Report = solve(rows, 8n);
    if (actual.layers !== 2n || actual.last_layer_count !== 2n || actual.layer_sum !== 2n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.layers !== 2n || actual.last_layer_count !== 1n || actual.layer_sum !== 1n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 1n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.layers !== 2n || actual.last_layer_count !== 1n || actual.layer_sum !== 1n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 1n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 1n, dependent: 3n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 2n, dependent: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.layers !== 4n || actual.last_layer_count !== 1n || actual.layer_sum !== 6n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 1n, dependent: 2n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 1n, dependent: 3n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 1n, dependent: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.layers !== 4n || actual.last_layer_count !== 1n || actual.layer_sum !== 8n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 2n, dependent: 3n }, { prerequisite: 0n, dependent: 3n }, { prerequisite: 1n, dependent: 3n }, { prerequisite: 2n, dependent: 5n }, { prerequisite: 0n, dependent: 2n }, { prerequisite: 3n, dependent: 5n }, { prerequisite: 2n, dependent: 5n }, { prerequisite: 0n, dependent: 5n }];
    const actual: Report = solve(rows, 6n);
    if (actual.layers !== 4n || actual.last_layer_count !== 1n || actual.layer_sum !== 6n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 3n, dependent: 4n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 0n, dependent: 1n }, { prerequisite: 0n, dependent: 4n }];
    const actual: Report = solve(rows, 6n);
    if (actual.layers !== 5n || actual.last_layer_count !== 1n || actual.layer_sum !== 10n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ prerequisite: 0n, dependent: 2n }, { prerequisite: 1n, dependent: 2n }, { prerequisite: 2n, dependent: 3n }, { prerequisite: 1n, dependent: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.layers !== 3n || actual.last_layer_count !== 1n || actual.layer_sum !== 4n) throw new Error("fixture 23");
}
