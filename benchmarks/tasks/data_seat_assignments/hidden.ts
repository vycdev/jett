import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 0n);
    if (actual.assigned !== 0n || actual.rejected !== 0n || actual.seat_checksum !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 5n);
    if (actual.assigned !== 0n || actual.rejected !== 0n || actual.seat_checksum !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 1n);
    if (actual.assigned !== 0n || actual.rejected !== 0n || actual.seat_checksum !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ passenger: 3n, preferred: 8n }];
    const actual: Report = solve(rows, 2n);
    if (actual.assigned !== 1n || actual.rejected !== 0n || actual.seat_checksum !== 4n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ passenger: 1n, preferred: 6n }, { passenger: 3n, preferred: 7n }];
    const actual: Report = solve(rows, 3n);
    if (actual.assigned !== 2n || actual.rejected !== 0n || actual.seat_checksum !== 10n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ passenger: 2n, preferred: 11n }, { passenger: 0n, preferred: 0n }, { passenger: 0n, preferred: (-2n) }];
    const actual: Report = solve(rows, 4n);
    if (actual.assigned !== 2n || actual.rejected !== 1n || actual.seat_checksum !== 5n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ passenger: 3n, preferred: 10n }, { passenger: 2n, preferred: 2n }, { passenger: 4n, preferred: 0n }, { passenger: 0n, preferred: 11n }];
    const actual: Report = solve(rows, 5n);
    if (actual.assigned !== 4n || actual.rejected !== 0n || actual.seat_checksum !== 29n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ passenger: 3n, preferred: 10n }, { passenger: 0n, preferred: 5n }, { passenger: 4n, preferred: 2n }, { passenger: 1n, preferred: 3n }, { passenger: 1n, preferred: 1n }];
    const actual: Report = solve(rows, 6n);
    if (actual.assigned !== 4n || actual.rejected !== 1n || actual.seat_checksum !== 25n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ passenger: 3n, preferred: 5n }, { passenger: 1n, preferred: (-2n) }, { passenger: 3n, preferred: 11n }, { passenger: 4n, preferred: 2n }, { passenger: 0n, preferred: 10n }, { passenger: 3n, preferred: 6n }];
    const actual: Report = solve(rows, 7n);
    if (actual.assigned !== 4n || actual.rejected !== 2n || actual.seat_checksum !== 35n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ passenger: 1n, preferred: 5n }, { passenger: 1n, preferred: 1n }, { passenger: 3n, preferred: (-1n) }, { passenger: 0n, preferred: 8n }, { passenger: 3n, preferred: 7n }, { passenger: 0n, preferred: 5n }, { passenger: 1n, preferred: 11n }];
    const actual: Report = solve(rows, 8n);
    if (actual.assigned !== 3n || actual.rejected !== 4n || actual.seat_checksum !== 22n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ passenger: 0n, preferred: (-3n) }, { passenger: 1n, preferred: 7n }, { passenger: 1n, preferred: 7n }, { passenger: 3n, preferred: (-2n) }, { passenger: 4n, preferred: (-1n) }, { passenger: 0n, preferred: 4n }, { passenger: 2n, preferred: 8n }, { passenger: 1n, preferred: (-2n) }];
    const actual: Report = solve(rows, 1n);
    if (actual.assigned !== 1n || actual.rejected !== 7n || actual.seat_checksum !== 1n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ passenger: 2n, preferred: 10n }, { passenger: 2n, preferred: 11n }, { passenger: 4n, preferred: 2n }, { passenger: 3n, preferred: 1n }, { passenger: 2n, preferred: 1n }, { passenger: 2n, preferred: 4n }, { passenger: 0n, preferred: 2n }, { passenger: 3n, preferred: (-3n) }, { passenger: 4n, preferred: 2n }];
    const actual: Report = solve(rows, 2n);
    if (actual.assigned !== 2n || actual.rejected !== 7n || actual.seat_checksum !== 13n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ passenger: 0n, preferred: 7n }, { passenger: 3n, preferred: (-3n) }, { passenger: 1n, preferred: 5n }, { passenger: 0n, preferred: 10n }, { passenger: 1n, preferred: 8n }, { passenger: 3n, preferred: 6n }, { passenger: 0n, preferred: 2n }, { passenger: 4n, preferred: 5n }, { passenger: 2n, preferred: (-1n) }, { passenger: 0n, preferred: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.assigned !== 3n || actual.rejected !== 7n || actual.seat_checksum !== 15n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ passenger: 2n, preferred: 5n }, { passenger: 4n, preferred: 3n }, { passenger: 3n, preferred: 11n }, { passenger: 1n, preferred: 8n }, { passenger: 3n, preferred: 4n }, { passenger: 0n, preferred: 1n }, { passenger: 4n, preferred: 3n }, { passenger: 1n, preferred: 5n }, { passenger: 4n, preferred: 6n }, { passenger: 1n, preferred: 9n }, { passenger: 4n, preferred: 7n }];
    const actual: Report = solve(rows, 4n);
    if (actual.assigned !== 4n || actual.rejected !== 7n || actual.seat_checksum !== 34n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ passenger: 1n, preferred: 3n }, { passenger: 1n, preferred: 7n }, { passenger: 3n, preferred: 4n }, { passenger: 2n, preferred: 6n }, { passenger: 2n, preferred: 11n }, { passenger: 1n, preferred: 0n }, { passenger: 0n, preferred: 2n }, { passenger: 1n, preferred: 10n }, { passenger: 1n, preferred: (-1n) }, { passenger: 0n, preferred: (-1n) }, { passenger: 3n, preferred: 11n }, { passenger: 3n, preferred: (-3n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.assigned !== 4n || actual.rejected !== 8n || actual.seat_checksum !== 27n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows, 6n);
    if (actual.assigned !== 0n || actual.rejected !== 0n || actual.seat_checksum !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ passenger: 3n, preferred: (-1n) }];
    const actual: Report = solve(rows, 7n);
    if (actual.assigned !== 1n || actual.rejected !== 0n || actual.seat_checksum !== 4n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ passenger: 0n, preferred: 4n }, { passenger: 4n, preferred: 8n }];
    const actual: Report = solve(rows, 8n);
    if (actual.assigned !== 2n || actual.rejected !== 0n || actual.seat_checksum !== 44n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ passenger: 3n, preferred: 7n }, { passenger: 0n, preferred: (-1n) }, { passenger: 0n, preferred: 3n }];
    const actual: Report = solve(rows, 1n);
    if (actual.assigned !== 1n || actual.rejected !== 2n || actual.seat_checksum !== 4n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ passenger: 1n, preferred: 11n }, { passenger: 0n, preferred: 5n }, { passenger: 3n, preferred: 4n }, { passenger: 0n, preferred: 4n }];
    const actual: Report = solve(rows, 2n);
    if (actual.assigned !== 2n || actual.rejected !== 2n || actual.seat_checksum !== 4n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ passenger: 1n, preferred: 1n }, { passenger: 2n, preferred: 5n }, { passenger: 4n, preferred: (-3n) }, { passenger: 2n, preferred: 5n }, { passenger: 1n, preferred: (-3n) }];
    const actual: Report = solve(rows, 3n);
    if (actual.assigned !== 3n || actual.rejected !== 2n || actual.seat_checksum !== 23n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ passenger: 0n, preferred: 4n }, { passenger: 2n, preferred: 1n }, { passenger: 3n, preferred: 10n }, { passenger: 4n, preferred: (-3n) }, { passenger: 2n, preferred: 4n }, { passenger: 1n, preferred: 4n }];
    const actual: Report = solve(rows, 4n);
    if (actual.assigned !== 4n || actual.rejected !== 2n || actual.seat_checksum !== 30n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ passenger: 2n, preferred: 1n }, { passenger: 0n, preferred: 6n }, { passenger: 1n, preferred: 6n }, { passenger: 0n, preferred: 0n }, { passenger: 1n, preferred: 0n }, { passenger: 2n, preferred: 11n }, { passenger: 4n, preferred: (-1n) }];
    const actual: Report = solve(rows, 5n);
    if (actual.assigned !== 4n || actual.rejected !== 3n || actual.seat_checksum !== 31n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ passenger: 4n, preferred: 6n }, { passenger: 4n, preferred: (-1n) }, { passenger: 0n, preferred: (-3n) }, { passenger: 2n, preferred: 9n }, { passenger: 0n, preferred: (-1n) }, { passenger: 0n, preferred: 2n }, { passenger: 1n, preferred: (-2n) }, { passenger: 1n, preferred: (-1n) }];
    const actual: Report = solve(rows, 6n);
    if (actual.assigned !== 4n || actual.rejected !== 4n || actual.seat_checksum !== 43n) throw new Error("fixture 23");
}
{
    const rows: readonly Entry[] = [{ passenger: 0n, preferred: 0n }, { passenger: 1n, preferred: (-1n) }, { passenger: 2n, preferred: 9n }, { passenger: 0n, preferred: 3n }];
    const actual: Report = solve(rows, 3n);
    if (actual.assigned !== 3n || actual.rejected !== 1n || actual.seat_checksum !== 14n) throw new Error("fixture 24");
}
{
    const rows: readonly Entry[] = [{ passenger: 2n, preferred: 2n }, { passenger: 3n, preferred: 2n }, { passenger: 2n, preferred: 3n }, { passenger: 4n, preferred: 9n }, { passenger: 5n, preferred: 1n }];
    const actual: Report = solve(rows, 3n);
    if (actual.assigned !== 3n || actual.rejected !== 2n || actual.seat_checksum !== 25n) throw new Error("fixture 25");
}
