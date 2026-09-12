"""Domain specifications for 30 structured-data evaluation problems.

Algorithm bodies are deliberately readable typed Python, translated mechanically
by bench_data_generate. Hand-computed anchors supplement deterministic cases.
"""
from __future__ import annotations

import random
import re
from typing import Any

SPECS: list[dict[str, Any]] = []


def cases(kind: str) -> list[tuple[list[list[int]], int]]:
    rng = random.Random(76123)
    samples: list[tuple[list[list[int]], int]] = [([], 0), ([], 5)]
    for index in range(22):
        limit = 1 + index % 8
        size = index % 13
        rows: list[list[int]] = []
        for item in range(size):
            if kind == "graph":
                rows.append([rng.randrange(limit), rng.randrange(limit), 1 + rng.randrange(7)])
            elif kind == "dag":
                if limit > 1:
                    start = rng.randrange(limit - 1)
                    rows.append([start, rng.randrange(start + 1, limit), 1 + rng.randrange(7)])
            elif kind == "tree":
                if item < limit:
                    rows.append([item + 1, rng.randrange(item + 1), rng.randrange(-8, 12)])
            elif kind == "interval":
                start = rng.randrange(12)
                rows.append([start, start + 1 + rng.randrange(7), 1 + rng.randrange(5)])
            elif kind == "deadline":
                rows.append([1 + rng.randrange(8), rng.randrange(30), 1 + rng.randrange(4)])
            elif kind == "events":
                rows.append([rng.randrange(5), rng.randrange(4), rng.randrange(-2, 10)])
            elif kind == "records":
                rows.append([rng.randrange(5), rng.randrange(-3, 12), rng.randrange(1, 7)])
            elif kind == "permissions":
                rows.append([rng.randrange(5), rng.randrange(2), rng.randrange(4)])
            else:
                raise ValueError(kind)
        if kind == "tree":
            rng.shuffle(rows)
        samples.append((rows, limit))
    return samples


def add(task_id: str, title: str, category: str, fields: str, outputs: str,
        statement: str, domain: str, code: str, kind: str,
        anchors: list[tuple[list[list[int]], int, list[int]]]) -> None:
    SPECS.append(dict(id="data_" + task_id, title=title, category=category,
                     fields=fields, outputs=outputs, statement=statement,
                     domain=domain, code=code, cases=cases(kind), anchors=anchors))


add("ledger_audit", "Audit inter-account ledger entries", "state",
    "account delta reference", "total lowest_balance first_overdraw",
    "Process ledger records in order. Each account begins with balance limit. Add delta to its balance, even if this makes it negative. Report total as the sum of final balances of distinct accounts that appear, lowest_balance as the minimum balance observed after any entry (or limit when no entries), and first_overdraw as the zero-based index of the first entry producing a negative balance, or -1. reference is an audit identifier and does not affect balances.",
    "account is nonnegative; delta can be negative; repeated references are allowed and are not deduplicated.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    balances: dict[int, int] = {}
    seen: dict[int, int] = {}
    total: int = 0
    lowest: int = limit
    first: int = -1
    index: int = 0
    for row in rows:
        if read(seen, row.account, 0) == 0:
            total = total + limit
            seen = put(seen, row.account, 1)
        balance: int = read(balances, row.account, limit) + row.delta
        balances = put(balances, row.account, balance)
        total = total + row.delta
        if index == 0 or balance < lowest:
            lowest = balance
        if balance < 0 and first == -1:
            first = index
        index = index + 1
    return Report(total, lowest, first)
''', "records", [([[2, -7, 1], [2, 9, 1], [3, -1, 2]], 5, [11, -2, 0])])

add("reservation_gate", "Accept bounded reservations independently", "validation",
    "party seats priority", "accepted rejected occupied",
    "The venue has limit seats. Consider requests in order. Reject a request if seats <= 0 or its party already has an accepted request or it would exceed capacity. Otherwise accept it and permanently reserve its seats. A rejected request does not reserve its party ID, so a later request by that party may succeed. Return counts accepted/rejected and occupied seats. priority is informational and does not reorder requests.",
    "party is nonnegative; seats may be negative; priority is nonnegative.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    parties: dict[int, int] = {}
    accepted: int = 0
    rejected: int = 0
    occupied: int = 0
    for row in rows:
        invalid: bool = row.seats <= 0 or read(parties, row.party, 0) == 1
        if invalid or occupied + row.seats > limit:
            rejected = rejected + 1
        else:
            accepted = accepted + 1
            occupied = occupied + row.seats
            parties = put(parties, row.party, 1)
    return Report(accepted, rejected, occupied)
''', "records", [([[1, 6, 0], [1, 3, 2], [1, 1, 1], [2, 2, 0]], 5, [2, 2, 5])])

add("sensor_sessions", "Fold per-sensor open and close sessions", "state",
    "sensor operation timestamp", "completed active rejected",
    "Process records in input order. operation 0 opens an inactive sensor at timestamp; operation 1 closes an active sensor if timestamp is at least its opening time. All other operations, negative timestamps, duplicate opens and unmatched/backdated closes are rejected without changing state. Return completed closes, number of still-active sensors, and rejected records. limit is reserved and has no effect.",
    "sensor is nonnegative; operation is an integer (unknown values are rejected); timestamp may be negative.",
    '''def step(operation: int, timestamp: int, opened: int) -> int:
    if timestamp < 0:
        return -2
    if operation == 0 and opened == -1:
        return timestamp
    if operation == 1 and opened >= 0 and timestamp >= opened:
        return -1
    return -2

def solve(rows: list[Entry], limit: int) -> Report:
    opened: dict[int, int] = {}
    completed: int = 0
    active: int = 0
    rejected: int = 0
    for row in rows:
        prior: int = read(opened, row.sensor, -1)
        next_time: int = step(row.operation, row.timestamp, prior)
        if next_time == -2:
            rejected = rejected + 1
        else:
            opened = put(opened, row.sensor, next_time)
            if next_time == -1:
                completed = completed + 1
                active = active - 1
            else:
                active = active + 1
    return Report(completed, active, rejected)
''', "events", [([[1, 0, 2], [1, 1, 1], [1, 1, 3], [2, 0, 0]], 0, [1, 1, 1])])

