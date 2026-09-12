import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 5n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 1n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ duration: 8n, deadline: 23n, penalty: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 10n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ duration: 4n, deadline: 18n, penalty: 3n }, { duration: 7n, deadline: 20n, penalty: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 14n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ duration: 1n, deadline: 0n, penalty: 2n }, { duration: 2n, deadline: 2n, penalty: 4n }, { duration: 4n, deadline: 11n, penalty: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.late !== 2n || actual.weighted_tardiness !== 30n || actual.finish !== 11n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ duration: 6n, deadline: 18n, penalty: 2n }, { duration: 8n, deadline: 2n, penalty: 2n }, { duration: 7n, deadline: 27n, penalty: 3n }, { duration: 1n, deadline: 17n, penalty: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.late !== 2n || actual.weighted_tardiness !== 64n || actual.finish !== 27n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ duration: 3n, deadline: 12n, penalty: 1n }, { duration: 4n, deadline: 8n, penalty: 1n }, { duration: 7n, deadline: 16n, penalty: 2n }, { duration: 2n, deadline: 24n, penalty: 4n }, { duration: 6n, deadline: 14n, penalty: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.late !== 3n || actual.weighted_tardiness !== 27n || actual.finish !== 28n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ duration: 7n, deadline: 19n, penalty: 2n }, { duration: 1n, deadline: 7n, penalty: 3n }, { duration: 2n, deadline: 14n, penalty: 2n }, { duration: 1n, deadline: 0n, penalty: 4n }, { duration: 7n, deadline: 21n, penalty: 4n }, { duration: 1n, deadline: 17n, penalty: 4n }];
    const actual: Report = solve(rows, 7n);
    if (actual.late !== 5n || actual.weighted_tardiness !== 154n || actual.finish !== 26n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ duration: 3n, deadline: 29n, penalty: 3n }, { duration: 1n, deadline: 1n, penalty: 3n }, { duration: 4n, deadline: 21n, penalty: 4n }, { duration: 3n, deadline: 21n, penalty: 1n }, { duration: 8n, deadline: 3n, penalty: 3n }, { duration: 3n, deadline: 26n, penalty: 4n }, { duration: 2n, deadline: 15n, penalty: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.late !== 4n || actual.weighted_tardiness !== 172n || actual.finish !== 32n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ duration: 7n, deadline: 24n, penalty: 2n }, { duration: 2n, deadline: 24n, penalty: 3n }, { duration: 4n, deadline: 21n, penalty: 3n }, { duration: 3n, deadline: 27n, penalty: 3n }, { duration: 3n, deadline: 13n, penalty: 3n }, { duration: 3n, deadline: 8n, penalty: 3n }, { duration: 6n, deadline: 15n, penalty: 2n }, { duration: 1n, deadline: 10n, penalty: 1n }];
    const actual: Report = solve(rows, 1n);
    if (actual.late !== 4n || actual.weighted_tardiness !== 114n || actual.finish !== 30n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ duration: 8n, deadline: 0n, penalty: 4n }, { duration: 6n, deadline: 6n, penalty: 1n }, { duration: 2n, deadline: 15n, penalty: 1n }, { duration: 1n, deadline: 4n, penalty: 1n }, { duration: 2n, deadline: 20n, penalty: 2n }, { duration: 6n, deadline: 12n, penalty: 3n }, { duration: 2n, deadline: 10n, penalty: 3n }, { duration: 5n, deadline: 29n, penalty: 3n }, { duration: 3n, deadline: 6n, penalty: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.late !== 9n || actual.weighted_tardiness !== 218n || actual.finish !== 37n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ duration: 5n, deadline: 6n, penalty: 3n }, { duration: 5n, deadline: 16n, penalty: 4n }, { duration: 7n, deadline: 29n, penalty: 1n }, { duration: 4n, deadline: 22n, penalty: 4n }, { duration: 8n, deadline: 6n, penalty: 1n }, { duration: 5n, deadline: 29n, penalty: 1n }, { duration: 7n, deadline: 7n, penalty: 2n }, { duration: 3n, deadline: 24n, penalty: 2n }, { duration: 4n, deadline: 13n, penalty: 2n }, { duration: 3n, deadline: 21n, penalty: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.late !== 8n || actual.weighted_tardiness !== 310n || actual.finish !== 54n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ duration: 8n, deadline: 14n, penalty: 3n }, { duration: 6n, deadline: 8n, penalty: 3n }, { duration: 4n, deadline: 6n, penalty: 1n }, { duration: 6n, deadline: 4n, penalty: 2n }, { duration: 5n, deadline: 5n, penalty: 2n }, { duration: 2n, deadline: 4n, penalty: 1n }, { duration: 8n, deadline: 28n, penalty: 4n }, { duration: 1n, deadline: 12n, penalty: 4n }, { duration: 3n, deadline: 16n, penalty: 1n }, { duration: 8n, deadline: 2n, penalty: 4n }, { duration: 7n, deadline: 21n, penalty: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.late !== 10n || actual.weighted_tardiness !== 735n || actual.finish !== 62n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ duration: 2n, deadline: 5n, penalty: 4n }, { duration: 1n, deadline: 13n, penalty: 4n }, { duration: 3n, deadline: 29n, penalty: 3n }, { duration: 1n, deadline: 17n, penalty: 2n }, { duration: 8n, deadline: 14n, penalty: 3n }, { duration: 1n, deadline: 15n, penalty: 2n }, { duration: 4n, deadline: 9n, penalty: 3n }, { duration: 7n, deadline: 16n, penalty: 1n }, { duration: 5n, deadline: 9n, penalty: 2n }, { duration: 1n, deadline: 21n, penalty: 1n }, { duration: 8n, deadline: 13n, penalty: 3n }, { duration: 5n, deadline: 25n, penalty: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.late !== 9n || actual.weighted_tardiness !== 378n || actual.finish !== 51n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 6n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ duration: 1n, deadline: 16n, penalty: 1n }];
    const actual: Report = solve(rows, 7n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 8n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ duration: 2n, deadline: 11n, penalty: 4n }, { duration: 1n, deadline: 28n, penalty: 2n }];
    const actual: Report = solve(rows, 8n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 11n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ duration: 8n, deadline: 23n, penalty: 3n }, { duration: 5n, deadline: 21n, penalty: 1n }, { duration: 3n, deadline: 19n, penalty: 1n }];
    const actual: Report = solve(rows, 1n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 17n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ duration: 4n, deadline: 18n, penalty: 2n }, { duration: 4n, deadline: 7n, penalty: 3n }, { duration: 1n, deadline: 22n, penalty: 2n }, { duration: 8n, deadline: 16n, penalty: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.late !== 2n || actual.weighted_tardiness !== 12n || actual.finish !== 19n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ duration: 3n, deadline: 2n, penalty: 1n }, { duration: 1n, deadline: 29n, penalty: 4n }, { duration: 5n, deadline: 24n, penalty: 1n }, { duration: 3n, deadline: 28n, penalty: 2n }, { duration: 2n, deadline: 10n, penalty: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.late !== 2n || actual.weighted_tardiness !== 18n || actual.finish !== 17n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ duration: 3n, deadline: 3n, penalty: 3n }, { duration: 3n, deadline: 4n, penalty: 2n }, { duration: 6n, deadline: 9n, penalty: 4n }, { duration: 1n, deadline: 7n, penalty: 4n }, { duration: 7n, deadline: 2n, penalty: 3n }, { duration: 3n, deadline: 0n, penalty: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.late !== 6n || actual.weighted_tardiness !== 266n || actual.finish !== 27n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ duration: 1n, deadline: 26n, penalty: 4n }, { duration: 1n, deadline: 5n, penalty: 2n }, { duration: 1n, deadline: 15n, penalty: 4n }, { duration: 5n, deadline: 3n, penalty: 4n }, { duration: 8n, deadline: 15n, penalty: 1n }, { duration: 7n, deadline: 0n, penalty: 2n }, { duration: 2n, deadline: 7n, penalty: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.late !== 5n || actual.weighted_tardiness !== 129n || actual.finish !== 30n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ duration: 6n, deadline: 10n, penalty: 4n }, { duration: 2n, deadline: 10n, penalty: 4n }, { duration: 2n, deadline: 8n, penalty: 1n }, { duration: 2n, deadline: 28n, penalty: 3n }, { duration: 5n, deadline: 15n, penalty: 2n }, { duration: 7n, deadline: 11n, penalty: 3n }, { duration: 8n, deadline: 2n, penalty: 2n }, { duration: 7n, deadline: 2n, penalty: 2n }];
    const actual: Report = solve(rows, 6n);
    if (actual.late !== 7n || actual.weighted_tardiness !== 263n || actual.finish !== 45n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ duration: 2n, deadline: 5n, penalty: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.late !== 0n || actual.weighted_tardiness !== 0n || actual.finish !== 5n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ duration: 1n, deadline: 0n, penalty: 2n }, { duration: 3n, deadline: 4n, penalty: 5n }];
    const actual: Report = solve(rows, 0n);
    if (actual.late !== 1n || actual.weighted_tardiness !== 2n || actual.finish !== 4n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ duration: 3n, deadline: 5n, penalty: 2n }, { duration: 4n, deadline: 6n, penalty: 3n }, { duration: 1n, deadline: 20n, penalty: 9n }];
    const actual: Report = solve(rows, 2n);
    if (actual.late !== 1n || actual.weighted_tardiness !== 9n || actual.finish !== 10n) throw new Error("fixture 26");
}
