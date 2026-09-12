import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.restock_units !== 0n || actual.at_risk !== 0n || actual.worst_shortfall !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.restock_units !== 0n || actual.at_risk !== 0n || actual.worst_shortfall !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.restock_units !== 0n || actual.at_risk !== 0n || actual.worst_shortfall !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ stock: 8n, daily_demand: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.restock_units !== 0n || actual.at_risk !== 0n || actual.worst_shortfall !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ stock: 6n, daily_demand: 3n }, { stock: 7n, daily_demand: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.restock_units !== 11n || actual.at_risk !== 2n || actual.worst_shortfall !== 8n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ stock: 11n, daily_demand: 1n }, { stock: 0n, daily_demand: 6n }, { stock: (-2n), daily_demand: 5n }];
    const actual: Report = solve(rows, 4n);
    if (actual.restock_units !== 46n || actual.at_risk !== 2n || actual.worst_shortfall !== 24n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ stock: 10n, daily_demand: 2n }, { stock: 2n, daily_demand: 3n }, { stock: 0n, daily_demand: 4n }, { stock: 11n, daily_demand: 2n }];
    const actual: Report = solve(rows, 5n);
    if (actual.restock_units !== 33n || actual.at_risk !== 2n || actual.worst_shortfall !== 20n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ stock: 10n, daily_demand: 3n }, { stock: 5n, daily_demand: 5n }, { stock: 2n, daily_demand: 6n }, { stock: 3n, daily_demand: 1n }, { stock: 1n, daily_demand: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.restock_units !== 75n || actual.at_risk !== 5n || actual.worst_shortfall !== 34n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ stock: 5n, daily_demand: 6n }, { stock: (-2n), daily_demand: 5n }, { stock: 11n, daily_demand: 6n }, { stock: 2n, daily_demand: 4n }, { stock: 10n, daily_demand: 6n }, { stock: 6n, daily_demand: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.restock_units !== 192n || actual.at_risk !== 6n || actual.worst_shortfall !== 37n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ stock: 5n, daily_demand: 1n }, { stock: 1n, daily_demand: 1n }, { stock: (-1n), daily_demand: 1n }, { stock: 8n, daily_demand: 4n }, { stock: 7n, daily_demand: 4n }, { stock: 5n, daily_demand: 4n }, { stock: 11n, daily_demand: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.restock_units !== 108n || actual.at_risk !== 7n || actual.worst_shortfall !== 27n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ stock: (-3n), daily_demand: 3n }, { stock: 7n, daily_demand: 4n }, { stock: 7n, daily_demand: 1n }, { stock: (-2n), daily_demand: 3n }, { stock: (-1n), daily_demand: 4n }, { stock: 4n, daily_demand: 5n }, { stock: 8n, daily_demand: 4n }, { stock: (-2n), daily_demand: 6n }];
    const actual: Report = solve(rows, 1n);
    if (actual.restock_units !== 25n || actual.at_risk !== 5n || actual.worst_shortfall !== 8n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ stock: 10n, daily_demand: 2n }, { stock: 11n, daily_demand: 2n }, { stock: 2n, daily_demand: 2n }, { stock: 1n, daily_demand: 2n }, { stock: 1n, daily_demand: 5n }, { stock: 4n, daily_demand: 2n }, { stock: 2n, daily_demand: 1n }, { stock: (-3n), daily_demand: 4n }, { stock: 2n, daily_demand: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.restock_units !== 27n || actual.at_risk !== 5n || actual.worst_shortfall !== 11n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ stock: 7n, daily_demand: 1n }, { stock: (-3n), daily_demand: 1n }, { stock: 5n, daily_demand: 6n }, { stock: 10n, daily_demand: 1n }, { stock: 8n, daily_demand: 3n }, { stock: 6n, daily_demand: 3n }, { stock: 2n, daily_demand: 3n }, { stock: 5n, daily_demand: 3n }, { stock: (-1n), daily_demand: 2n }, { stock: 1n, daily_demand: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.restock_units !== 46n || actual.at_risk !== 8n || actual.worst_shortfall !== 13n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ stock: 5n, daily_demand: 3n }, { stock: 3n, daily_demand: 6n }, { stock: 11n, daily_demand: 1n }, { stock: 8n, daily_demand: 6n }, { stock: 4n, daily_demand: 2n }, { stock: 1n, daily_demand: 1n }, { stock: 3n, daily_demand: 2n }, { stock: 5n, daily_demand: 6n }, { stock: 6n, daily_demand: 5n }, { stock: 9n, daily_demand: 5n }, { stock: 7n, daily_demand: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.restock_units !== 101n || actual.at_risk !== 10n || actual.worst_shortfall !== 21n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ stock: 3n, daily_demand: 2n }, { stock: 7n, daily_demand: 2n }, { stock: 4n, daily_demand: 6n }, { stock: 6n, daily_demand: 3n }, { stock: 11n, daily_demand: 3n }, { stock: 0n, daily_demand: 5n }, { stock: 2n, daily_demand: 2n }, { stock: 10n, daily_demand: 3n }, { stock: (-1n), daily_demand: 5n }, { stock: (-1n), daily_demand: 1n }, { stock: 11n, daily_demand: 6n }, { stock: (-3n), daily_demand: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.restock_units !== 161n || actual.at_risk !== 12n || actual.worst_shortfall !== 26n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.restock_units !== 0n || actual.at_risk !== 0n || actual.worst_shortfall !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ stock: (-1n), daily_demand: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.restock_units !== 36n || actual.at_risk !== 1n || actual.worst_shortfall !== 36n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ stock: 4n, daily_demand: 1n }, { stock: 8n, daily_demand: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.restock_units !== 28n || actual.at_risk !== 2n || actual.worst_shortfall !== 24n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ stock: 7n, daily_demand: 3n }, { stock: (-1n), daily_demand: 4n }, { stock: 3n, daily_demand: 4n }];
    const actual: Report = solve(rows, 1n);
    if (actual.restock_units !== 6n || actual.at_risk !== 2n || actual.worst_shortfall !== 5n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ stock: 11n, daily_demand: 3n }, { stock: 5n, daily_demand: 2n }, { stock: 4n, daily_demand: 3n }, { stock: 4n, daily_demand: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.restock_units !== 2n || actual.at_risk !== 1n || actual.worst_shortfall !== 2n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ stock: 1n, daily_demand: 5n }, { stock: 5n, daily_demand: 4n }, { stock: (-3n), daily_demand: 3n }, { stock: 5n, daily_demand: 5n }, { stock: (-3n), daily_demand: 6n }];
    const actual: Report = solve(rows, 3n);
    if (actual.restock_units !== 64n || actual.at_risk !== 5n || actual.worst_shortfall !== 21n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ stock: 4n, daily_demand: 4n }, { stock: 1n, daily_demand: 6n }, { stock: 10n, daily_demand: 1n }, { stock: (-3n), daily_demand: 1n }, { stock: 4n, daily_demand: 1n }, { stock: 4n, daily_demand: 6n }];
    const actual: Report = solve(rows, 4n);
    if (actual.restock_units !== 62n || actual.at_risk !== 4n || actual.worst_shortfall !== 23n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ stock: 1n, daily_demand: 6n }, { stock: 6n, daily_demand: 5n }, { stock: 6n, daily_demand: 5n }, { stock: 0n, daily_demand: 5n }, { stock: 0n, daily_demand: 2n }, { stock: 11n, daily_demand: 1n }, { stock: (-1n), daily_demand: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.restock_units !== 123n || actual.at_risk !== 6n || actual.worst_shortfall !== 29n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ stock: 6n, daily_demand: 1n }, { stock: (-1n), daily_demand: 1n }, { stock: (-3n), daily_demand: 4n }, { stock: 9n, daily_demand: 5n }, { stock: (-1n), daily_demand: 2n }, { stock: 2n, daily_demand: 2n }, { stock: (-2n), daily_demand: 3n }, { stock: (-1n), daily_demand: 6n }];
    const actual: Report = solve(rows, 6n);
    if (actual.restock_units !== 135n || actual.at_risk !== 7n || actual.worst_shortfall !== 37n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ stock: (-5n), daily_demand: 3n }, { stock: 0n, daily_demand: 3n }];
    const actual: Report = solve(rows, 0n);
    if (actual.restock_units !== 5n || actual.at_risk !== 1n || actual.worst_shortfall !== 5n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ stock: 6n, daily_demand: 3n }, { stock: 5n, daily_demand: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.restock_units !== 1n || actual.at_risk !== 1n || actual.worst_shortfall !== 1n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ stock: 5n, daily_demand: 3n }, { stock: 8n, daily_demand: 2n }, { stock: (-2n), daily_demand: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.restock_units !== 5n || actual.at_risk !== 2n || actual.worst_shortfall !== 4n) throw new Error("fixture 26");
}