add("job_readiness", "Resolve jobs whose prerequisite chains are present", "graph",
    "job prerequisite cost", "ready blocked total_cost",
    "Each row declares a job with one prerequisite; prerequisite 0 means none. A job is ready if repeatedly following its prerequisite reaches 0 and every referenced job exists. Missing prerequisites block the whole dependent chain. Report ready and blocked job counts and the sum of costs of ready jobs. limit is reserved. Input order is arbitrary.",
    "job IDs are unique positive integers; 0 <= prerequisite < job, so chains are acyclic. Costs may be negative. At most 30 jobs.",
    '''def ready_job(rows: list[Entry], job: int) -> bool:
    current: int = job
    while current != 0:
        parent: int = -1
        for row in rows:
            if row.job == current:
                parent = row.prerequisite
        if parent == -1:
            return False
        current = parent
    return True

def solve(rows: list[Entry], limit: int) -> Report:
    ready: int = 0
    blocked: int = 0
    total: int = 0
    for row in rows:
        if ready_job(rows, row.job):
            ready = ready + 1
            total = total + row.cost
        else:
            blocked = blocked + 1
    return Report(ready, blocked, total)
''', "tree", [([[3, 2, 5], [1, 0, 2], [5, 3, 9]], 0, [1, 2, 2])])

add("graph_reachability", "Summarize directed reachability from vertex zero", "graph",
    "source target label", "reachable unreachable reachable_edge_count",
    "Vertices are the IDs 0 through limit-1. Starting at vertex 0 when present, find all vertices reachable through directed edges. Count reachable and unreachable vertices; reachable_edge_count counts input edge records whose source is reachable (including duplicate records and self-loops). label does not affect reachability.",
    "0 <= source,target < limit; when limit is zero the edge list is empty. Cycles, self-loops, and duplicate edges are allowed.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    seen: dict[int, int] = {}
    if limit > 0:
        seen = put(seen, 0, 1)
    turn: int = 0
    while turn < limit:
        for row in rows:
            if read(seen, row.source, 0) == 1:
                seen = put(seen, row.target, 1)
        turn = turn + 1
    reachable: int = 0
    vertex: int = 0
    while vertex < limit:
        reachable = reachable + read(seen, vertex, 0)
        vertex = vertex + 1
    edges: int = 0
    for row in rows:
        edges = edges + read(seen, row.source, 0)
    return Report(reachable, limit - reachable, edges)
''', "graph", [([[0, 1, 2], [1, 0, 5], [3, 4, 1], [1, 1, 9]], 5, [2, 3, 3])])

add("graph_shortest_hops", "Measure shortest directed hop distances", "graph",
    "source target weight", "distance_sum farthest unreachable",
    "From vertex 0, compute the shortest number of edges to each reachable vertex. Return their distance sum, largest distance, and count of unreachable vertices. For an empty graph, all output fields are zero. weight is intentionally ignored: every edge costs one hop.",
    "Vertices are 0..limit-1; endpoints are valid. limit=0 implies no edges. Directed cycles, duplicates and self-loops are allowed.",
    '''def relax(rows: list[Entry], distance: dict[int, int]) -> dict[int, int]:
    updated: dict[int, int] = {}
    for row in rows:
        prior: int = read(distance, row.source, 1000)
        old: int = read(distance, row.target, 1000)
        best: int = read(updated, row.target, old)
        if prior + 1 < best:
            updated = put(updated, row.target, prior + 1)
    return updated

def solve(rows: list[Entry], limit: int) -> Report:
    distance: dict[int, int] = {}
    distance = put(distance, 0, 0)
    turn: int = 0
    while turn < limit:
        updated: dict[int, int] = relax(rows, distance)
        vertex: int = 0
        while vertex < limit:
            distance = put(distance, vertex, read(updated, vertex, read(distance, vertex, 1000)))
            vertex = vertex + 1
        turn = turn + 1
    total: int = 0
    farthest: int = 0
    missing: int = 0
    vertex: int = 0
    while vertex < limit:
        hops: int = read(distance, vertex, 1000)
        if hops == 1000:
            missing = missing + 1
        else:
            total = total + hops
            if hops > farthest:
                farthest = hops
        vertex = vertex + 1
    return Report(total, farthest, missing)
''', "graph", [([[0, 2, 9], [0, 1, 2], [1, 2, 1], [2, 3, 1]], 5, [4, 2, 1])])

add("graph_components", "Count undirected components including isolated vertices", "graph",
    "source target tag", "components largest isolated",
    "Interpret every input edge as undirected. Return the number of connected components, size of the largest component, and count of isolated vertices (vertices with no incident edge; a self-loop means the vertex is not isolated). tag is informational. Empty vertex set yields all zero.",
    "Vertices are 0..limit-1; endpoints are valid; duplicates, cycles and self-loops are allowed. limit=0 means no edges.",
    '''def spread(rows: list[Entry], seen: dict[int, int], vertex: int) -> int:
    found: int = read(seen, vertex, 0)
    for row in rows:
        if row.source == vertex:
            if read(seen, row.target, 0) == 1:
                found = 1
        if row.target == vertex:
            if read(seen, row.source, 0) == 1:
                found = 1
    return found

def component_size(rows: list[Entry], start: int, limit: int) -> int:
    seen: dict[int, int] = {}
    seen = put(seen, start, 1)
    turn: int = 0
    while turn < limit:
        vertex: int = 0
        while vertex < limit:
            seen = put(seen, vertex, spread(rows, seen, vertex))
            vertex = vertex + 1
        turn = turn + 1
    count: int = 0
    vertex: int = 0
    while vertex < limit:
        if read(seen, vertex, 0) == 1:
            if vertex < start:
                return 0
            count = count + 1
        vertex = vertex + 1
    return count

def solve(rows: list[Entry], limit: int) -> Report:
    components: int = 0
    largest: int = 0
    isolated: int = 0
    vertex: int = 0
    while vertex < limit:
        count: int = component_size(rows, vertex, limit)
        if count > 0:
            components = components + 1
        if count > largest:
            largest = count
        incident: int = 0
        for row in rows:
            if row.source == vertex or row.target == vertex:
                incident = incident + 1
        if incident == 0:
            isolated = isolated + 1
        vertex = vertex + 1
    return Report(components, largest, isolated)
''', "graph", [([[0, 1, 1], [1, 2, 1], [3, 3, 1]], 5, [3, 3, 1])])

add("graph_degrees", "Audit directed source and sink degrees", "graph",
    "source target label", "sources sinks balanced",
    "Count source vertices with indegree zero and positive outdegree; sink vertices with outdegree zero and positive indegree; and balanced vertices with equal indegree and outdegree (including isolated vertices). Count every edge record, including duplicates, in degrees. Self-loops add one to each degree. label is ignored.",
    "Vertices are 0..limit-1; endpoints valid; empty graph allowed.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    sources: int = 0
    sinks: int = 0
    balanced: int = 0
    vertex: int = 0
    while vertex < limit:
        incoming: int = 0
        outgoing: int = 0
        for row in rows:
            if row.source == vertex:
                outgoing = outgoing + 1
            if row.target == vertex:
                incoming = incoming + 1
        if incoming == 0 and outgoing > 0:
            sources = sources + 1
        if outgoing == 0 and incoming > 0:
            sinks = sinks + 1
        if incoming == outgoing:
            balanced = balanced + 1
        vertex = vertex + 1
    return Report(sources, sinks, balanced)
''', "graph", [([[0, 1, 0], [0, 1, 0], [1, 2, 0], [3, 3, 0]], 5, [1, 1, 2])])

