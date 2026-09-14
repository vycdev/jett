#!/usr/bin/env python3
"""Audit a complete frozen campaign without writes; emit unique-response accounting JSON."""

from __future__ import annotations

import sys

# Even importing local helpers must not create files during a read-only audit.
sys.dont_write_bytecode = True

import argparse
import json
import math
from pathlib import Path
import re

try:
    from tools import bench_campaign_staging as staging
    from tools import bench_recover_interrupted as recovery
except ModuleNotFoundError:
    import bench_campaign_staging as staging
    import bench_recover_interrupted as recovery

campaign, bench = staging.campaign, staging.campaign.bench
Row = dict[str, object]
Error = bench.BenchmarkError
TOKEN_FIELDS = ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens")
USAGE_FIELDS = (*TOKEN_FIELDS, "latency_ms")
GRADE_ADDITIONS = {"generation_status", "generation_sha256", "grading_image", "compile_succeeded",
                   "grader_runtime_ms", "command_logs", "diagnostic"}


def object_file(path: Path) -> Row:
    return staging.json_object(path.read_bytes(), str(path))


def inventory(directory: Path) -> dict[str, str]:
    if (directory / "active.lock").exists():
        raise Error("campaign has an active lock; audit requires quiescent evidence")
    result: dict[str, str] = {}
    for path in sorted(directory.rglob("*")):
        if path.is_symlink() or not path.resolve().is_relative_to(directory):
            raise Error("campaign evidence must not contain symlinks or escaping paths")
        if path.is_file():
            result[path.relative_to(directory).as_posix()] = campaign.digest(path)
    return result


def integer(row: Row, field: str, minimum: int = 0) -> int:
    value = row.get(field)
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        raise Error(f"invalid integer {field}")
    return value


def names(row: Row, field: str, allowed: set[str]) -> list[str]:
    value = row.get(field)
    if not isinstance(value, list) or not value or not all(isinstance(item, str) for item in value):
        raise Error(f"invalid manifest/config {field}")
    result = [str(item) for item in value]
    if len(set(result)) != len(result) or not set(result).issubset(allowed):
        raise Error(f"duplicate or unsupported manifest/config {field}")
    return result


def matrix(manifest: Row, config: Row, plans: dict[str, Row]) -> None:
    languages = names(manifest, "languages", set(bench.LANGUAGE_SKILLS))
    tracks = names(manifest, "tracks", set(campaign.TRACKS))
    if not set(languages).issubset(names(config, "languages", set(bench.LANGUAGE_SKILLS))):
        raise Error("manifest languages differ from frozen config")
    for field in ("model", "benchmark_version"):
        if manifest.get(field) != config.get(field):
            raise Error(f"manifest differs from frozen config: {field}")
    count = integer(manifest, "task_count", 1)
    if integer(manifest, "initial_count", 1) != count * len(languages) * len(tracks):
        raise Error("manifest initial count does not match its matrix")
    if integer(manifest, "max_repairs_per_failure") != 1 or len(plans) != manifest["initial_count"]:
        raise Error("incomplete initial matrix or unsupported repair budget")
    cells: set[tuple[str, str, str]] = set()
    versions: dict[str, str] = {}
    for sequence, row in enumerate(plans.values(), 1):
        task = staging.string_field(row, "task_id", "initial plan")
        language = staging.string_field(row, "language", task)
        track = staging.string_field(row, "track", task)
        version = staging.string_field(row, "task_version", task)
        if (language not in languages or track not in tracks or (task, language, track) in cells
                or versions.get(task, version) != version):
            raise Error("initial matrix has a duplicate, foreign, or inconsistent task cell")
        cells.add((task, language, track))
        versions[task] = version
        for field in ("benchmark_version", "git_revision", "model", "reasoning_effort"):
            if row.get(field) != staging.string_field(manifest, field, "manifest"):
                raise Error(f"initial plan differs from manifest: {field}")
        if (integer(row, "repair_attempt") != 0 or integer(row, "repetition", 1) != 1
                or integer(row, "sequence", 1) != sequence or row.get("status") != "planned"
                or row.get("passed") is not False or row.get("evaluation_mode") != "one_shot"):
            raise Error("initial plan is not one canonical initial observation per cell")
        expected_id = f"{row['benchmark_version']}:{task}:{version}:{language}:{track}:{row['reasoning_effort']}:01"
        if row.get("run_id") != expected_id:
            raise Error("initial run ID differs from its frozen task identity")
    if len(versions) != count:
        raise Error("initial matrix task count differs from manifest")


