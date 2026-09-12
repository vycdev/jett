import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.allowed !== 0n || actual.denied !== 0n || actual.defaulted !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.allowed !== 0n || actual.denied !== 5n || actual.defaulted !== 5n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.allowed !== 0n || actual.denied !== 1n || actual.defaulted !== 1n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 1n, specificity: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.allowed !== 0n || actual.denied !== 2n || actual.defaulted !== 2n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ principal: 4n, allowed: 1n, specificity: 3n }, { principal: 4n, allowed: 1n, specificity: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.allowed !== 0n || actual.denied !== 3n || actual.defaulted !== 3n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ principal: 0n, allowed: 0n, specificity: 0n }, { principal: 0n, allowed: 1n, specificity: 1n }, { principal: 2n, allowed: 1n, specificity: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.allowed !== 2n || actual.denied !== 2n || actual.defaulted !== 2n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ principal: 4n, allowed: 0n, specificity: 3n }, { principal: 0n, allowed: 0n, specificity: 3n }, { principal: 2n, allowed: 0n, specificity: 2n }, { principal: 1n, allowed: 1n, specificity: 0n }];
    const actual: Report = solve(rows, 5n);
    if (actual.allowed !== 1n || actual.denied !== 4n || actual.defaulted !== 1n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ principal: 1n, allowed: 1n, specificity: 0n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 4n, allowed: 1n, specificity: 2n }, { principal: 3n, allowed: 0n, specificity: 3n }, { principal: 4n, allowed: 0n, specificity: 0n }];
    const actual: Report = solve(rows, 6n);
    if (actual.allowed !== 2n || actual.denied !== 4n || actual.defaulted !== 3n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ principal: 1n, allowed: 1n, specificity: 0n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 0n, allowed: 1n, specificity: 3n }, { principal: 3n, allowed: 0n, specificity: 3n }, { principal: 1n, allowed: 1n, specificity: 0n }, { principal: 0n, allowed: 1n, specificity: 1n }];
    const actual: Report = solve(rows, 7n);
    if (actual.allowed !== 2n || actual.denied !== 5n || actual.defaulted !== 4n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 0n, specificity: 0n }, { principal: 3n, allowed: 0n, specificity: 2n }, { principal: 4n, allowed: 0n, specificity: 3n }, { principal: 0n, allowed: 1n, specificity: 2n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 2n, allowed: 0n, specificity: 2n }, { principal: 1n, allowed: 1n, specificity: 1n }];
    const actual: Report = solve(rows, 8n);
    if (actual.allowed !== 2n || actual.denied !== 6n || actual.defaulted !== 3n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 1n, specificity: 1n }, { principal: 2n, allowed: 1n, specificity: 2n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 2n, allowed: 0n, specificity: 3n }, { principal: 0n, allowed: 1n, specificity: 2n }, { principal: 1n, allowed: 0n, specificity: 0n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 1n, allowed: 0n, specificity: 0n }];
    const actual: Report = solve(rows, 1n);
    if (actual.allowed !== 1n || actual.denied !== 0n || actual.defaulted !== 0n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ principal: 1n, allowed: 1n, specificity: 3n }, { principal: 4n, allowed: 1n, specificity: 0n }, { principal: 2n, allowed: 1n, specificity: 2n }, { principal: 2n, allowed: 0n, specificity: 1n }, { principal: 0n, allowed: 1n, specificity: 1n }, { principal: 2n, allowed: 1n, specificity: 3n }, { principal: 3n, allowed: 0n, specificity: 1n }, { principal: 3n, allowed: 1n, specificity: 1n }, { principal: 0n, allowed: 1n, specificity: 0n }];
    const actual: Report = solve(rows, 2n);
    if (actual.allowed !== 2n || actual.denied !== 0n || actual.defaulted !== 0n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ principal: 4n, allowed: 1n, specificity: 1n }, { principal: 1n, allowed: 0n, specificity: 1n }, { principal: 1n, allowed: 1n, specificity: 1n }, { principal: 1n, allowed: 0n, specificity: 3n }, { principal: 3n, allowed: 1n, specificity: 2n }, { principal: 2n, allowed: 1n, specificity: 1n }, { principal: 1n, allowed: 0n, specificity: 2n }, { principal: 1n, allowed: 0n, specificity: 2n }, { principal: 1n, allowed: 0n, specificity: 0n }, { principal: 1n, allowed: 0n, specificity: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.allowed !== 1n || actual.denied !== 2n || actual.defaulted !== 1n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 0n, specificity: 3n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 3n, allowed: 0n, specificity: 3n }, { principal: 3n, allowed: 1n, specificity: 0n }, { principal: 1n, allowed: 1n, specificity: 0n }, { principal: 3n, allowed: 1n, specificity: 1n }, { principal: 2n, allowed: 0n, specificity: 1n }, { principal: 3n, allowed: 1n, specificity: 2n }, { principal: 0n, allowed: 1n, specificity: 1n }, { principal: 1n, allowed: 1n, specificity: 2n }, { principal: 4n, allowed: 1n, specificity: 0n }];
    const actual: Report = solve(rows, 4n);
    if (actual.allowed !== 2n || actual.denied !== 2n || actual.defaulted !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ principal: 2n, allowed: 1n, specificity: 1n }, { principal: 0n, allowed: 0n, specificity: 3n }, { principal: 3n, allowed: 1n, specificity: 2n }, { principal: 3n, allowed: 0n, specificity: 0n }, { principal: 0n, allowed: 1n, specificity: 3n }, { principal: 0n, allowed: 0n, specificity: 3n }, { principal: 2n, allowed: 1n, specificity: 0n }, { principal: 4n, allowed: 0n, specificity: 0n }, { principal: 1n, allowed: 0n, specificity: 1n }, { principal: 1n, allowed: 1n, specificity: 0n }, { principal: 4n, allowed: 0n, specificity: 3n }, { principal: 4n, allowed: 0n, specificity: 1n }];
    const actual: Report = solve(rows, 5n);
    if (actual.allowed !== 2n || actual.denied !== 3n || actual.defaulted !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.allowed !== 0n || actual.denied !== 6n || actual.defaulted !== 6n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ principal: 0n, allowed: 0n, specificity: 0n }];
    const actual: Report = solve(rows, 7n);
    if (actual.allowed !== 0n || actual.denied !== 7n || actual.defaulted !== 6n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 1n, specificity: 0n }, { principal: 1n, allowed: 0n, specificity: 0n }];
    const actual: Report = solve(rows, 8n);
    if (actual.allowed !== 1n || actual.denied !== 7n || actual.defaulted !== 6n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ principal: 2n, allowed: 0n, specificity: 1n }, { principal: 0n, allowed: 1n, specificity: 1n }, { principal: 1n, allowed: 0n, specificity: 2n }];
    const actual: Report = solve(rows, 1n);
    if (actual.allowed !== 1n || actual.denied !== 0n || actual.defaulted !== 0n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ principal: 2n, allowed: 1n, specificity: 0n }, { principal: 1n, allowed: 1n, specificity: 3n }, { principal: 0n, allowed: 1n, specificity: 1n }, { principal: 0n, allowed: 1n, specificity: 0n }];
    const actual: Report = solve(rows, 2n);
    if (actual.allowed !== 2n || actual.denied !== 0n || actual.defaulted !== 0n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 0n, specificity: 1n }, { principal: 1n, allowed: 0n, specificity: 3n }, { principal: 3n, allowed: 1n, specificity: 0n }, { principal: 3n, allowed: 1n, specificity: 3n }, { principal: 0n, allowed: 1n, specificity: 0n }];
    const actual: Report = solve(rows, 3n);
    if (actual.allowed !== 1n || actual.denied !== 2n || actual.defaulted !== 1n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ principal: 4n, allowed: 0n, specificity: 0n }, { principal: 1n, allowed: 0n, specificity: 2n }, { principal: 2n, allowed: 1n, specificity: 0n }, { principal: 2n, allowed: 1n, specificity: 0n }, { principal: 2n, allowed: 0n, specificity: 0n }, { principal: 2n, allowed: 1n, specificity: 3n }];
    const actual: Report = solve(rows, 4n);
    if (actual.allowed !== 1n || actual.denied !== 3n || actual.defaulted !== 2n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ principal: 1n, allowed: 1n, specificity: 2n }, { principal: 4n, allowed: 1n, specificity: 3n }, { principal: 0n, allowed: 0n, specificity: 3n }, { principal: 0n, allowed: 0n, specificity: 3n }, { principal: 0n, allowed: 1n, specificity: 1n }, { principal: 1n, allowed: 0n, specificity: 2n }, { principal: 2n, allowed: 0n, specificity: 3n }];
    const actual: Report = solve(rows, 5n);
    if (actual.allowed !== 1n || actual.denied !== 4n || actual.defaulted !== 1n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ principal: 3n, allowed: 0n, specificity: 0n }, { principal: 4n, allowed: 1n, specificity: 2n }, { principal: 2n, allowed: 1n, specificity: 0n }, { principal: 4n, allowed: 1n, specificity: 0n }, { principal: 3n, allowed: 0n, specificity: 2n }, { principal: 4n, allowed: 1n, specificity: 0n }, { principal: 4n, allowed: 1n, specificity: 0n }, { principal: 4n, allowed: 0n, specificity: 2n }];
    const actual: Report = solve(rows, 6n);
    if (actual.allowed !== 1n || actual.denied !== 5n || actual.defaulted !== 3n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ principal: 0n, allowed: 0n, specificity: 1n }, { principal: 0n, allowed: 1n, specificity: 1n }];
    const actual: Report = solve(rows, 1n);
    if (actual.allowed !== 1n || actual.denied !== 0n || actual.defaulted !== 0n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ principal: 0n, allowed: 1n, specificity: 2n }, { principal: 0n, allowed: 0n, specificity: 1n }, { principal: 4n, allowed: 1n, specificity: 8n }];
    const actual: Report = solve(rows, 2n);
    if (actual.allowed !== 1n || actual.denied !== 1n || actual.defaulted !== 1n) throw new Error("fixture 25");
}
{
    const rows: readonly Entry[] = [{ principal: 0n, allowed: 1n, specificity: 2n }, { principal: 0n, allowed: 0n, specificity: 1n }, { principal: 1n, allowed: 1n, specificity: 3n }, { principal: 1n, allowed: 0n, specificity: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.allowed !== 1n || actual.denied !== 2n || actual.defaulted !== 1n) throw new Error("fixture 26");
}