add("tree_depths", "Summarize a rooted forest's depth profile", "tree",
    "node parent weight", "roots max_depth weighted_depth",
    "Each parent=0 row is a root at depth zero. All other nodes have depth one plus their parent's depth. Report number of roots, maximum depth (zero for no nodes), and sum of weight*depth across nodes. Input records may be in any order. limit is reserved.",
    "node IDs are unique positive integers; 0 <= parent < node; every nonzero parent has a row; therefore input is a finite acyclic rooted forest. Weights may be negative.",
    '''def depth_of(rows: list[Entry], node: int) -> int:
    current: int = node
    depth: int = -1
    while current != 0:
        parent: int = 0
        for row in rows:
            if row.node == current:
                parent = row.parent
        current = parent
        depth = depth + 1
    return depth

def solve(rows: list[Entry], limit: int) -> Report:
    roots: int = 0
    maximum: int = 0
    total: int = 0
    for row in rows:
        depth: int = depth_of(rows, row.node)
        if depth == 0:
            roots = roots + 1
        if depth > maximum:
            maximum = depth
        total = total + row.weight * depth
    return Report(roots, maximum, total)
''', "tree", [([[4, 2, 3], [2, 1, 5], [1, 0, 9], [3, 0, 7]], 0, [2, 2, 11])])

add("tree_subtree_weight", "Measure the selected subtree", "tree",
    "node parent weight", "node_count total_weight leaf_count",
    "Select node ID limit. Return the number of nodes in its subtree, sum of their weights, and number of leaves in that subtree. A leaf has no children in the complete input forest. Include the selected node itself. Missing selection (including ID zero) yields all zeros. Input order is arbitrary.",
    "Unique positive node IDs; 0 <= parent < node; all nonzero parents exist, yielding a finite rooted forest. Weights may be negative.",
    '''def belongs(rows: list[Entry], node: int, selected: int) -> bool:
    current: int = node
    while current != 0:
        if current == selected:
            return True
        parent: int = 0
        for row in rows:
            if row.node == current:
                parent = row.parent
        current = parent
    return False

def is_leaf(rows: list[Entry], node: int) -> bool:
    for row in rows:
        if row.parent == node:
            return False
    return True

def solve(rows: list[Entry], limit: int) -> Report:
    count: int = 0
    total: int = 0
    leaves: int = 0
    for row in rows:
        if belongs(rows, row.node, limit):
            count = count + 1
            total = total + row.weight
            if is_leaf(rows, row.node):
                leaves = leaves + 1
    return Report(count, total, leaves)
''', "tree", [([[1, 0, 7], [2, 1, 5], [3, 1, 8], [4, 2, -2]], 2, [2, 3, 1])])

add("tree_lowest_ancestor", "Find a lowest common ancestor and path distances", "tree",
    "node parent weight", "ancestor selected_distance largest_distance",
    "Find the lowest common ancestor of node ID limit and the largest node ID in the forest. Return its ID and the edge distances from it to the selected node and to the largest-ID node. Return (0,-1,-1) if the forest is empty, the selected node is absent, or they belong to different roots. weight is ignored.",
    "Unique positive IDs, 0 <= parent < node, and every nonzero parent exists. Forest is finite and acyclic; rows may be shuffled.",
    '''def parent_of(rows: list[Entry], node: int) -> int:
    for row in rows:
        if row.node == node:
            return row.parent
    return -1

def solve(rows: list[Entry], limit: int) -> Report:
    largest: int = 0
    for row in rows:
        if row.node > largest:
            largest = row.node
    if parent_of(rows, limit) == -1:
        return Report(0, -1, -1)
    selected: int = limit
    selected_distance: int = 0
    while selected > 0:
        other: int = largest
        other_distance: int = 0
        while other > 0:
            if other == selected:
                return Report(selected, selected_distance, other_distance)
            other = parent_of(rows, other)
            other_distance = other_distance + 1
        selected = parent_of(rows, selected)
        selected_distance = selected_distance + 1
    return Report(0, -1, -1)
''', "tree", [([[1, 0, 0], [2, 1, 0], [3, 1, 0], [4, 2, 0]], 3, [1, 1, 2])])

add("schedule_conflicts", "Count intersecting bookings by resource", "schedule",
    "start end resource", "conflict_pairs affected_bookings longest_overlap",
    "Bookings are half-open [start,end). Two distinct bookings conflict only if they have the same resource and their intervals overlap for positive duration. Return the number of unordered conflicting pairs, number of bookings in at least one conflicting pair, and maximum overlap duration among conflicting pairs (zero if none). Duplicate records represent separate bookings. limit is reserved.",
    "0 <= start < end; resource is nonnegative; records may be unsorted.",
    '''def overlap(a_start: int, a_end: int, b_start: int, b_end: int) -> int:
    start: int = a_start
    end: int = a_end
    if b_start > start:
        start = b_start
    if b_end < end:
        end = b_end
    if end > start:
        return end - start
    return 0

def solve(rows: list[Entry], limit: int) -> Report:
    pairs: int = 0
    affected: int = 0
    longest: int = 0
    left_index: int = 0
    for left in rows:
        hit: bool = False
        right_index: int = 0
        for right in rows:
            duration: int = 0
            if left_index != right_index and left.resource == right.resource:
                duration = overlap(left.start, left.end, right.start, right.end)
            if duration > 0:
                hit = True
                if left_index < right_index:
                    pairs = pairs + 1
                if duration > longest:
                    longest = duration
            right_index = right_index + 1
        if hit:
            affected = affected + 1
        left_index = left_index + 1
    return Report(pairs, affected, longest)
''', "interval", [([[0, 4, 1], [4, 9, 1], [2, 6, 1], [0, 8, 2]], 0, [2, 3, 2])])

