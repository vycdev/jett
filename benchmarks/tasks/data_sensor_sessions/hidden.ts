import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 0n || actual.rejected !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ sensor: 3n, operation: 2n, timestamp: 1n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 0n || actual.rejected !== 1n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ sensor: 4n, operation: 2n, timestamp: 4n }, { sensor: 4n, operation: 2n, timestamp: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 0n || actual.rejected !== 2n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ sensor: 0n, operation: 1n, timestamp: 8n }, { sensor: 0n, operation: 0n, timestamp: 6n }, { sensor: 3n, operation: 1n, timestamp: 3n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 2n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ sensor: 2n, operation: 2n, timestamp: 7n }, { sensor: 1n, operation: 3n, timestamp: (-1n) }, { sensor: 1n, operation: 3n, timestamp: 3n }, { sensor: 0n, operation: 2n, timestamp: 9n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 0n || actual.rejected !== 4n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ sensor: 1n, operation: 3n, timestamp: (-2n) }, { sensor: 1n, operation: 2n, timestamp: (-1n) }, { sensor: 3n, operation: 1n, timestamp: (-1n) }, { sensor: 4n, operation: 3n, timestamp: 9n }, { sensor: 4n, operation: 2n, timestamp: 5n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 0n || actual.rejected !== 5n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ sensor: 0n, operation: 3n, timestamp: 7n }, { sensor: 4n, operation: 1n, timestamp: 6n }, { sensor: 0n, operation: 1n, timestamp: 2n }, { sensor: 0n, operation: 3n, timestamp: 0n }, { sensor: 0n, operation: 0n, timestamp: 9n }, { sensor: 3n, operation: 3n, timestamp: 8n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 5n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ sensor: 3n, operation: 0n, timestamp: 6n }, { sensor: 3n, operation: 1n, timestamp: 3n }, { sensor: 0n, operation: 0n, timestamp: 2n }, { sensor: 1n, operation: 3n, timestamp: 0n }, { sensor: 0n, operation: 3n, timestamp: (-1n) }, { sensor: 2n, operation: 1n, timestamp: 4n }, { sensor: 0n, operation: 3n, timestamp: 7n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 2n || actual.rejected !== 5n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ sensor: 2n, operation: 3n, timestamp: 0n }, { sensor: 0n, operation: 2n, timestamp: 1n }, { sensor: 2n, operation: 1n, timestamp: 7n }, { sensor: 2n, operation: 1n, timestamp: 4n }, { sensor: 2n, operation: 1n, timestamp: 2n }, { sensor: 2n, operation: 2n, timestamp: 5n }, { sensor: 1n, operation: 0n, timestamp: 3n }, { sensor: 0n, operation: 3n, timestamp: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 7n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ sensor: 3n, operation: 2n, timestamp: 1n }, { sensor: 0n, operation: 0n, timestamp: 5n }, { sensor: 0n, operation: 0n, timestamp: 0n }, { sensor: 4n, operation: 0n, timestamp: (-1n) }, { sensor: 1n, operation: 2n, timestamp: 4n }, { sensor: 4n, operation: 2n, timestamp: 9n }, { sensor: 0n, operation: 2n, timestamp: 2n }, { sensor: 4n, operation: 2n, timestamp: 3n }, { sensor: 1n, operation: 1n, timestamp: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 8n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ sensor: 2n, operation: 1n, timestamp: 3n }, { sensor: 4n, operation: 2n, timestamp: 6n }, { sensor: 3n, operation: 3n, timestamp: (-2n) }, { sensor: 1n, operation: 3n, timestamp: 5n }, { sensor: 1n, operation: 0n, timestamp: 2n }, { sensor: 0n, operation: 3n, timestamp: 1n }, { sensor: 1n, operation: 1n, timestamp: 7n }, { sensor: 4n, operation: 1n, timestamp: 1n }, { sensor: 3n, operation: 1n, timestamp: 0n }, { sensor: 1n, operation: 3n, timestamp: 5n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 1n || actual.active !== 0n || actual.rejected !== 8n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ sensor: 2n, operation: 2n, timestamp: 2n }, { sensor: 2n, operation: 1n, timestamp: 1n }, { sensor: 4n, operation: 0n, timestamp: 3n }, { sensor: 1n, operation: 1n, timestamp: 2n }, { sensor: 1n, operation: 1n, timestamp: 7n }, { sensor: 0n, operation: 1n, timestamp: (-1n) }, { sensor: 3n, operation: 3n, timestamp: (-2n) }, { sensor: 3n, operation: 3n, timestamp: 0n }, { sensor: 4n, operation: 0n, timestamp: 5n }, { sensor: 0n, operation: 3n, timestamp: 4n }, { sensor: 2n, operation: 0n, timestamp: 0n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 2n || actual.rejected !== 9n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ sensor: 3n, operation: 0n, timestamp: 4n }, { sensor: 3n, operation: 1n, timestamp: 3n }, { sensor: 0n, operation: 1n, timestamp: 5n }, { sensor: 3n, operation: 2n, timestamp: (-2n) }, { sensor: 3n, operation: 1n, timestamp: 1n }, { sensor: 2n, operation: 2n, timestamp: 6n }, { sensor: 3n, operation: 0n, timestamp: 2n }, { sensor: 2n, operation: 1n, timestamp: (-2n) }, { sensor: 0n, operation: 3n, timestamp: 4n }, { sensor: 2n, operation: 2n, timestamp: 8n }, { sensor: 3n, operation: 0n, timestamp: 6n }, { sensor: 0n, operation: 0n, timestamp: 3n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 2n || actual.rejected !== 10n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ sensor: 3n, operation: 0n, timestamp: 1n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ sensor: 3n, operation: 2n, timestamp: 2n }, { sensor: 0n, operation: 1n, timestamp: 7n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 0n || actual.rejected !== 2n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ sensor: 4n, operation: 0n, timestamp: 1n }, { sensor: 4n, operation: 1n, timestamp: 1n }, { sensor: 1n, operation: 2n, timestamp: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.completed !== 1n || actual.active !== 0n || actual.rejected !== 1n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ sensor: 4n, operation: 1n, timestamp: 5n }, { sensor: 4n, operation: 0n, timestamp: 6n }, { sensor: 1n, operation: 0n, timestamp: 8n }, { sensor: 0n, operation: 0n, timestamp: 5n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 3n || actual.rejected !== 1n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ sensor: 2n, operation: 0n, timestamp: 0n }, { sensor: 1n, operation: 0n, timestamp: 3n }, { sensor: 1n, operation: 1n, timestamp: (-1n) }, { sensor: 2n, operation: 1n, timestamp: 0n }, { sensor: 1n, operation: 2n, timestamp: 2n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 1n || actual.active !== 1n || actual.rejected !== 2n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ sensor: 4n, operation: 3n, timestamp: (-2n) }, { sensor: 1n, operation: 3n, timestamp: 6n }, { sensor: 3n, operation: 0n, timestamp: 2n }, { sensor: 4n, operation: 1n, timestamp: (-2n) }, { sensor: 4n, operation: 3n, timestamp: (-2n) }, { sensor: 3n, operation: 0n, timestamp: 0n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 5n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ sensor: 1n, operation: 0n, timestamp: 5n }, { sensor: 3n, operation: 2n, timestamp: (-1n) }, { sensor: 3n, operation: 3n, timestamp: 5n }, { sensor: 0n, operation: 3n, timestamp: (-2n) }, { sensor: 4n, operation: 1n, timestamp: (-1n) }, { sensor: 1n, operation: 0n, timestamp: 3n }, { sensor: 2n, operation: 3n, timestamp: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 6n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ sensor: 2n, operation: 3n, timestamp: (-1n) }, { sensor: 2n, operation: 0n, timestamp: (-1n) }, { sensor: 2n, operation: 2n, timestamp: 5n }, { sensor: 1n, operation: 3n, timestamp: 3n }, { sensor: 4n, operation: 2n, timestamp: 6n }, { sensor: 3n, operation: 0n, timestamp: 1n }, { sensor: 3n, operation: 0n, timestamp: 1n }, { sensor: 3n, operation: 0n, timestamp: 9n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 0n || actual.active !== 1n || actual.rejected !== 7n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ sensor: 1n, operation: 0n, timestamp: 0n }, { sensor: 1n, operation: 1n, timestamp: 0n }, { sensor: 1n, operation: 1n, timestamp: 1n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 1n || actual.active !== 0n || actual.rejected !== 1n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ sensor: 1n, operation: 0n, timestamp: (-1n) }, { sensor: 1n, operation: 0n, timestamp: 2n }, { sensor: 1n, operation: 0n, timestamp: 3n }, { sensor: 1n, operation: 1n, timestamp: 1n }, { sensor: 1n, operation: 1n, timestamp: 2n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 1n || actual.active !== 0n || actual.rejected !== 3n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ sensor: 1n, operation: 0n, timestamp: 2n }, { sensor: 1n, operation: 1n, timestamp: 1n }, { sensor: 1n, operation: 1n, timestamp: 3n }, { sensor: 2n, operation: 0n, timestamp: 0n }];
    const actual: Report = solve(rows);
    if (actual.completed !== 1n || actual.active !== 1n || actual.rejected !== 1n) throw new Error("fixture 23");
}