def event_metrics(path: Path, raw: Row) -> Row:
    content = path.read_bytes()
    if not content.endswith(b"\n"):
        raise Error("truncated event JSONL")
    events = [staging.json_object(line, str(path)) for line in content.split(b"\n")[:-1]]
    kinds = [event.get("type") for event in events]
    if kinds.count("thread.started") != 1 or kinds.count("turn.completed") != 1 or any(
            kind in {"turn.failed", "error"} for kind in kinds):
        raise Error("event trace does not contain one successfully completed response")
    metrics = bench.codex_event_metrics(events)
    if integer(metrics, "tool_calls") != 0 or metrics.get("tool_types") != []:
        raise Error("observed tool use invalidates isolated generation")
    staging.string_field(metrics, "response_id", "event metrics")
    integer(metrics, "input_tokens")
    for field in TOKEN_FIELDS:
        if metrics.get(field) is not None:
            integer(metrics, field)
    for subset, total in (("cached_input_tokens", "input_tokens"), ("reasoning_tokens", "output_tokens")):
        if metrics.get(subset) is not None and metrics.get(total) is not None:
            if integer(metrics, subset) > integer(metrics, total):
                raise Error(f"event accounting subset exceeds {total}")
    messages: list[str] = []
    for event in events:
        item = event.get("item")
        if isinstance(item, dict) and item.get("type") == "agent_message" and event.get("type") == "item.completed":
            message = item.get("text")
            if not isinstance(message, str):
                raise Error("agent message has no text")
            messages.append(message)
    output = raw.get("raw_output")
    if not isinstance(output, str) or not messages or output.rstrip("\r\n") != messages[-1].rstrip("\r\n"):
        raise Error("saved raw output differs from the final event message")
    return metrics


def validate_raw(directory: Path, stage: str, plan: Row, raw: Row, approved: dict[str, Row]) -> None:
    run_id = staging.string_field(plan, "run_id", "stage plan")
    instructions = staging.string_field(plan, "instructions", run_id)
    prompt = staging.string_field(plan, "prompt", run_id)
    if plan.get("prompt_sha256") != bench.sha256_text(instructions + "\n" + prompt):
        raise Error("stage plan prompt hash differs from its content")
    event = directory / f"events-{stage}" / staging.string_field(raw, "raw_event_log", run_id)
    metrics = event_metrics(event, raw)
    latency = raw.get("latency_ms")
    if not isinstance(latency, (int, float)) or isinstance(latency, bool) or not math.isfinite(latency) or latency < 0:
        raise Error("invalid saved-response latency")
    backend = {"version": staging.string_field(raw, "codex_cli_version", run_id), "login": "ChatGPT subscription"}
    output = raw.get("raw_output")
    if not isinstance(output, str):
        raise Error("saved response has no raw output")
    expected = bench.result_from_codex_subscription(plan, backend, output, metrics, latency, str(event))
    expected["completed_at_utc"] = staging.string_field(raw, "completed_at_utc", run_id)
    if stage == "initial" and run_id in approved:
        expected["response_recovery"] = approved[run_id]
    if campaign.row_digest(raw) != campaign.row_digest(expected):
        raise Error(f"raw response identity, metrics, or extracted source mismatch: {run_id}")


def validate_attempts(directory: Path, stage: str, raw: dict[str, Row], originals: set[str]) -> None:
    events = directory / f"events-{stage}"
    expected_files = set(originals)
    for run_id, row in raw.items():
        filename = staging.string_field(row, "raw_event_log", run_id)
        if not filename.endswith(".jsonl"):
            raise Error("event filename must end in .jsonl")
        receipt = filename.removesuffix(".jsonl") + ".attempt.json"
        attempt = object_file(events / receipt)
        if (attempt.get("run_id") != run_id or attempt.get("prompt_sha256") != row.get("prompt_sha256")
                or attempt.get("status") != "completed" or attempt.get("raw_event_log") != filename):
            raise Error(f"completed attempt receipt does not match response: {run_id}")
        expected_files.update((filename, receipt))
    actual = {path.name for path in events.iterdir()} if events.exists() else set()
    if actual != expected_files:
        raise Error(f"unexplained or missing {stage} attempt/event evidence")


def validate_stage(directory: Path, stage: str, plans: dict[str, Row], image: str,
                   approved: dict[str, Row], response_ids: set[str]) -> tuple[staging.Journal, staging.Journal]:
    raw = staging.journal(directory / f"{stage}-raw.jsonl", optional=not plans)
    graded = staging.journal(directory / f"{stage}-graded.jsonl", optional=not plans)
    staging.raw_matches_plan(raw, plans, f"complete {stage} raw", full=True)
    staging.raw_matches_plan(graded, plans, f"complete {stage} grades", full=True)
    staging.valid_grades(graded, raw, image, f"{stage} grading")
    campaign.verify_event_evidence(directory, stage, list(raw.rows.values()))
    for run_id, row in raw.rows.items():
        validate_raw(directory, stage, plans[run_id], row, approved)
        response_id = staging.string_field(row, "response_id", run_id)
        if response_id in response_ids:
            raise Error("duplicate response ID across the campaign")
        response_ids.add(response_id)
        grade = graded.rows[run_id]
        if grade.keys() - row.keys() - GRADE_ADDITIONS:
            raise Error("grade added unexpected generation fields")
        for key, value in row.items():
            if key not in {"status", "passed", "diagnostic"} and (
                    key not in grade or campaign.row_digest({key: grade[key]}) != campaign.row_digest({key: value})):
                raise Error(f"grade changed raw generation identity or usage: {run_id}: {key}")
        if row.get("extracted_source"):
            if grade.get("generation_status") != row.get("status"):
                raise Error("grade lost its original generation status")
        elif grade.get("status") != "extraction_error" or grade.get("diagnostic") != row.get("diagnostic"):
            raise Error("unextracted response must retain its extraction error")
    originals = {staging.string_field(item, "prior_attempt_filename", "approval") for item in approved.values()}
    validate_attempts(directory, stage, raw.rows, originals if stage == "initial" else set())
    return raw, graded