add("schedule_peak_load", "Find earliest peak concurrent demand", "schedule",
    "start end demand", "peak earliest overloaded_starts",
    "For half-open reservations [start,end), concurrent load is the sum of active demands. Return peak load, earliest input start timestamp attaining that peak, and number of distinct input start timestamps with load greater than capacity limit. If no bookings, return (0,-1,0). Multiple bookings may start together; apply all starts and ends at that timestamp before measuring.",
    "0 <= start < end; demand is positive; unsorted bookings and duplicate timestamps allowed.",
    '''def load_at(rows: list[Entry], time: int) -> int:
    total: int = 0
    for row in rows:
        if row.start <= time and row.end > time:
            total = total + row.demand
    return total

def solve(rows: list[Entry], limit: int) -> Report:
    seen: dict[int, int] = {}
    peak: int = 0
    earliest: int = -1
    overloaded: int = 0
    for row in rows:
        load: int = load_at(rows, row.start)
        if load > peak or load == peak and row.start < earliest:
            peak = load
            earliest = row.start
        if read(seen, row.start, 0) == 0:
            if load > limit:
                overloaded = overloaded + 1
            seen = put(seen, row.start, 1)
    return Report(peak, earliest, overloaded)
''', "interval", [([[4, 8, 2], [0, 4, 3], [2, 6, 4], [2, 3, 1]], 5, [8, 2, 2])])

add("schedule_deadline_audit", "Audit serial job deadlines", "schedule",
    "duration deadline penalty", "late weighted_tardiness finish",
    "Execute jobs serially in input order, starting at time limit, with no idle gaps. Each finishes after its duration. A job is late only when finish > deadline. Return late count, sum of (finish-deadline)*penalty for late jobs, and final finish time. An empty list finishes at limit.",
    "duration and penalty are positive; deadline is nonnegative.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    late: int = 0
    weighted: int = 0
    finish: int = limit
    for row in rows:
        finish = finish + row.duration
        if finish > row.deadline:
            late = late + 1
            weighted = weighted + (finish - row.deadline) * row.penalty
    return Report(late, weighted, finish)
''', "deadline", [([[3, 5, 2], [4, 6, 3], [1, 20, 9]], 2, [1, 9, 10])])

add("schedule_room_reuse", "Assign the lowest available room in booking order", "schedule",
    "start end guest", "rooms assignment_checksum reuses",
    "Bookings arrive in the given order, which is nondecreasing start time. Assign each to the lowest numbered existing room whose previous booking ends <= start; if none exists, create the next room. Room numbers start at 1. Return rooms created, sum of (zero-based booking index+1)*assigned_room, and number of bookings assigned to an existing room. guest does not affect assignment. limit is reserved.",
    "0 <= start < end; records sorted by start with input order breaking ties; guest is nonnegative.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    ends: dict[int, int] = {}
    rooms: int = 0
    checksum: int = 0
    reuses: int = 0
    index: int = 1
    for row in rows:
        selected: int = 0
        room: int = 1
        while room <= rooms:
            if selected == 0 and read(ends, room, 0) <= row.start:
                selected = room
            room = room + 1
        if selected == 0:
            rooms = rooms + 1
            selected = rooms
        else:
            reuses = reuses + 1
        ends = put(ends, selected, row.end)
        checksum = checksum + index * selected
        index = index + 1
    return Report(rooms, checksum, reuses)
''', "interval", [([[0, 5, 1], [1, 3, 2], [3, 6, 3], [5, 7, 4]], 0, [2, 15, 2])])

add("versioned_register", "Apply per-key monotonic version updates", "state",
    "key version value", "accepted stale checksum",
    "Each key initially has version -1 and no value. Accept a row only if version >= 0 and strictly greater than the key's current version, updating its value; otherwise count it as stale. Return accepted/stale counts and sum of (key+1)*final_value over keys that accepted at least one row. Negative values are valid. limit is reserved.",
    "key is nonnegative; versions and values may be negative. Equal versions are stale even when values differ.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    versions: dict[int, int] = {}
    values: dict[int, int] = {}
    accepted: int = 0
    stale: int = 0
    checksum: int = 0
    for row in rows:
        if row.version > read(versions, row.key, -1):
            checksum = checksum + (row.key + 1) * (row.value - read(values, row.key, 0))
            values = put(values, row.key, row.value)
            versions = put(versions, row.key, row.version)
            accepted = accepted + 1
        else:
            stale = stale + 1
    return Report(accepted, stale, checksum)
''', "records", [([[1, 0, 8], [1, 0, 9], [1, 2, -3], [2, -1, 5]], 0, [2, 2, -6])])

add("permission_rules", "Resolve last matching authorization rules", "validation",
    "principal allowed specificity", "allowed denied defaulted",
    "For each principal ID 0..limit-1, choose a rule with greatest specificity among its matching rows; if multiple tie, the last input row wins. allowed=1 grants access and allowed=0 denies. A principal without a matching rule is denied by default. Return allowed count, denied count including default denials, and defaulted count.",
    "principal is nonnegative and may be outside 0..limit-1; allowed is 0 or 1; specificity is nonnegative.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    allowed: int = 0
    denied: int = 0
    defaulted: int = 0
    principal: int = 0
    while principal < limit:
        priority: int = -1
        decision: int = 0
        for row in rows:
            if row.principal == principal and row.specificity >= priority:
                priority = row.specificity
                decision = row.allowed
        if priority == -1:
            defaulted = defaulted + 1
        allowed = allowed + decision
        denied = denied + 1 - decision
        principal = principal + 1
    return Report(allowed, denied, defaulted)
''', "permissions", [([[0, 1, 2], [0, 0, 1], [1, 1, 3], [1, 0, 3]], 3, [1, 2, 1])])

add("restock_projection", "Project minimum replenishment quantities", "records",
    "sku stock daily_demand", "restock_units at_risk worst_shortfall",
    "For each independent SKU row, required stock is daily_demand*limit days. A shortfall is max(required-stock,0). Return sum of shortfalls, count of rows with positive shortfall, and largest shortfall. A negative stock represents backorders and increases shortfall. Distinct rows are separate SKU locations even if IDs repeat.",
    "sku is nonnegative; stock may be negative; daily_demand is positive.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    units: int = 0
    risk: int = 0
    worst: int = 0
    for row in rows:
        shortage: int = row.daily_demand * limit - row.stock
        if shortage > 0:
            units = units + shortage
            risk = risk + 1
            if shortage > worst:
                worst = shortage
    return Report(units, risk, worst)
''', "records", [([[7, 5, 3], [7, 8, 2], [9, -2, 1]], 2, [5, 2, 4])])

add("invoice_reconciliation", "Reconcile invoice balances from signed entries", "records",
    "invoice amount source", "settled outstanding credit",
    "Group all rows by invoice ID and sum amount, where positive amounts are charges and negative amounts are payments. Report number of distinct invoices whose final sum is zero, total positive final balances, and absolute total of negative final balances. source records provenance and is ignored for arithmetic. limit is reserved.",
    "invoice is nonnegative; amount is signed; duplicates are separate transactions, not deduplicated.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    sums: dict[int, int] = {}
    for row in rows:
        sums = put(sums, row.invoice, read(sums, row.invoice, 0) + row.amount)
    seen: dict[int, int] = {}
    settled: int = 0
    outstanding: int = 0
    credit: int = 0
    for row in rows:
        if read(seen, row.invoice, 0) == 0:
            seen = put(seen, row.invoice, 1)
            balance: int = read(sums, row.invoice, 0)
            if balance == 0:
                settled = settled + 1
            if balance > 0:
                outstanding = outstanding + balance
            if balance < 0:
                credit = credit - balance
    return Report(settled, outstanding, credit)
''', "records", [([[1, 5, 0], [2, -4, 0], [1, -5, 1], [3, 7, 0]], 0, [1, 7, 4])])

