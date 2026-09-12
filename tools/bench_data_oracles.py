"""Independent expected-value implementations for the structured task family.

These routines are not emitted into baselines. They use separate algorithms and
standard library data structures to cross-check the deliberately constrained
portable implementations. This module is evaluator-only material.
"""
from __future__ import annotations

from collections import Counter, defaultdict, deque
from functools import cache
import heapq

Record = dict[str, int]


def expected(task_id: str, rows: list[Record], limit: int) -> list[int]:
    task = task_id.removeprefix("data_")
    if task == "ledger_audit":
        balances: dict[int, int] = {}
        observed: list[int] = []
        negatives: list[int] = []
        for index, row in enumerate(rows):
            account = row["account"]
            balances[account] = balances.get(account, limit) + row["delta"]
            observed.append(balances[account])
            if balances[account] < 0:
                negatives.append(index)
        return [sum(balances.values()), min(observed, default=limit), negatives[0] if negatives else -1]
    if task == "reservation_gate":
        accepted: list[Record] = []
        for row in rows:
            if row["seats"] > 0 and row["party"] not in {entry["party"] for entry in accepted}:
                if sum(entry["seats"] for entry in accepted) + row["seats"] <= limit:
                    accepted.append(row)
        return [len(accepted), len(rows) - len(accepted), sum(row["seats"] for row in accepted)]
    if task == "sensor_sessions":
        opened: dict[int, int] = {}
        completed = rejected = 0
        for row in rows:
            sensor, op, timestamp = row["sensor"], row["operation"], row["timestamp"]
            if timestamp < 0:
                rejected += 1
            elif op == 0 and sensor not in opened:
                opened[sensor] = timestamp
            elif op == 1 and sensor in opened and timestamp >= opened[sensor]:
                del opened[sensor]
                completed += 1
            else:
                rejected += 1
        return [completed, len(opened), rejected]
    if task == "job_readiness":
        ready = {0}
        changed = True
        while changed:
            next_ready = ready | {row["job"] for row in rows if row["prerequisite"] in ready}
            changed = next_ready != ready
            ready = next_ready
        accepted = [row for row in rows if row["job"] in ready]
        return [len(accepted), len(rows) - len(accepted), sum(row["cost"] for row in accepted)]
    if task in {"graph_reachability", "graph_shortest_hops"}:
        neighbors: dict[int, list[int]] = defaultdict(list)
        for row in rows:
            neighbors[row["source"]].append(row["target"])
        distance = {0: 0} if limit else {}
        queue = deque(distance)
        while queue:
            start = queue.popleft()
            for target in neighbors[start]:
                if target not in distance:
                    distance[target] = distance[start] + 1
                    queue.append(target)
        if task == "graph_reachability":
            return [len(distance), limit - len(distance), sum(row["source"] in distance for row in rows)]
        return [sum(distance.values()), max(distance.values(), default=0), limit - len(distance)]
    if task == "graph_components":
        parent = list(range(limit))
        def root(vertex: int) -> int:
            while parent[vertex] != vertex:
                vertex = parent[vertex]
            return vertex
        incident: set[int] = set()
        for row in rows:
            a, b = row["source"], row["target"]
            parent[root(a)] = root(b)
            incident.update((a, b))
        sizes = Counter(root(vertex) for vertex in range(limit))
        return [len(sizes), max(sizes.values(), default=0), limit - len(incident)]
    if task == "graph_degrees":
        incoming = Counter(row["target"] for row in rows)
        outgoing = Counter(row["source"] for row in rows)
        return [sum(incoming[v] == 0 and outgoing[v] > 0 for v in range(limit)),
                sum(outgoing[v] == 0 and incoming[v] > 0 for v in range(limit)),
                sum(incoming[v] == outgoing[v] for v in range(limit))]
    if task in {"tree_depths", "tree_subtree_weight", "tree_lowest_ancestor"}:
        parents = {row["node"]: row["parent"] for row in rows}
        children: dict[int, list[int]] = defaultdict(list)
        for node, parent_id in parents.items():
            children[parent_id].append(node)
        depths: dict[int, int] = {}
        queue = deque((node, 0) for node in children[0])
        while queue:
            node, depth = queue.popleft()
            depths[node] = depth
            queue.extend((child, depth + 1) for child in children[node])
        if task == "tree_depths":
            return [len(children[0]), max(depths.values(), default=0), sum(depths[row["node"]] * row["weight"] for row in rows)]
        if task == "tree_subtree_weight":
            selected: set[int] = set()
            pending = [limit] if limit in parents else []
            while pending:
                node = pending.pop()
                selected.add(node)
                pending.extend(children[node])
            return [len(selected), sum(row["weight"] for row in rows if row["node"] in selected), sum(not children[node] for node in selected)]
        if limit not in parents:
            return [0, -1, -1]
        left, right = limit, max(parents)
        left_distance = right_distance = 0
        while left != right:
            if depths[left] >= depths[right]:
                left = parents[left]
                left_distance += 1
            else:
                right = parents[right]
                right_distance += 1
            if not left or not right:
                return [0, -1, -1]
        return [left, left_distance, right_distance]
    if task == "schedule_conflicts":
        overlaps: list[int] = []
        involved: set[int] = set()
        for i, left in enumerate(rows):
            for j in range(i + 1, len(rows)):
                right = rows[j]
                if left["resource_id"] == right["resource_id"]:
                    duration = min(left["end"], right["end"]) - max(left["start"], right["start"])
                    if duration > 0:
                        overlaps.append(duration)
                        involved.update((i, j))
        return [len(overlaps), len(involved), max(overlaps, default=0)]
    if task == "schedule_peak_load":
        changes: Counter[int] = Counter()
        starts = {row["start"] for row in rows}
        for row in rows:
            changes[row["start"]] += row["demand"]
            changes[row["end"]] -= row["demand"]
        loads: dict[int, int] = {}
        active = 0
        for moment in sorted(changes):
            active += changes[moment]
            if moment in starts:
                loads[moment] = active
        peak = max(loads.values(), default=0)
        return [peak, min((moment for moment in loads if loads[moment] == peak), default=-1), sum(load > limit for load in loads.values())]
    if task == "schedule_deadline_audit":
        finishes = [limit + sum(row["duration"] for row in rows[:i + 1]) for i in range(len(rows))]
        tardiness = [max(0, finish - row["deadline"]) for finish, row in zip(finishes, rows, strict=True)]
        return [sum(value > 0 for value in tardiness), sum(value * row["penalty"] for value, row in zip(tardiness, rows, strict=True)), finishes[-1] if finishes else limit]
    if task == "schedule_room_reuse":
        rooms: list[int] = []
        assignments: list[int] = []
        reused = 0
        for row in rows:
            available = [index for index, end in enumerate(rooms) if end <= row["start"]]
            if available:
                slot = available[0]
                rooms[slot] = row["end"]
                reused += 1
            else:
                slot = len(rooms)
                rooms.append(row["end"])
            assignments.append(slot + 1)
        return [len(rooms), sum((index + 1) * slot for index, slot in enumerate(assignments)), reused]
    if task == "versioned_register":
        accepted: list[Record] = []
        for index, row in enumerate(rows):
            previous = [entry["version"] for entry in rows[:index] if entry["key"] == row["key"]]
            if row["version"] > max(previous, default=-1) and row["version"] >= 0:
                accepted.append(row)
        values = {row["key"]: row["value"] for row in accepted}
        return [len(accepted), len(rows) - len(accepted), sum((key + 1) * value for key, value in values.items())]
    if task == "permission_rules":
        decisions: list[int] = []
        defaulted = 0
        for principal in range(limit):
            choices = [(row["specificity"], index, row["allowed"]) for index, row in enumerate(rows) if row["principal"] == principal]
            if choices:
                decisions.append(max(choices)[2])
            else:
                defaulted += 1
                decisions.append(0)
        return [sum(decisions), limit - sum(decisions), defaulted]
    if task == "restock_projection":
        shortages = [max(row["daily_demand"] * limit - row["stock"], 0) for row in rows]
        return [sum(shortages), sum(value > 0 for value in shortages), max(shortages, default=0)]
    if task == "invoice_reconciliation":
        balances = [sum(row["amount"] for row in rows if row["invoice"] == key) for key in {row["invoice"] for row in rows}]
        return [balances.count(0), sum(value for value in balances if value > 0), -sum(value for value in balances if value < 0)]
    if task == "vote_quorum":
        ballots = {row["voter"]: row for row in rows}
        yes = sum(row["weight"] for row in ballots.values() if row["choice"] == 1 and row["weight"] > 0)
        no = sum(row["weight"] for row in ballots.values() if row["choice"] == 0 and row["weight"] > 0)
        return [yes, no, int(yes >= limit and yes > no)]
    if task == "ranking_ties":
        ordered = sorted(rows, key=lambda row: (-row["score"], row["penalty"]))
        groups = Counter((row["score"], row["penalty"]) for row in rows)
        ranks: dict[tuple[int, int], int] = {}
        for index, row in enumerate(ordered):
            ranks.setdefault((row["score"], row["penalty"]), index + 1)
        selected = [ranks[row["score"], row["penalty"]] for row in rows if ranks[row["score"], row["penalty"]] <= limit]
        return [len(selected), sum(selected), sum(size > 1 for size in groups.values())]
    if task == "join_unmatched":
        left = Counter[int]()
        right = Counter[int]()
        for row in rows:
            if row["side"] == 0:
                left[row["key"]] += row["quantity"]
            elif row["side"] == 1:
                right[row["key"]] += row["quantity"]
        matched = sum((left & right).values())
        return [matched, sum(left.values()) - matched, sum(right.values()) - matched]
    if task == "retention_sweep":
        newest = {key: max((row["modified"], index) for index, row in enumerate(rows) if row["key"] == key)[1] for key in {row["key"] for row in rows}}
        removed = [row for index, row in enumerate(rows) if index != newest[row["key"]] and row["modified"] < limit]
        return [len(removed), sum(row["size"] for row in removed), len(rows) - len(removed)]
    if task == "dependency_layers":
        predecessors: dict[int, list[int]] = defaultdict(list)
        for row in rows:
            predecessors[row["dependent"]].append(row["prerequisite"])
        @cache
        def layer(vertex: int) -> int:
            return 1 + max((layer(parent) for parent in predecessors[vertex]), default=-1)
        levels = [layer(vertex) for vertex in range(limit)]
        maximum = max(levels, default=-1)
        return [maximum + 1, levels.count(maximum), sum(levels)]
    if task == "route_cost":
        adjacency: dict[int, list[tuple[int, int]]] = defaultdict(list)
        for row in rows:
            adjacency[row["source"]].append((row["target"], row["cost"]))
        pending = [(0, 0)] if limit else []
        settled: dict[int, int] = {}
        while pending:
            cost, vertex = heapq.heappop(pending)
            if vertex in settled:
                continue
            settled[vertex] = cost
            for target, edge_cost in adjacency[vertex]:
                heapq.heappush(pending, (cost + edge_cost, target))
        return [len(settled), sum(settled.values()), max(settled.values(), default=0)]
    if task == "union_coverage":
        covered = {moment for row in rows for moment in range(max(0, row["start"]), min(limit, row["end"]))}
        absent = sorted(set(range(limit)) - covered)
        runs: list[list[int]] = []
        for moment in absent:
            if not runs or runs[-1][-1] != moment - 1:
                runs.append([])
            runs[-1].append(moment)
        return [len(covered), len(runs), max(map(len, runs), default=0)]
    if task == "resource_leases":
        accepted: list[Record] = []
        for row in rows:
            prior = [entry["timestamp"] + entry["duration"] for entry in accepted if entry["resource_id"] == row["resource_id"]]
            if row["timestamp"] >= max(prior, default=0) and row["duration"] > 0:
                accepted.append(row)
        expires = {row["resource_id"]: row["timestamp"] + row["duration"] for row in accepted}
        return [len(accepted), len(rows) - len(accepted), sum(expires.values())]
    if task == "seat_assignments":
        free = set(range(1, limit + 1))
        assignments: dict[int, int] = {}
        for row in rows:
            if row["passenger"] in assignments or not free:
                continue
            selected = row["preferred"] if row["preferred"] in free else min(free)
            assignments[row["passenger"]] = selected
            free.remove(selected)
        return [len(assignments), len(rows) - len(assignments), sum((passenger + 1) * seat for passenger, seat in assignments.items())]
    if task == "patient_triage":
        selected = sorted(enumerate(rows), key=lambda pair: (-pair[1]["severity"], pair[1]["arrival"], pair[0]))[:limit]
        return [len(selected), sum(rank * (row["patient"] + 1) for rank, (_, row) in enumerate(selected, 1)), sum(row["severity"] for _, row in selected)]
    if task == "change_log":
        first: dict[int, Record] = {}
        for row in reversed(rows):
            first[row["event"]] = row
        return [len(first), len(rows) - len(first), sum((row["key"] + 1) * row["delta"] for row in first.values())]
    raise ValueError(task_id)
