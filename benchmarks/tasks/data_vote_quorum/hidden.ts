import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ voter: 3n, choice: 2n, weight: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ voter: 4n, choice: 2n, weight: 4n }, { voter: 4n, choice: 2n, weight: (-2n) }];
    const actual: Report = solve(rows, 3n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ voter: 0n, choice: 1n, weight: 8n }, { voter: 0n, choice: 0n, weight: 6n }, { voter: 3n, choice: 1n, weight: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.yes_weight !== 3n || actual.no_weight !== 6n || actual.quorum_met !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ voter: 2n, choice: 2n, weight: 7n }, { voter: 1n, choice: 3n, weight: (-1n) }, { voter: 1n, choice: 3n, weight: 3n }, { voter: 0n, choice: 2n, weight: 9n }];
    const actual: Report = solve(rows, 5n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ voter: 1n, choice: 3n, weight: (-2n) }, { voter: 1n, choice: 2n, weight: (-1n) }, { voter: 3n, choice: 1n, weight: (-1n) }, { voter: 4n, choice: 3n, weight: 9n }, { voter: 4n, choice: 2n, weight: 5n }];
    const actual: Report = solve(rows, 6n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ voter: 0n, choice: 3n, weight: 7n }, { voter: 4n, choice: 1n, weight: 6n }, { voter: 0n, choice: 1n, weight: 2n }, { voter: 0n, choice: 3n, weight: 0n }, { voter: 0n, choice: 0n, weight: 9n }, { voter: 3n, choice: 3n, weight: 8n }];
    const actual: Report = solve(rows, 7n);
    if (actual.yes_weight !== 6n || actual.no_weight !== 9n || actual.quorum_met !== 0n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ voter: 3n, choice: 0n, weight: 6n }, { voter: 3n, choice: 1n, weight: 3n }, { voter: 0n, choice: 0n, weight: 2n }, { voter: 1n, choice: 3n, weight: 0n }, { voter: 0n, choice: 3n, weight: (-1n) }, { voter: 2n, choice: 1n, weight: 4n }, { voter: 0n, choice: 3n, weight: 7n }];
    const actual: Report = solve(rows, 8n);
    if (actual.yes_weight !== 7n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ voter: 2n, choice: 3n, weight: 0n }, { voter: 0n, choice: 2n, weight: 1n }, { voter: 2n, choice: 1n, weight: 7n }, { voter: 2n, choice: 1n, weight: 4n }, { voter: 2n, choice: 1n, weight: 2n }, { voter: 2n, choice: 2n, weight: 5n }, { voter: 1n, choice: 0n, weight: 3n }, { voter: 0n, choice: 3n, weight: (-2n) }];
    const actual: Report = solve(rows, 1n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 3n || actual.quorum_met !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ voter: 3n, choice: 2n, weight: 1n }, { voter: 0n, choice: 0n, weight: 5n }, { voter: 0n, choice: 0n, weight: 0n }, { voter: 4n, choice: 0n, weight: (-1n) }, { voter: 1n, choice: 2n, weight: 4n }, { voter: 4n, choice: 2n, weight: 9n }, { voter: 0n, choice: 2n, weight: 2n }, { voter: 4n, choice: 2n, weight: 3n }, { voter: 1n, choice: 1n, weight: (-1n) }];
    const actual: Report = solve(rows, 2n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ voter: 2n, choice: 1n, weight: 3n }, { voter: 4n, choice: 2n, weight: 6n }, { voter: 3n, choice: 3n, weight: (-2n) }, { voter: 1n, choice: 3n, weight: 5n }, { voter: 1n, choice: 0n, weight: 2n }, { voter: 0n, choice: 3n, weight: 1n }, { voter: 1n, choice: 1n, weight: 7n }, { voter: 4n, choice: 1n, weight: 1n }, { voter: 3n, choice: 1n, weight: 0n }, { voter: 1n, choice: 3n, weight: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.yes_weight !== 4n || actual.no_weight !== 0n || actual.quorum_met !== 1n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ voter: 2n, choice: 2n, weight: 2n }, { voter: 2n, choice: 1n, weight: 1n }, { voter: 4n, choice: 0n, weight: 3n }, { voter: 1n, choice: 1n, weight: 2n }, { voter: 1n, choice: 1n, weight: 7n }, { voter: 0n, choice: 1n, weight: (-1n) }, { voter: 3n, choice: 3n, weight: (-2n) }, { voter: 3n, choice: 3n, weight: 0n }, { voter: 4n, choice: 0n, weight: 5n }, { voter: 0n, choice: 3n, weight: 4n }, { voter: 2n, choice: 0n, weight: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.yes_weight !== 7n || actual.no_weight !== 5n || actual.quorum_met !== 1n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ voter: 3n, choice: 0n, weight: 4n }, { voter: 3n, choice: 1n, weight: 3n }, { voter: 0n, choice: 1n, weight: 5n }, { voter: 3n, choice: 2n, weight: (-2n) }, { voter: 3n, choice: 1n, weight: 1n }, { voter: 2n, choice: 2n, weight: 6n }, { voter: 3n, choice: 0n, weight: 2n }, { voter: 2n, choice: 1n, weight: (-2n) }, { voter: 0n, choice: 3n, weight: 4n }, { voter: 2n, choice: 2n, weight: 8n }, { voter: 3n, choice: 0n, weight: 6n }, { voter: 0n, choice: 0n, weight: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 9n || actual.quorum_met !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ voter: 3n, choice: 0n, weight: 1n }];
    const actual: Report = solve(rows, 7n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 1n || actual.quorum_met !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ voter: 3n, choice: 2n, weight: 2n }, { voter: 0n, choice: 1n, weight: 7n }];
    const actual: Report = solve(rows, 8n);
    if (actual.yes_weight !== 7n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ voter: 4n, choice: 0n, weight: 1n }, { voter: 4n, choice: 1n, weight: 1n }, { voter: 1n, choice: 2n, weight: (-2n) }];
    const actual: Report = solve(rows, 1n);
    if (actual.yes_weight !== 1n || actual.no_weight !== 0n || actual.quorum_met !== 1n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ voter: 4n, choice: 1n, weight: 5n }, { voter: 4n, choice: 0n, weight: 6n }, { voter: 1n, choice: 0n, weight: 8n }, { voter: 0n, choice: 0n, weight: 5n }];
    const actual: Report = solve(rows, 2n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 19n || actual.quorum_met !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ voter: 2n, choice: 0n, weight: 0n }, { voter: 1n, choice: 0n, weight: 3n }, { voter: 1n, choice: 1n, weight: (-1n) }, { voter: 2n, choice: 1n, weight: 0n }, { voter: 1n, choice: 2n, weight: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ voter: 4n, choice: 3n, weight: (-2n) }, { voter: 1n, choice: 3n, weight: 6n }, { voter: 3n, choice: 0n, weight: 2n }, { voter: 4n, choice: 1n, weight: (-2n) }, { voter: 4n, choice: 3n, weight: (-2n) }, { voter: 3n, choice: 0n, weight: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ voter: 1n, choice: 0n, weight: 5n }, { voter: 3n, choice: 2n, weight: (-1n) }, { voter: 3n, choice: 3n, weight: 5n }, { voter: 0n, choice: 3n, weight: (-2n) }, { voter: 4n, choice: 1n, weight: (-1n) }, { voter: 1n, choice: 0n, weight: 3n }, { voter: 2n, choice: 3n, weight: (-1n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 3n || actual.quorum_met !== 0n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ voter: 2n, choice: 3n, weight: (-1n) }, { voter: 2n, choice: 0n, weight: (-1n) }, { voter: 2n, choice: 2n, weight: 5n }, { voter: 1n, choice: 3n, weight: 3n }, { voter: 4n, choice: 2n, weight: 6n }, { voter: 3n, choice: 0n, weight: 1n }, { voter: 3n, choice: 0n, weight: 1n }, { voter: 3n, choice: 0n, weight: 9n }];
    const actual: Report = solve(rows, 6n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 9n || actual.quorum_met !== 0n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ voter: 0n, choice: 1n, weight: 5n }, { voter: 0n, choice: 1n, weight: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.yes_weight !== 0n || actual.no_weight !== 0n || actual.quorum_met !== 0n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ voter: 0n, choice: 1n, weight: 3n }, { voter: 1n, choice: 0n, weight: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.yes_weight !== 3n || actual.no_weight !== 3n || actual.quorum_met !== 0n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ voter: 0n, choice: 1n, weight: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.yes_weight !== 3n || actual.no_weight !== 0n || actual.quorum_met !== 1n) throw new Error("fixture 26");
}
{
    const rows: readonly Entry[] = [{ voter: 1n, choice: 1n, weight: 5n }, { voter: 2n, choice: 0n, weight: 3n }, { voter: 1n, choice: 2n, weight: 9n }, { voter: 3n, choice: 1n, weight: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.yes_weight !== 4n || actual.no_weight !== 3n || actual.quorum_met !== 1n) throw new Error("fixture 27");
}