add("vote_quorum", "Apply latest ballots and test weighted quorum", "state",
    "voter choice weight", "yes_weight no_weight quorum_met",
    "Only each voter's last input ballot counts. Valid ballots have choice 0 (no) or 1 (yes), and weight > 0. If the last ballot is invalid, that voter contributes nothing; do not restore an earlier ballot. Sum final yes/no weights. quorum_met is 1 exactly when yes_weight >= limit and yes_weight > no_weight; otherwise 0.",
    "voter is nonnegative; choice is an integer and may be invalid; weight may be nonpositive.",
    '''def contribution(choice: int, weight: int, wanted: int) -> int:
    if choice == wanted and weight > 0:
        return weight
    return 0

def solve(rows: list[Entry], limit: int) -> Report:
    yes_votes: dict[int, int] = {}
    no_votes: dict[int, int] = {}
    yes: int = 0
    no: int = 0
    for row in rows:
        next_yes: int = contribution(row.choice, row.weight, 1)
        next_no: int = contribution(row.choice, row.weight, 0)
        yes = yes + next_yes - read(yes_votes, row.voter, 0)
        no = no + next_no - read(no_votes, row.voter, 0)
        yes_votes = put(yes_votes, row.voter, next_yes)
        no_votes = put(no_votes, row.voter, next_no)
    met: int = 0
    if yes >= limit and yes > no:
        met = 1
    return Report(yes, no, met)
''', "events", [([[1, 1, 5], [2, 0, 3], [1, 2, 9], [3, 1, 4]], 4, [4, 3, 1])])

add("ranking_ties", "Assign competition ranks to scored submissions", "records",
    "contestant score penalty", "selected rank_sum tie_groups",
    "A submission ranks ahead of another if its score is higher, or its score is equal and its penalty is lower. Competition rank is 1 plus the number of records strictly ahead; identical score/penalty records share rank. Return number of records with rank <= limit, sum of those ranks, and number of distinct score/penalty groups containing at least two records across the whole input. IDs do not break ties.",
    "contestant is nonnegative; score may be negative; penalty is nonnegative; duplicate records are separate submissions.",
    '''def ahead(score: int, penalty: int, other_score: int, other_penalty: int) -> bool:
    return other_score > score or other_score == score and other_penalty < penalty

def solve(rows: list[Entry], limit: int) -> Report:
    selected: int = 0
    rank_sum: int = 0
    groups: int = 0
    index: int = 0
    for row in rows:
        rank: int = 1
        ties: int = 0
        first: int = index
        other_index: int = 0
        for other in rows:
            if ahead(row.score, row.penalty, other.score, other.penalty):
                rank = rank + 1
            if row.score == other.score and row.penalty == other.penalty:
                ties = ties + 1
                if other_index < first:
                    first = other_index
            other_index = other_index + 1
        if rank <= limit:
            selected = selected + 1
            rank_sum = rank_sum + rank
        if ties > 1 and first == index:
            groups = groups + 1
        index = index + 1
    return Report(selected, rank_sum, groups)
''', "records", [([[1, 9, 1], [2, 9, 1], [3, 9, 2], [4, 8, 0]], 3, [3, 5, 1])])

add("join_unmatched", "Reconcile multiplicities in a two-sided ID join", "records",
    "key side quantity", "matched left_only right_only",
    "Rows with side=0 contribute quantity units to the left relation; side=1 contributes to the right. Ignore other side values. For each key, pair min(left_units,right_units) units. Return total matched units and residual left-only and right-only units. Repeated keys accumulate. limit is reserved.",
    "key is nonnegative; quantity is positive; side may be any integer.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    left: dict[int, int] = {}
    right: dict[int, int] = {}
    for row in rows:
        if row.side == 0:
            left = put(left, row.key, read(left, row.key, 0) + row.quantity)
        if row.side == 1:
            right = put(right, row.key, read(right, row.key, 0) + row.quantity)
    seen: dict[int, int] = {}
    matched: int = 0
    left_only: int = 0
    right_only: int = 0
    for row in rows:
        if read(seen, row.key, 0) == 0:
            seen = put(seen, row.key, 1)
            a: int = read(left, row.key, 0)
            b: int = read(right, row.key, 0)
            pairs: int = a
            if b < pairs:
                pairs = b
            matched = matched + pairs
            left_only = left_only + a - pairs
            right_only = right_only + b - pairs
    return Report(matched, left_only, right_only)
''', "records", [([[1, 0, 4], [1, 1, 2], [2, 1, 5], [1, 0, 1]], 0, [2, 3, 5])])

add("retention_sweep", "Sweep expired objects while preserving newest copies", "records",
    "key modified size", "removed reclaimed retained",
    "Within each key, preserve exactly the record with largest modified timestamp, breaking ties in favor of the last input occurrence. Among all other records, remove those with modified < limit; retain those at or after the cutoff. Return removed record count, sum of removed sizes, and retained record count. The preserved newest record is retained even when expired.",
    "key is nonnegative; modified may be negative; size is positive; duplicate records are distinct copies.",
    '''def newest_index(rows: list[Entry], key: int) -> int:
    best_index: int = -1
    best_time: int = -1000000
    index: int = 0
    for row in rows:
        if row.key == key and row.modified >= best_time:
            best_index = index
            best_time = row.modified
        index = index + 1
    return best_index

def solve(rows: list[Entry], limit: int) -> Report:
    removed: int = 0
    reclaimed: int = 0
    retained: int = 0
    index: int = 0
    for row in rows:
        if row.modified < limit and index != newest_index(rows, row.key):
            removed = removed + 1
            reclaimed = reclaimed + row.size
        else:
            retained = retained + 1
        index = index + 1
    return Report(removed, reclaimed, retained)
''', "records", [([[1, 2, 5], [1, 2, 7], [2, 1, 9], [1, 5, 4]], 4, [2, 12, 2])])

add("dependency_layers", "Compute parallel build layers in a dependency DAG", "graph",
    "prerequisite dependent weight", "layers last_layer_count layer_sum",
    "Vertices are jobs 0..limit-1. A dependency edge prerequisite->dependent requires the prerequisite to complete first. A job's zero-based layer is zero with no prerequisites, otherwise 1+maximum prerequisite layer. Return number of nonempty layers (max+1), number of jobs in the last layer, and sum of layer indices. Empty job set yields (0,0,0). weight is ignored.",
    "0 <= prerequisite < dependent < limit, guaranteeing a finite DAG. Duplicate dependency records are allowed. Input order is arbitrary.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    levels: dict[int, int] = {}
    maximum: int = -1
    last_count: int = 0
    total: int = 0
    vertex: int = 0
    while vertex < limit:
        level: int = 0
        for row in rows:
            if row.dependent == vertex:
                prior: int = read(levels, row.prerequisite, 0) + 1
                if prior > level:
                    level = prior
        levels = put(levels, vertex, level)
        total = total + level
        if level > maximum:
            maximum = level
            last_count = 0
        if level == maximum:
            last_count = last_count + 1
        vertex = vertex + 1
    return Report(maximum + 1, last_count, total)
''', "dag", [([[0, 2, 1], [1, 2, 1], [2, 3, 1], [1, 4, 1]], 5, [3, 1, 4])])

