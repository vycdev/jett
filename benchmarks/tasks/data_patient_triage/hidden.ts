import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.selected !== 0n || actual.patient_checksum !== 0n || actual.severity_sum !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 0n || actual.patient_checksum !== 0n || actual.severity_sum !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 0n || actual.patient_checksum !== 0n || actual.severity_sum !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ patient: 3n, severity: 8n, arrival: 3n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 1n || actual.patient_checksum !== 4n || actual.severity_sum !== 8n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ patient: 1n, severity: 6n, arrival: 3n }, { patient: 3n, severity: 7n, arrival: 5n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 2n || actual.patient_checksum !== 8n || actual.severity_sum !== 13n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ patient: 2n, severity: 11n, arrival: 1n }, { patient: 0n, severity: 0n, arrival: 6n }, { patient: 0n, severity: (-2n), arrival: 5n }];
    const actual: Report = solve(rows, 4n);
    if (actual.selected !== 3n || actual.patient_checksum !== 8n || actual.severity_sum !== 9n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ patient: 3n, severity: 10n, arrival: 2n }, { patient: 2n, severity: 2n, arrival: 3n }, { patient: 4n, severity: 0n, arrival: 4n }, { patient: 0n, severity: 11n, arrival: 2n }];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 4n || actual.patient_checksum !== 38n || actual.severity_sum !== 23n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ patient: 3n, severity: 10n, arrival: 3n }, { patient: 0n, severity: 5n, arrival: 5n }, { patient: 4n, severity: 2n, arrival: 6n }, { patient: 1n, severity: 3n, arrival: 1n }, { patient: 1n, severity: 1n, arrival: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.selected !== 5n || actual.patient_checksum !== 42n || actual.severity_sum !== 21n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ patient: 3n, severity: 5n, arrival: 6n }, { patient: 1n, severity: (-2n), arrival: 5n }, { patient: 3n, severity: 11n, arrival: 6n }, { patient: 4n, severity: 2n, arrival: 4n }, { patient: 0n, severity: 10n, arrival: 6n }, { patient: 3n, severity: 6n, arrival: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.selected !== 6n || actual.patient_checksum !== 71n || actual.severity_sum !== 32n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ patient: 1n, severity: 5n, arrival: 1n }, { patient: 1n, severity: 1n, arrival: 1n }, { patient: 3n, severity: (-1n), arrival: 1n }, { patient: 0n, severity: 8n, arrival: 4n }, { patient: 3n, severity: 7n, arrival: 4n }, { patient: 0n, severity: 5n, arrival: 4n }, { patient: 1n, severity: 11n, arrival: 3n }];
    const actual: Report = solve(rows, 8n);
    if (actual.selected !== 7n || actual.patient_checksum !== 69n || actual.severity_sum !== 36n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ patient: 0n, severity: (-3n), arrival: 3n }, { patient: 1n, severity: 7n, arrival: 4n }, { patient: 1n, severity: 7n, arrival: 1n }, { patient: 3n, severity: (-2n), arrival: 3n }, { patient: 4n, severity: (-1n), arrival: 4n }, { patient: 0n, severity: 4n, arrival: 5n }, { patient: 2n, severity: 8n, arrival: 4n }, { patient: 1n, severity: (-2n), arrival: 6n }];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 1n || actual.patient_checksum !== 3n || actual.severity_sum !== 8n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ patient: 2n, severity: 10n, arrival: 2n }, { patient: 2n, severity: 11n, arrival: 2n }, { patient: 4n, severity: 2n, arrival: 2n }, { patient: 3n, severity: 1n, arrival: 2n }, { patient: 2n, severity: 1n, arrival: 5n }, { patient: 2n, severity: 4n, arrival: 2n }, { patient: 0n, severity: 2n, arrival: 1n }, { patient: 3n, severity: (-3n), arrival: 4n }, { patient: 4n, severity: 2n, arrival: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.patient_checksum !== 9n || actual.severity_sum !== 21n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ patient: 0n, severity: 7n, arrival: 1n }, { patient: 3n, severity: (-3n), arrival: 1n }, { patient: 1n, severity: 5n, arrival: 6n }, { patient: 0n, severity: 10n, arrival: 1n }, { patient: 1n, severity: 8n, arrival: 3n }, { patient: 3n, severity: 6n, arrival: 3n }, { patient: 0n, severity: 2n, arrival: 3n }, { patient: 4n, severity: 5n, arrival: 3n }, { patient: 2n, severity: (-1n), arrival: 2n }, { patient: 0n, severity: 1n, arrival: 2n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 3n || actual.patient_checksum !== 8n || actual.severity_sum !== 25n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ patient: 2n, severity: 5n, arrival: 3n }, { patient: 4n, severity: 3n, arrival: 6n }, { patient: 3n, severity: 11n, arrival: 1n }, { patient: 1n, severity: 8n, arrival: 6n }, { patient: 3n, severity: 4n, arrival: 2n }, { patient: 0n, severity: 1n, arrival: 1n }, { patient: 4n, severity: 3n, arrival: 2n }, { patient: 1n, severity: 5n, arrival: 6n }, { patient: 4n, severity: 6n, arrival: 5n }, { patient: 1n, severity: 9n, arrival: 5n }, { patient: 4n, severity: 7n, arrival: 2n }];
    const actual: Report = solve(rows, 4n);
    if (actual.selected !== 4n || actual.patient_checksum !== 34n || actual.severity_sum !== 35n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ patient: 1n, severity: 3n, arrival: 2n }, { patient: 1n, severity: 7n, arrival: 2n }, { patient: 3n, severity: 4n, arrival: 6n }, { patient: 2n, severity: 6n, arrival: 3n }, { patient: 2n, severity: 11n, arrival: 3n }, { patient: 1n, severity: 0n, arrival: 5n }, { patient: 0n, severity: 2n, arrival: 2n }, { patient: 1n, severity: 10n, arrival: 3n }, { patient: 1n, severity: (-1n), arrival: 5n }, { patient: 0n, severity: (-1n), arrival: 1n }, { patient: 3n, severity: 11n, arrival: 6n }, { patient: 3n, severity: (-3n), arrival: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 5n || actual.patient_checksum !== 40n || actual.severity_sum !== 45n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.selected !== 0n || actual.patient_checksum !== 0n || actual.severity_sum !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ patient: 3n, severity: (-1n), arrival: 5n }];
    const actual: Report = solve(rows, 7n);
    if (actual.selected !== 1n || actual.patient_checksum !== 4n || actual.severity_sum !== -1n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ patient: 0n, severity: 4n, arrival: 1n }, { patient: 4n, severity: 8n, arrival: 4n }];
    const actual: Report = solve(rows, 8n);
    if (actual.selected !== 2n || actual.patient_checksum !== 7n || actual.severity_sum !== 12n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ patient: 3n, severity: 7n, arrival: 3n }, { patient: 0n, severity: (-1n), arrival: 4n }, { patient: 0n, severity: 3n, arrival: 4n }];
    const actual: Report = solve(rows, 1n);
    if (actual.selected !== 1n || actual.patient_checksum !== 4n || actual.severity_sum !== 7n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ patient: 1n, severity: 11n, arrival: 3n }, { patient: 0n, severity: 5n, arrival: 2n }, { patient: 3n, severity: 4n, arrival: 3n }, { patient: 0n, severity: 4n, arrival: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.patient_checksum !== 4n || actual.severity_sum !== 16n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ patient: 1n, severity: 1n, arrival: 5n }, { patient: 2n, severity: 5n, arrival: 4n }, { patient: 4n, severity: (-3n), arrival: 3n }, { patient: 2n, severity: 5n, arrival: 5n }, { patient: 1n, severity: (-3n), arrival: 6n }];
    const actual: Report = solve(rows, 3n);
    if (actual.selected !== 3n || actual.patient_checksum !== 15n || actual.severity_sum !== 11n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ patient: 0n, severity: 4n, arrival: 4n }, { patient: 2n, severity: 1n, arrival: 6n }, { patient: 3n, severity: 10n, arrival: 1n }, { patient: 4n, severity: (-3n), arrival: 1n }, { patient: 2n, severity: 4n, arrival: 1n }, { patient: 1n, severity: 4n, arrival: 6n }];
    const actual: Report = solve(rows, 4n);
    if (actual.selected !== 4n || actual.patient_checksum !== 21n || actual.severity_sum !== 22n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ patient: 2n, severity: 1n, arrival: 6n }, { patient: 0n, severity: 6n, arrival: 5n }, { patient: 1n, severity: 6n, arrival: 5n }, { patient: 0n, severity: 0n, arrival: 5n }, { patient: 1n, severity: 0n, arrival: 2n }, { patient: 2n, severity: 11n, arrival: 1n }, { patient: 4n, severity: (-1n), arrival: 4n }];
    const actual: Report = solve(rows, 5n);
    if (actual.selected !== 5n || actual.patient_checksum !== 33n || actual.severity_sum !== 24n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ patient: 4n, severity: 6n, arrival: 1n }, { patient: 4n, severity: (-1n), arrival: 1n }, { patient: 0n, severity: (-3n), arrival: 4n }, { patient: 2n, severity: 9n, arrival: 5n }, { patient: 0n, severity: (-1n), arrival: 2n }, { patient: 0n, severity: 2n, arrival: 2n }, { patient: 1n, severity: (-2n), arrival: 3n }, { patient: 1n, severity: (-1n), arrival: 6n }];
    const actual: Report = solve(rows, 6n);
    if (actual.selected !== 6n || actual.patient_checksum !== 53n || actual.severity_sum !== 14n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ patient: 8n, severity: 5n, arrival: 2n }, { patient: 1n, severity: 5n, arrival: 2n }, { patient: 3n, severity: 5n, arrival: 1n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.patient_checksum !== 22n || actual.severity_sum !== 10n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ patient: 9n, severity: 2n, arrival: 1n }, { patient: 3n, severity: 5n, arrival: 4n }, { patient: 4n, severity: 5n, arrival: 4n }, { patient: 8n, severity: 5n, arrival: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.selected !== 2n || actual.patient_checksum !== 17n || actual.severity_sum !== 10n) throw new Error("fixture 25");
}
