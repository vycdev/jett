import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 0n || actual.credit !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ invoice: 3n, amount: 8n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 8n || actual.credit !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 6n }, { invoice: 3n, amount: 7n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 13n || actual.credit !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ invoice: 2n, amount: 11n }, { invoice: 0n, amount: 0n }, { invoice: 0n, amount: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 11n || actual.credit !== 2n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ invoice: 3n, amount: 10n }, { invoice: 2n, amount: 2n }, { invoice: 4n, amount: 0n }, { invoice: 0n, amount: 11n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 1n || actual.outstanding !== 23n || actual.credit !== 0n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ invoice: 3n, amount: 10n }, { invoice: 0n, amount: 5n }, { invoice: 4n, amount: 2n }, { invoice: 1n, amount: 3n }, { invoice: 1n, amount: 1n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 21n || actual.credit !== 0n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ invoice: 3n, amount: 5n }, { invoice: 1n, amount: (-2n) }, { invoice: 3n, amount: 11n }, { invoice: 4n, amount: 2n }, { invoice: 0n, amount: 10n }, { invoice: 3n, amount: 6n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 34n || actual.credit !== 2n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 5n }, { invoice: 1n, amount: 1n }, { invoice: 3n, amount: (-1n) }, { invoice: 0n, amount: 8n }, { invoice: 3n, amount: 7n }, { invoice: 0n, amount: 5n }, { invoice: 1n, amount: 11n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 36n || actual.credit !== 0n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ invoice: 0n, amount: (-3n) }, { invoice: 1n, amount: 7n }, { invoice: 1n, amount: 7n }, { invoice: 3n, amount: (-2n) }, { invoice: 4n, amount: (-1n) }, { invoice: 0n, amount: 4n }, { invoice: 2n, amount: 8n }, { invoice: 1n, amount: (-2n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 21n || actual.credit !== 3n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ invoice: 2n, amount: 10n }, { invoice: 2n, amount: 11n }, { invoice: 4n, amount: 2n }, { invoice: 3n, amount: 1n }, { invoice: 2n, amount: 1n }, { invoice: 2n, amount: 4n }, { invoice: 0n, amount: 2n }, { invoice: 3n, amount: (-3n) }, { invoice: 4n, amount: 2n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 32n || actual.credit !== 2n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ invoice: 0n, amount: 7n }, { invoice: 3n, amount: (-3n) }, { invoice: 1n, amount: 5n }, { invoice: 0n, amount: 10n }, { invoice: 1n, amount: 8n }, { invoice: 3n, amount: 6n }, { invoice: 0n, amount: 2n }, { invoice: 4n, amount: 5n }, { invoice: 2n, amount: (-1n) }, { invoice: 0n, amount: 1n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 41n || actual.credit !== 1n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ invoice: 2n, amount: 5n }, { invoice: 4n, amount: 3n }, { invoice: 3n, amount: 11n }, { invoice: 1n, amount: 8n }, { invoice: 3n, amount: 4n }, { invoice: 0n, amount: 1n }, { invoice: 4n, amount: 3n }, { invoice: 1n, amount: 5n }, { invoice: 4n, amount: 6n }, { invoice: 1n, amount: 9n }, { invoice: 4n, amount: 7n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 62n || actual.credit !== 0n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 3n }, { invoice: 1n, amount: 7n }, { invoice: 3n, amount: 4n }, { invoice: 2n, amount: 6n }, { invoice: 2n, amount: 11n }, { invoice: 1n, amount: 0n }, { invoice: 0n, amount: 2n }, { invoice: 1n, amount: 10n }, { invoice: 1n, amount: (-1n) }, { invoice: 0n, amount: (-1n) }, { invoice: 3n, amount: 11n }, { invoice: 3n, amount: (-3n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 49n || actual.credit !== 0n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ invoice: 3n, amount: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 0n || actual.credit !== 1n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ invoice: 0n, amount: 4n }, { invoice: 4n, amount: 8n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 12n || actual.credit !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ invoice: 3n, amount: 7n }, { invoice: 0n, amount: (-1n) }, { invoice: 0n, amount: 3n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 9n || actual.credit !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 11n }, { invoice: 0n, amount: 5n }, { invoice: 3n, amount: 4n }, { invoice: 0n, amount: 4n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 24n || actual.credit !== 0n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 1n }, { invoice: 2n, amount: 5n }, { invoice: 4n, amount: (-3n) }, { invoice: 2n, amount: 5n }, { invoice: 1n, amount: (-3n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 10n || actual.credit !== 5n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ invoice: 0n, amount: 4n }, { invoice: 2n, amount: 1n }, { invoice: 3n, amount: 10n }, { invoice: 4n, amount: (-3n) }, { invoice: 2n, amount: 4n }, { invoice: 1n, amount: 4n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 23n || actual.credit !== 3n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ invoice: 2n, amount: 1n }, { invoice: 0n, amount: 6n }, { invoice: 1n, amount: 6n }, { invoice: 0n, amount: 0n }, { invoice: 1n, amount: 0n }, { invoice: 2n, amount: 11n }, { invoice: 4n, amount: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 24n || actual.credit !== 1n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ invoice: 4n, amount: 6n }, { invoice: 4n, amount: (-1n) }, { invoice: 0n, amount: (-3n) }, { invoice: 2n, amount: 9n }, { invoice: 0n, amount: (-1n) }, { invoice: 0n, amount: 2n }, { invoice: 1n, amount: (-2n) }, { invoice: 1n, amount: (-1n) }];
    const actual: Report = solve(rows);
    if (actual.settled !== 0n || actual.outstanding !== 14n || actual.credit !== 5n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 0n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 1n || actual.outstanding !== 0n || actual.credit !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: (-3n) }, { invoice: 1n, amount: 3n }, { invoice: 2n, amount: (-1n) }, { invoice: 3n, amount: 2n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 1n || actual.outstanding !== 2n || actual.credit !== 1n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ invoice: 1n, amount: 5n }, { invoice: 2n, amount: (-4n) }, { invoice: 1n, amount: (-5n) }, { invoice: 3n, amount: 7n }];
    const actual: Report = solve(rows);
    if (actual.settled !== 1n || actual.outstanding !== 7n || actual.credit !== 4n) throw new Error("fixture 23");
}