add("route_cost", "Find cheapest forward routes from the origin", "graph",
    "source target cost", "reachable cost_sum most_expensive",
    "Compute minimum total route cost from vertex 0 to each reachable vertex in the directed graph. Return reachable vertex count including 0, sum of these minimum costs, and their maximum. Empty vertex set yields zeros. Parallel edges are alternative routes, and each edge's cost matters.",
    "0 <= source < target < limit (a finite DAG); cost is positive and <=1000; vertices 0..limit-1; duplicate and parallel edges allowed.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    costs: dict[int, int] = {}
    costs = put(costs, 0, 0)
    reachable: int = 0
    total: int = 0
    maximum: int = 0
    vertex: int = 0
    while vertex < limit:
        best: int = read(costs, vertex, 1000000)
        for row in rows:
            if row.target == vertex:
                candidate: int = read(costs, row.source, 1000000) + row.cost
                if candidate < best:
                    best = candidate
        costs = put(costs, vertex, best)
        if best < 1000000:
            reachable = reachable + 1
            total = total + best
            if best > maximum:
                maximum = best
        vertex = vertex + 1
    return Report(reachable, total, maximum)
''', "dag", [([[0, 1, 8], [0, 2, 2], [1, 3, 1], [2, 3, 3]], 5, [4, 15, 8])])

add("union_coverage", "Measure integer timeline coverage inside a window", "schedule",
    "start end channel", "covered gaps longest_gap",
    "Clip all half-open intervals to [0,limit). All endpoints are integers, so measure each unit slot [t,t+1). Return the number of slots covered by at least one interval, number of maximal uncovered runs, and longest uncovered run length. channel does not restrict union coverage. Empty window has zero outputs.",
    "0 <= start < end; arbitrary input order and overlaps allowed.",
    '''def covered_at(rows: list[Entry], time: int) -> bool:
    for row in rows:
        if row.start <= time and row.end > time:
            return True
    return False

def solve(rows: list[Entry], limit: int) -> Report:
    covered: int = 0
    gaps: int = 0
    longest: int = 0
    current: int = 0
    time: int = 0
    while time < limit:
        if covered_at(rows, time):
            covered = covered + 1
            current = 0
        else:
            if current == 0:
                gaps = gaps + 1
            current = current + 1
            if current > longest:
                longest = current
        time = time + 1
    return Report(covered, gaps, longest)
''', "interval", [([[1, 3, 0], [2, 4, 1], [6, 9, 0]], 8, [5, 2, 2])])

add("resource_leases", "Accept lease renewals against per-resource expiry", "state",
    "resource timestamp duration", "accepted rejected expires_sum",
    "Process requests in input order. Each resource initially expires at time zero. A request is accepted if timestamp >= current expiry, timestamp >= 0, and duration > 0; it replaces expiry with timestamp+duration. Otherwise reject it without changing expiry. Return accepted/rejected counts and sum of current expiries for all resources that accepted a request. limit is reserved.",
    "resource is nonnegative; timestamps may be nonmonotonic; duration and timestamp may be invalid/nonpositive.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    expiries: dict[int, int] = {}
    accepted: int = 0
    rejected: int = 0
    total: int = 0
    for row in rows:
        prior: int = read(expiries, row.resource, 0)
        if row.timestamp >= prior and row.duration > 0:
            next_expiry: int = row.timestamp + row.duration
            total = total + next_expiry - prior
            expiries = put(expiries, row.resource, next_expiry)
            accepted = accepted + 1
        else:
            rejected = rejected + 1
    return Report(accepted, rejected, total)
''', "records", [([[1, 0, 3], [1, 2, 7], [1, 3, 2], [2, -1, 4]], 0, [2, 2, 5])])

add("seat_assignments", "Allocate requested seats with deterministic fallback", "state",
    "passenger preferred group", "assigned rejected seat_checksum",
    "There are limit seats numbered 1..limit. For each row, reject passengers who already have an assigned seat. Otherwise assign preferred if it is in range and free; if unavailable/invalid, assign the lowest free seat. Reject if all are occupied. Rejected rows do not reserve passenger IDs. Return assigned/rejected counts and sum of (passenger+1)*seat across accepted rows. group is informational.",
    "passenger is nonnegative; preferred may be negative or out of range; repeated passengers allowed.",
    '''def choose_seat(occupied: dict[int, int], preferred: int, limit: int) -> int:
    if preferred > 0 and preferred <= limit:
        if read(occupied, preferred, 0) == 0:
            return preferred
    seat: int = 1
    while seat <= limit:
        if read(occupied, seat, 0) == 0:
            return seat
        seat = seat + 1
    return 0

def solve(rows: list[Entry], limit: int) -> Report:
    occupied: dict[int, int] = {}
    passengers: dict[int, int] = {}
    assigned: int = 0
    rejected: int = 0
    checksum: int = 0
    for row in rows:
        seat: int = 0
        if read(passengers, row.passenger, 0) == 0:
            seat = choose_seat(occupied, row.preferred, limit)
        if seat == 0:
            rejected = rejected + 1
        else:
            assigned = assigned + 1
            checksum = checksum + (row.passenger + 1) * seat
            occupied = put(occupied, seat, 1)
            passengers = put(passengers, row.passenger, 1)
    return Report(assigned, rejected, checksum)
''', "records", [([[2, 2, 0], [3, 2, 0], [2, 3, 1], [4, 9, 0], [5, 1, 0]], 3, [3, 2, 25])])

