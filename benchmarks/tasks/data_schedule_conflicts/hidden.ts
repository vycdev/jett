import { Entry, Report, solve } from "./solution.js";

{
    const rows: readonly Entry[] = [];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 0");
}
{
    const rows: readonly Entry[] = [{ start: 11n, end: 15n, resource_id: 3n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 1");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n, resource_id: 3n }, { start: 6n, end: 12n, resource_id: 5n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 2");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 13n, resource_id: 1n }, { start: 0n, end: 2n, resource_id: 1n }, { start: 1n, end: 6n, resource_id: 4n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 3");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 6n, resource_id: 3n }, { start: 5n, end: 10n, resource_id: 2n }, { start: 7n, end: 8n, resource_id: 2n }, { start: 6n, end: 13n, resource_id: 3n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 1n) throw new Error("fixture 4");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 5n, resource_id: 5n }, { start: 9n, end: 12n, resource_id: 2n }, { start: 6n, end: 7n, resource_id: 2n }, { start: 4n, end: 5n, resource_id: 4n }, { start: 8n, end: 15n, resource_id: 2n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 3n) throw new Error("fixture 5");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 8n, resource_id: 5n }, { start: 7n, end: 13n, resource_id: 5n }, { start: 5n, end: 9n, resource_id: 1n }, { start: 10n, end: 14n, resource_id: 5n }, { start: 9n, end: 11n, resource_id: 5n }, { start: 0n, end: 2n, resource_id: 3n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 4n || actual.affected_bookings !== 4n || actual.longest_overlap !== 3n) throw new Error("fixture 6");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 5n, resource_id: 2n }, { start: 0n, end: 1n, resource_id: 4n }, { start: 6n, end: 12n, resource_id: 4n }, { start: 11n, end: 12n, resource_id: 5n }, { start: 7n, end: 14n, resource_id: 2n }, { start: 5n, end: 12n, resource_id: 1n }, { start: 0n, end: 3n, resource_id: 2n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 2n) throw new Error("fixture 7");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 14n, resource_id: 2n }, { start: 10n, end: 17n, resource_id: 1n }, { start: 7n, end: 8n, resource_id: 3n }, { start: 9n, end: 11n, resource_id: 4n }, { start: 11n, end: 12n, resource_id: 4n }, { start: 9n, end: 12n, resource_id: 4n }, { start: 2n, end: 3n, resource_id: 3n }, { start: 3n, end: 9n, resource_id: 3n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 3n || actual.affected_bookings !== 5n || actual.longest_overlap !== 2n) throw new Error("fixture 8");
}
{
    const rows: readonly Entry[] = [{ start: 2n, end: 9n, resource_id: 5n }, { start: 5n, end: 7n, resource_id: 4n }, { start: 4n, end: 6n, resource_id: 3n }, { start: 4n, end: 11n, resource_id: 5n }, { start: 5n, end: 9n, resource_id: 2n }, { start: 10n, end: 11n, resource_id: 3n }, { start: 0n, end: 4n, resource_id: 1n }, { start: 6n, end: 11n, resource_id: 3n }, { start: 3n, end: 4n, resource_id: 1n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 3n || actual.affected_bookings !== 6n || actual.longest_overlap !== 5n) throw new Error("fixture 9");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 8n, resource_id: 1n }, { start: 2n, end: 7n, resource_id: 1n }, { start: 1n, end: 7n, resource_id: 2n }, { start: 11n, end: 14n, resource_id: 4n }, { start: 9n, end: 12n, resource_id: 1n }, { start: 5n, end: 8n, resource_id: 5n }, { start: 8n, end: 11n, resource_id: 3n }, { start: 2n, end: 4n, resource_id: 1n }, { start: 4n, end: 6n, resource_id: 3n }, { start: 8n, end: 11n, resource_id: 5n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 2n) throw new Error("fixture 10");
}
{
    const rows: readonly Entry[] = [{ start: 6n, end: 12n, resource_id: 4n }, { start: 0n, end: 2n, resource_id: 4n }, { start: 7n, end: 9n, resource_id: 1n }, { start: 4n, end: 5n, resource_id: 5n }, { start: 6n, end: 8n, resource_id: 2n }, { start: 8n, end: 14n, resource_id: 5n }, { start: 9n, end: 14n, resource_id: 2n }, { start: 9n, end: 14n, resource_id: 2n }, { start: 3n, end: 7n, resource_id: 2n }, { start: 2n, end: 8n, resource_id: 2n }, { start: 7n, end: 11n, resource_id: 3n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 4n || actual.affected_bookings !== 5n || actual.longest_overlap !== 5n) throw new Error("fixture 11");
}
{
    const rows: readonly Entry[] = [{ start: 9n, end: 12n, resource_id: 3n }, { start: 5n, end: 12n, resource_id: 2n }, { start: 3n, end: 8n, resource_id: 1n }, { start: 5n, end: 7n, resource_id: 2n }, { start: 4n, end: 6n, resource_id: 2n }, { start: 9n, end: 10n, resource_id: 2n }, { start: 1n, end: 8n, resource_id: 4n }, { start: 11n, end: 15n, resource_id: 1n }, { start: 6n, end: 10n, resource_id: 2n }, { start: 8n, end: 9n, resource_id: 4n }, { start: 1n, end: 6n, resource_id: 4n }, { start: 6n, end: 12n, resource_id: 3n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 9n || actual.affected_bookings !== 9n || actual.longest_overlap !== 5n) throw new Error("fixture 12");
}
{
    const rows: readonly Entry[] = [{ start: 1n, end: 3n, resource_id: 4n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 13");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 4n, resource_id: 4n }, { start: 2n, end: 5n, resource_id: 1n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 14");
}
{
    const rows: readonly Entry[] = [{ start: 8n, end: 10n, resource_id: 4n }, { start: 7n, end: 10n, resource_id: 1n }, { start: 7n, end: 9n, resource_id: 2n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 15");
}
{
    const rows: readonly Entry[] = [{ start: 4n, end: 9n, resource_id: 3n }, { start: 8n, end: 12n, resource_id: 5n }, { start: 0n, end: 3n, resource_id: 3n }, { start: 8n, end: 15n, resource_id: 5n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 4n) throw new Error("fixture 16");
}
{
    const rows: readonly Entry[] = [{ start: 10n, end: 12n, resource_id: 1n }, { start: 10n, end: 11n, resource_id: 4n }, { start: 6n, end: 9n, resource_id: 3n }, { start: 10n, end: 14n, resource_id: 1n }, { start: 8n, end: 9n, resource_id: 1n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 2n) throw new Error("fixture 17");
}
{
    const rows: readonly Entry[] = [{ start: 5n, end: 9n, resource_id: 1n }, { start: 3n, end: 7n, resource_id: 3n }, { start: 4n, end: 10n, resource_id: 1n }, { start: 9n, end: 16n, resource_id: 5n }, { start: 10n, end: 12n, resource_id: 5n }, { start: 9n, end: 16n, resource_id: 1n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 3n || actual.affected_bookings !== 5n || actual.longest_overlap !== 4n) throw new Error("fixture 18");
}
{
    const rows: readonly Entry[] = [{ start: 3n, end: 8n, resource_id: 2n }, { start: 3n, end: 5n, resource_id: 3n }, { start: 0n, end: 6n, resource_id: 5n }, { start: 2n, end: 9n, resource_id: 4n }, { start: 8n, end: 13n, resource_id: 1n }, { start: 8n, end: 10n, resource_id: 1n }, { start: 10n, end: 11n, resource_id: 1n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 2n || actual.affected_bookings !== 3n || actual.longest_overlap !== 2n) throw new Error("fixture 19");
}
{
    const rows: readonly Entry[] = [{ start: 7n, end: 14n, resource_id: 3n }, { start: 8n, end: 15n, resource_id: 1n }, { start: 2n, end: 9n, resource_id: 2n }, { start: 10n, end: 11n, resource_id: 3n }, { start: 3n, end: 5n, resource_id: 1n }, { start: 5n, end: 7n, resource_id: 2n }, { start: 10n, end: 12n, resource_id: 3n }, { start: 4n, end: 9n, resource_id: 4n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 4n || actual.affected_bookings !== 5n || actual.longest_overlap !== 2n) throw new Error("fixture 20");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 2n, resource_id: 1n }, { start: 2n, end: 4n, resource_id: 1n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 0n || actual.affected_bookings !== 0n || actual.longest_overlap !== 0n) throw new Error("fixture 21");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 3n, resource_id: 1n }, { start: 0n, end: 3n, resource_id: 1n }, { start: 0n, end: 3n, resource_id: 2n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 1n || actual.affected_bookings !== 2n || actual.longest_overlap !== 3n) throw new Error("fixture 22");
}
{
    const rows: readonly Entry[] = [{ start: 0n, end: 4n, resource_id: 1n }, { start: 4n, end: 9n, resource_id: 1n }, { start: 2n, end: 6n, resource_id: 1n }, { start: 0n, end: 8n, resource_id: 2n }];
    const actual: Report = solve(rows);
    if (actual.conflict_pairs !== 2n || actual.affected_bookings !== 3n || actual.longest_overlap !== 2n) throw new Error("fixture 23");
}