def recovery_approvals(directory: Path, plans: dict[str, Row], raw: staging.Journal) -> dict[str, Row]:
    recovered: set[str] = set()
    decisions: set[str] = set()
    for run_id, row in raw.rows.items():
        if "response_recovery" not in row:
            continue
        metadata = row["response_recovery"]
        if not isinstance(metadata, dict):
            raise Error("malformed response recovery metadata")
        decisions.add(staging.string_field(metadata, "decision_file", run_id))
        recovered.add(run_id)
    if not decisions:
        return {}
    if len(decisions) != 1:
        raise Error("multiple recovery decisions require separate review")
    approved = recovery.approved_requests(directory, next(iter(decisions)), plans)
    if recovered != approved.keys():
        raise Error("recovered response set does not match the exact approved requests")
    recovery.check_journal(directory, plans, approved)
    return approved


def usage(rows: list[Row]) -> Row:
    result: Row = {"responses": len(rows)}
    for field in USAGE_FIELDS:
        known = [value for row in rows if isinstance(value := row.get(field), (int, float)) and not isinstance(value, bool)]
        result[field] = {"known_sum": sum(known) if known or not rows else None,
                         "known_responses": len(known), "expected_responses": len(rows)}
    return result


def audit(directory: Path) -> Row:
    directory = directory.resolve(strict=True)
    before = inventory(directory)
    manifest, config = object_file(directory / "campaign.json"), object_file(directory / "config.json")
    campaign.verify_snapshot(directory)
    plans = staging.journal(directory / "initial-plan.jsonl").rows
    matrix(manifest, config, plans)
    image = staging.string_field(object_file(directory / "grading-image.json"), "id", "grading image")
    if re.fullmatch(r"sha256:[0-9a-f]{64}", image) is None:
        raise Error("grading image must be pinned to a sha256 image ID")
    approved = recovery_approvals(directory, plans, staging.journal(directory / "initial-raw.jsonl"))
    response_ids: set[str] = set()
    initial, initial_grades = validate_stage(directory, "initial", plans, image, approved, response_ids)
    repair_plans = [bench.codex_repair_run(row, initial_grades.rows[run_id])
                    for run_id, row in plans.items() if not initial_grades.rows[run_id]["passed"]]
    repairs = {str(row["run_id"]): row for row in repair_plans}
    repair, repair_grades = validate_stage(directory, "repair", repairs, image, {}, response_ids)
    completed = [*initial.rows.values(), *repair.rows.values()]
    totals = campaign.usage(completed)
    consumption = {field: None if approved else totals[field] for field in USAGE_FIELDS}
    if before != inventory(directory):
        raise Error("campaign evidence changed during the read-only audit")
    campaign.verify_snapshot(directory)
    return {
        "schema_version": 1, "audit": "passed", "campaign": directory.name,
        "frozen_source_revision": manifest["git_revision"], "grading_image": image,
        "evidence_sha256": before, "scope": "unique saved responses; initial and repair counted once",
        "completed_response_usage": {"initial": usage(list(initial.rows.values())),
                                     "repair": usage(list(repair.rows.values())), "all": usage(completed)},
        "authorized_replacements": len(approved),
        "recovery_metadata": approved,
        "interrupted_overhead": {"attempts": len(approved),
                                 "server_completion": "unknown" if approved else "not_applicable",
                                 "tool_calls": None if approved else 0,
                                 **{field: None if approved else 0 for field in USAGE_FIELDS}},
        "total_campaign_consumption": {"complete": all(value is not None for value in consumption.values()),
                                       **consumption},
        "codex_cli_versions": sorted({str(row["codex_cli_version"]) for row in completed}),
        "four_cells": campaign.four_cells(list(initial_grades.rows.values()), list(repair_grades.rows.values())),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    try:
        result = audit(args.directory)
    except (Error, OSError, ValueError, KeyError, TypeError) as error:
        print(f"audit failed: {error}", file=sys.stderr, flush=True)
        return 1
    print(json.dumps(result, sort_keys=True, indent=2, allow_nan=False), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