add("patient_triage", "Select a bounded triage queue deterministically", "records",
    "patient severity arrival", "selected patient_checksum severity_sum",
    "Select the first limit records ordered by severity descending, then arrival ascending, then original input index ascending. Return selected count, sum of (one-based selection position)*(patient+1), and sum of selected severities. Duplicate patient IDs remain separate records. This is a scheduling exercise, not clinical guidance.",
    "patient is nonnegative; severity is an integer; arrival is nonnegative. Stable ties use original occurrence, not patient ID.",
    '''def precedes(severity: int, arrival: int, index: int, other_severity: int, other_arrival: int, other_index: int) -> bool:
    if other_severity != severity:
        return other_severity > severity
    if other_arrival != arrival:
        return other_arrival < arrival
    return other_index < index

def rank_of(rows: list[Entry], severity: int, arrival: int, index: int) -> int:
    rank: int = 1
    other_index: int = 0
    for other in rows:
        if precedes(severity, arrival, index, other.severity, other.arrival, other_index):
            rank = rank + 1
        other_index = other_index + 1
    return rank

def solve(rows: list[Entry], limit: int) -> Report:
    selected: int = 0
    checksum: int = 0
    total: int = 0
    index: int = 0
    for row in rows:
        rank: int = rank_of(rows, row.severity, row.arrival, index)
        if rank <= limit:
            selected = selected + 1
            checksum = checksum + rank * (row.patient + 1)
            total = total + row.severity
        index = index + 1
    return Report(selected, checksum, total)
''', "records", [([[9, 2, 1], [3, 5, 4], [4, 5, 4], [8, 5, 2]], 2, [2, 17, 10])])

add("change_log", "Apply idempotent changes by event identifier", "state",
    "event key delta", "applied duplicates checksum",
    "Only the first record for each event ID is applied; later occurrences count as duplicates even when their payload differs. Each key starts at zero. Apply delta to its key and report number applied, number duplicates, and sum of (key+1)*final_value over keys. limit is reserved.",
    "event and key are nonnegative; delta may be negative; duplicate event IDs may refer to other keys.",
    '''def solve(rows: list[Entry], limit: int) -> Report:
    seen: dict[int, int] = {}
    applied: int = 0
    duplicates: int = 0
    checksum: int = 0
    for row in rows:
        if read(seen, row.event, 0) == 0:
            seen = put(seen, row.event, 1)
            checksum = checksum + (row.key + 1) * row.delta
            applied = applied + 1
        else:
            duplicates = duplicates + 1
    return Report(applied, duplicates, checksum)
''', "events", [([[1, 0, 5], [2, 1, -2], [1, 9, 9], [3, 0, 1]], 0, [3, 1, 2])])

# Independent adversarial cases complement the deterministic input sampling.
EXTRA_CASES = {
    "ledger_audit": [([[0, 3, 1], [1, 6, 2]], 5), ([[0, -5, 1], [0, 1, 2]], 5), ([[0, -6, 1], [1, -8, 2]], 5)],
    "reservation_gate": [([[0, 5, 0]], 5), ([[0, 0, 0], [0, 2, 0]], 2), ([[1, 2, 0], [2, 3, 0], [1, 1, 0]], 5)],
    "sensor_sessions": [([[1, 0, 0], [1, 1, 0], [1, 1, 1]], 0), ([[1, 0, -1], [1, 0, 2], [1, 0, 3], [1, 1, 1], [1, 1, 2]], 0)],
    "job_readiness": [([[9, 8, 5], [3, 2, 1], [2, 1, 8], [1, 0, -2], [10, 9, 3]], 0)],
    "graph_reachability": [([[2, 3, 1], [1, 2, 1], [0, 1, 1], [3, 1, 1], [0, 1, 2]], 5)],
    "graph_shortest_hops": [([[0, 1, 50], [1, 2, 1], [2, 3, 1], [0, 3, 999], [3, 4, 1]], 6)],
    "graph_components": [([[2, 3, 1], [0, 1, 1], [1, 2, 1], [4, 4, 1]], 6)],
    "graph_degrees": [([[0, 1, 1], [0, 1, 1], [1, 0, 1], [2, 2, 1]], 4)],
    "tree_depths": [([[5, 4, -1], [4, 3, 2], [3, 2, 4], [2, 1, 8], [1, 0, 3]], 0)],
    "tree_subtree_weight": [([[1, 0, 2], [2, 1, 3], [3, 1, 4], [4, 2, -5], [5, 0, 8]], 1), ([[1, 0, 2]], 0)],
    "tree_lowest_ancestor": [([[1, 0, 1], [2, 1, 1], [3, 1, 1], [4, 2, 1]], 1), ([[1, 0, 1], [2, 0, 1]], 1)],
    "schedule_conflicts": [([[0, 2, 1], [2, 4, 1]], 0), ([[0, 3, 1], [0, 3, 1], [0, 3, 2]], 0)],
    "schedule_peak_load": [([[0, 2, 3], [2, 4, 3]], 3), ([[2, 4, 5], [0, 2, 5], [0, 2, 1]], 5)],
    "schedule_deadline_audit": [([[2, 5, 1]], 3), ([[1, 0, 2], [3, 4, 5]], 0)],
    "schedule_room_reuse": [([[0, 1, 1], [0, 2, 2], [0, 3, 3], [3, 4, 4], [3, 4, 5]], 0)],
    "versioned_register": [([[0, 0, 3], [0, 0, 9]], 0), ([[0, -1, 8], [0, -2, 9], [0, 0, 0]], 0)],
    "permission_rules": [([[0, 0, 1], [0, 1, 1]], 1), ([[0, 1, 2], [0, 0, 1], [4, 1, 8]], 2)],
    "restock_projection": [([[1, -5, 3], [2, 0, 3]], 0), ([[1, 6, 3], [1, 5, 3]], 2)],
    "invoice_reconciliation": [([[1, 0, 0]], 0), ([[1, -3, 0], [1, 3, 0], [2, -1, 0], [3, 2, 0]], 0)],
    "vote_quorum": [([[0, 1, 5], [0, 1, 0]], 1), ([[0, 1, 3], [1, 0, 3]], 3), ([[0, 1, 3]], 3)],
    "ranking_ties": [([[0, 9, 1], [1, 9, 1], [2, 8, 1]], 2), ([[0, 0, 2], [1, 0, 1]], 1)],
    "join_unmatched": [([[0, 0, 2], [0, 0, 3], [0, 1, 4], [1, 2, 9]], 0)],
    "retention_sweep": [([[0, 2, 3], [0, 2, 7]], 3), ([[0, 3, 5], [0, 4, 2], [1, 1, 9]], 3)],
    "dependency_layers": [([[3, 4, 1], [2, 3, 1], [1, 2, 1], [0, 1, 1], [0, 4, 1]], 6)],
    "route_cost": [([[0, 1, 9], [0, 1, 3], [1, 3, 5], [2, 3, 1], [0, 3, 12]], 5)],
    "union_coverage": [([[0, 3, 1], [3, 5, 2]], 5), ([[9, 12, 1]], 5), ([[0, 1, 1], [4, 5, 1]], 5)],
    "resource_leases": [([[0, 0, 0], [0, 0, 2], [0, 2, 3], [0, 1, 9]], 0)],
    "seat_assignments": [([[0, 0, 1], [1, -1, 1], [2, 9, 1], [0, 3, 1]], 3)],
    "patient_triage": [([[8, 5, 2], [1, 5, 2], [3, 5, 1]], 2)],
    "change_log": [([[0, 0, 0], [0, 1, 99], [1, 0, -1]], 0)],
}

