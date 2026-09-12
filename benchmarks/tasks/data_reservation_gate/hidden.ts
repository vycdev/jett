import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.accepted !== 0n || actual.rejected !== 0n || actual.occupied !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 0n || actual.rejected !== 0n || actual.occupied !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.accepted !== 0n || actual.rejected !== 0n || actual.occupied !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ party: 3n, seats: 8n }];
    const actual: Report = solve(rows, 2n);
    if (actual.accepted !== 0n || actual.rejected !== 1n || actual.occupied !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 6n }, { party: 3n, seats: 7n }];
    const actual: Report = solve(rows, 3n);
    if (actual.accepted !== 0n || actual.rejected !== 2n || actual.occupied !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ party: 2n, seats: 11n }, { party: 0n, seats: 0n }, { party: 0n, seats: (-2n) }];
    const actual: Report = solve(rows, 4n);
    if (actual.accepted !== 0n || actual.rejected !== 3n || actual.occupied !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ party: 3n, seats: 10n }, { party: 2n, seats: 2n }, { party: 4n, seats: 0n }, { party: 0n, seats: 11n }];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 1n || actual.rejected !== 3n || actual.occupied !== 2n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ party: 3n, seats: 10n }, { party: 0n, seats: 5n }, { party: 4n, seats: 2n }, { party: 1n, seats: 3n }, { party: 1n, seats: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.accepted !== 2n || actual.rejected !== 3n || actual.occupied !== 6n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ party: 3n, seats: 5n }, { party: 1n, seats: (-2n) }, { party: 3n, seats: 11n }, { party: 4n, seats: 2n }, { party: 0n, seats: 10n }, { party: 3n, seats: 6n }];
    const actual: Report = solve(rows, 7n);
    if (actual.accepted !== 2n || actual.rejected !== 4n || actual.occupied !== 7n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 5n }, { party: 1n, seats: 1n }, { party: 3n, seats: (-1n) }, { party: 0n, seats: 8n }, { party: 3n, seats: 7n }, { party: 0n, seats: 5n }, { party: 1n, seats: 11n }];
    const actual: Report = solve(rows, 8n);
    if (actual.accepted !== 1n || actual.rejected !== 6n || actual.occupied !== 5n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ party: 0n, seats: (-3n) }, { party: 1n, seats: 7n }, { party: 1n, seats: 7n }, { party: 3n, seats: (-2n) }, { party: 4n, seats: (-1n) }, { party: 0n, seats: 4n }, { party: 2n, seats: 8n }, { party: 1n, seats: (-2n) }];
    const actual: Report = solve(rows, 1n);
    if (actual.accepted !== 0n || actual.rejected !== 8n || actual.occupied !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ party: 2n, seats: 10n }, { party: 2n, seats: 11n }, { party: 4n, seats: 2n }, { party: 3n, seats: 1n }, { party: 2n, seats: 1n }, { party: 2n, seats: 4n }, { party: 0n, seats: 2n }, { party: 3n, seats: (-3n) }, { party: 4n, seats: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.accepted !== 1n || actual.rejected !== 8n || actual.occupied !== 2n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ party: 0n, seats: 7n }, { party: 3n, seats: (-3n) }, { party: 1n, seats: 5n }, { party: 0n, seats: 10n }, { party: 1n, seats: 8n }, { party: 3n, seats: 6n }, { party: 0n, seats: 2n }, { party: 4n, seats: 5n }, { party: 2n, seats: (-1n) }, { party: 0n, seats: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.accepted !== 1n || actual.rejected !== 9n || actual.occupied !== 2n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ party: 2n, seats: 5n }, { party: 4n, seats: 3n }, { party: 3n, seats: 11n }, { party: 1n, seats: 8n }, { party: 3n, seats: 4n }, { party: 0n, seats: 1n }, { party: 4n, seats: 3n }, { party: 1n, seats: 5n }, { party: 4n, seats: 6n }, { party: 1n, seats: 9n }, { party: 4n, seats: 7n }];
    const actual: Report = solve(rows, 4n);
    if (actual.accepted !== 2n || actual.rejected !== 9n || actual.occupied !== 4n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 3n }, { party: 1n, seats: 7n }, { party: 3n, seats: 4n }, { party: 2n, seats: 6n }, { party: 2n, seats: 11n }, { party: 1n, seats: 0n }, { party: 0n, seats: 2n }, { party: 1n, seats: 10n }, { party: 1n, seats: (-1n) }, { party: 0n, seats: (-1n) }, { party: 3n, seats: 11n }, { party: 3n, seats: (-3n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 2n || actual.rejected !== 10n || actual.occupied !== 5n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.accepted !== 0n || actual.rejected !== 0n || actual.occupied !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ party: 3n, seats: (-1n) }];
    const actual: Report = solve(rows, 7n);
    if (actual.accepted !== 0n || actual.rejected !== 1n || actual.occupied !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ party: 0n, seats: 4n }, { party: 4n, seats: 8n }];
    const actual: Report = solve(rows, 8n);
    if (actual.accepted !== 1n || actual.rejected !== 1n || actual.occupied !== 4n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ party: 3n, seats: 7n }, { party: 0n, seats: (-1n) }, { party: 0n, seats: 3n }];
    const actual: Report = solve(rows, 1n);
    if (actual.accepted !== 0n || actual.rejected !== 3n || actual.occupied !== 0n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 11n }, { party: 0n, seats: 5n }, { party: 3n, seats: 4n }, { party: 0n, seats: 4n }];
    const actual: Report = solve(rows, 2n);
    if (actual.accepted !== 0n || actual.rejected !== 4n || actual.occupied !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 1n }, { party: 2n, seats: 5n }, { party: 4n, seats: (-3n) }, { party: 2n, seats: 5n }, { party: 1n, seats: (-3n) }];
    const actual: Report = solve(rows, 3n);
    if (actual.accepted !== 1n || actual.rejected !== 4n || actual.occupied !== 1n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ party: 0n, seats: 4n }, { party: 2n, seats: 1n }, { party: 3n, seats: 10n }, { party: 4n, seats: (-3n) }, { party: 2n, seats: 4n }, { party: 1n, seats: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.accepted !== 1n || actual.rejected !== 5n || actual.occupied !== 4n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ party: 2n, seats: 1n }, { party: 0n, seats: 6n }, { party: 1n, seats: 6n }, { party: 0n, seats: 0n }, { party: 1n, seats: 0n }, { party: 2n, seats: 11n }, { party: 4n, seats: (-1n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 1n || actual.rejected !== 6n || actual.occupied !== 1n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ party: 4n, seats: 6n }, { party: 4n, seats: (-1n) }, { party: 0n, seats: (-3n) }, { party: 2n, seats: 9n }, { party: 0n, seats: (-1n) }, { party: 0n, seats: 2n }, { party: 1n, seats: (-2n) }, { party: 1n, seats: (-1n) }];
    const actual: Report = solve(rows, 6n);
    if (actual.accepted !== 1n || actual.rejected !== 7n || actual.occupied !== 6n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ party: 0n, seats: 5n }];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 1n || actual.rejected !== 0n || actual.occupied !== 5n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ party: 0n, seats: 0n }, { party: 0n, seats: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.accepted !== 1n || actual.rejected !== 1n || actual.occupied !== 2n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 2n }, { party: 2n, seats: 3n }, { party: 1n, seats: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 2n || actual.rejected !== 1n || actual.occupied !== 5n) throw new Error("fixture 26");
}
{
    const rows: readonly Entry[] = [{ party: 1n, seats: 6n }, { party: 1n, seats: 3n }, { party: 1n, seats: 1n }, { party: 2n, seats: 2n }];
    const actual: Report = solve(rows, 5n);
    if (actual.accepted !== 2n || actual.rejected !== 2n || actual.occupied !== 5n) throw new Error("fixture 27");
}