# Fixture refinements preserve each public domain rather than silently filtering
# invalid solver behavior after generation.
# Sparse IDs must work independently of row count and input order. The root is
# 2, its children are 9 and 30, and 12 is a child of 9. Weighted depths are
# 3*0 + 5*1 + (-2)*2 + 7*1 = 8; subtree 9 contains weights 5 and -2; the
# common ancestor of 12 and 30 is 2 at distances two and one respectively.
SPARSE_TREE_ANCHORS = {
    "data_tree_depths": ([[30, 2, 7], [12, 9, -2], [2, 0, 3], [9, 2, 5]], 0, [1, 2, 8]),
    "data_tree_subtree_weight": ([[30, 2, 7], [12, 9, -2], [2, 0, 3], [9, 2, 5]], 9, [2, 3, 1]),
    "data_tree_lowest_ancestor": ([[30, 2, 7], [12, 9, -2], [2, 0, 3], [9, 2, 5]], 12, [2, 2, 1]),
}
for spec in SPECS:
    if spec["id"] in SPARSE_TREE_ANCHORS:
        spec["anchors"].append(SPARSE_TREE_ANCHORS[spec["id"]])
    spec["cases"] += EXTRA_CASES[spec["id"].removeprefix("data_")]
    for old, new in [("resource", "resource_id"), ("time", "moment")]:
        spec["fields"] = re.sub(r"\b" + old + r"\b", new, spec["fields"])
        spec["code"] = re.sub(r"\b" + old + r"\b", new, spec["code"])
    if spec["id"] == "data_schedule_room_reuse":
        spec["cases"] = [(sorted(rows, key=lambda row: row[0]), limit) for rows, limit in spec["cases"]]
    if spec["id"] == "data_change_log":
        spec["cases"] = [([[row[0], abs(row[1]), row[2]] for row in rows], limit) for rows, limit in spec["cases"]]
    if spec["id"] == "data_retention_sweep":
        spec["domain"] += " modified is at least -1000000."
    starter_bugs = {
        "data_reservation_gate": ("occupied + row.seats > limit", "occupied + row.seats >= limit"),
        "data_versioned_register": ("row.version > read", "row.version >= read"),
        "data_retention_sweep": ("row.modified >= best_time", "row.modified > best_time"),
        "data_schedule_deadline_audit": ("finish > row.deadline", "finish >= row.deadline"),
    }
    if spec["id"] in starter_bugs:
        correct, incorrect = starter_bugs[spec["id"]]
        assert correct in spec["code"]
        spec["starter_code"] = spec["code"].replace(correct, incorrect)
        spec["statement"] = "Repair the supplied implementation to satisfy this contract, preserving the public types and signature. " + spec["statement"]
        spec["category"] = "maintenance"

    ignored_fields = {
        "data_ledger_audit": "reference", "data_reservation_gate": "priority",
        "data_graph_reachability": "label", "data_graph_shortest_hops": "weight",
        "data_graph_components": "tag", "data_graph_degrees": "label",
        "data_tree_lowest_ancestor": "weight", "data_schedule_room_reuse": "guest",
        "data_invoice_reconciliation": "source", "data_ranking_ties": "contestant",
        "data_dependency_layers": "weight", "data_union_coverage": "channel",
        "data_seat_assignments": "group", "data_restock_projection": "sku",
    }
    ignored = ignored_fields.get(spec["id"])
    if ignored:
        fields = spec["fields"].split()
        discarded = fields.index(ignored)
        spec["fields"] = " ".join(field for field in fields if field != ignored)
        spec["cases"] = [([row[:discarded] + row[discarded + 1:] for row in rows], limit) for rows, limit in spec["cases"]]
        spec["anchors"] = [([row[:discarded] + row[discarded + 1:] for row in rows], limit, expected) for rows, limit, expected in spec["anchors"]]
    spec["uses_limit"] = "limit is reserved" not in spec["statement"].lower()
    if not spec["uses_limit"]:
        for field in ["code", "starter_code"]:
            if field in spec:
                spec[field] = spec[field].replace("def solve(rows: list[Entry], limit: int)", "def solve(rows: list[Entry])")
        spec["statement"] = re.sub(r" ?limit is reserved(?: and has no effect)?\.", "", spec["statement"], flags=re.IGNORECASE)

# Remove prose referring to fields deliberately excluded from the interface.
PROSE_REPLACEMENTS = {
    "reference is an audit identifier and does not affect balances.": "",
    "repeated references are allowed and are not deduplicated.": "All rows are applied, including identical rows.",
    "priority is informational and does not reorder requests.": "", " priority is nonnegative.": "",
    "label does not affect reachability.": "", "weight is intentionally ignored: every edge costs one hop.": "Every edge costs one hop.",
    "tag is informational.": "", "label is ignored.": "", "weight is ignored.": "",
    "guest does not affect assignment.": "", "; guest is nonnegative": "",
    "source records provenance and is ignored for arithmetic.": "", "IDs do not break ties.": "",
    "contestant is nonnegative; ": "", "channel does not restrict union coverage.": "",
    "group is informational.": "", "For each independent SKU row": "For each independent stock record",
    "Distinct rows are separate SKU locations even if IDs repeat.": "Duplicate rows represent separate locations.",
    "sku is nonnegative; ": "",
}
for spec in SPECS:
    for field in ["statement", "domain"]:
        for old, new in PROSE_REPLACEMENTS.items():
            spec[field] = spec[field].replace(old, new)
        spec[field] = re.sub(r" +", " ", spec[field]).strip()

assert len(SPECS) == 30
