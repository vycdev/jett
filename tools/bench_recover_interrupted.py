#!/usr/bin/env python3
"""Repeat only explicitly approved lost initial responses, preserving all prior evidence."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import os
from pathlib import Path

try:
    from tools import bench_campaign_staging as staging
except ModuleNotFoundError:
    import bench_campaign_staging as staging

campaign = staging.campaign
bench = campaign.bench
Row = dict[str, object]
Error = bench.BenchmarkError
CLI_VERSION = "codex-cli 0.153.4"
API_KEYS = ("OPENAI_API_KEY", "CODEX_API_KEY", "AZURE_OPENAI_API_KEY")


def recovery_file(directory: Path, filename: str) -> Path:
    path = (directory / filename).resolve(strict=True)
    if Path(filename).is_absolute() or not path.is_relative_to(directory / "recovery") or not path.is_file():
        raise Error("decision and interruption receipt must be files beneath campaign recovery/")
    return path


def records(row: Row, field: str) -> dict[str, Row]:
    values = row.get(field)
    if not isinstance(values, list) or not 1 <= len(values) <= 3:
        raise Error(f"{field} must contain one to three explicitly approved records")
    result: dict[str, Row] = {}
    for value in values:
        if not isinstance(value, dict):
            raise Error(f"invalid record in {field}")
        item = {key: content for key, content in value.items()}
        run_id = staging.string_field(item, "run_id", field)
        if run_id in result:
            raise Error(f"duplicate run ID in {field}")
        result[run_id] = item
    return result


def approved_requests(directory: Path, filename: str, plans: dict[str, Row]) -> dict[str, Row]:
    path = recovery_file(directory, filename)
    decision = staging.json_object(path.read_bytes(), str(path))
    if (type(decision.get("schema_version")) is not int or decision["schema_version"] != 1
            or decision.get("action") != "repeat_lost_initial_responses"
            or decision.get("approved_by") != "user"
            or decision.get("approval_text") != "Yes, repeat and disclose"
            or decision.get("expected_cli_version") != CLI_VERSION):
        raise Error("missing explicit user approval or unsupported recovery decision")
    recorded = datetime.fromisoformat(staging.string_field(decision, "recorded_at_utc", "decision"))
    if recorded.tzinfo is None or recorded.utcoffset() != timezone.utc.utcoffset(recorded):
        raise Error("decision timestamp must be UTC")
    receipt_path = recovery_file(directory, staging.string_field(decision, "interrupted_receipt", "decision"))
    if campaign.digest(receipt_path) != decision.get("interrupted_receipt_sha256"):
        raise Error("interruption receipt hash mismatch")
    interruption = staging.json_object(receipt_path.read_bytes(), str(receipt_path))
    requests, interrupted = records(decision, "requests"), records(interruption, "interrupted_attempts")
    if requests.keys() != interrupted.keys():
        raise Error("approved request set must exactly match interrupted attempts")
    metadata: dict[str, Row] = {}
    for run_id, request in requests.items():
        prior = staging.string_field(request, "prior_attempt_filename", run_id)
        prior_path = directory / "events-initial" / prior
        if (Path(prior).name != prior or "/" in prior or "\\" in prior or not prior.endswith(".attempt.json")
                or prior_path.resolve(strict=True).parent != directory / "events-initial"):
            raise Error("prior attempt must be a stage-local receipt basename")
        original, plan = interrupted[run_id], plans.get(run_id)
        if (plan is None or plan.get("repair_attempt") != 0
                or original.get("receipt") != "events-initial/" + prior
                or original.get("receipt_sha256") != request.get("prior_attempt_sha256")
                or campaign.digest(prior_path) != request.get("prior_attempt_sha256")
                or original.get("prompt_sha256") != request.get("prompt_sha256")
                or plan.get("prompt_sha256") != request.get("prompt_sha256")):
            raise Error(f"receipt or frozen initial plan mismatch: {run_id}")
        instructions = staging.string_field(plan, "instructions", run_id)
        prompt = staging.string_field(plan, "prompt", run_id)
        if bench.sha256_text(instructions + "\n" + prompt) != request.get("prompt_sha256"):
            raise Error(f"frozen prompt content/hash mismatch: {run_id}")
        attempt = staging.json_object(prior_path.read_bytes(), str(prior_path))
        event = prior_path.with_name(prior.removesuffix(".attempt.json") + ".jsonl")
        if (attempt.get("status") != "started" or attempt.get("run_id") != run_id
                or attempt.get("prompt_sha256") != request.get("prompt_sha256")
                or attempt.get("raw_event_log") is not None or event.exists()):
            raise Error(f"original attempt is not a lost started response; inspect existing evidence: {run_id}")
        metadata[run_id] = {
            "action": decision["action"], "decision_file": path.relative_to(directory).as_posix(),
            "decision_sha256": campaign.digest(path), "prior_attempt_filename": prior,
            "prior_attempt_sha256": request["prior_attempt_sha256"],
            "prior_server_completion": "unknown", "prior_usage": None,
        }
    return metadata


def check_journal(directory: Path, plans: dict[str, Row], approved: dict[str, Row]) -> dict[str, Row]:
    raw = staging.journal(directory / "initial-raw.jsonl", optional=True)
    staging.raw_matches_plan(raw, plans, "canonical initial responses")
    campaign.verify_event_evidence(directory, "initial", list(raw.rows.values()))
    originals = {item["prior_attempt_filename"] for item in approved.values()}
    journaled: set[str] = set()
    for path in sorted((directory / "events-initial").glob("*.attempt.json")):
        if path.name in originals:
            continue
        attempt = staging.json_object(path.read_bytes(), str(path))
        run_id = staging.string_field(attempt, "run_id", str(path))
        row = raw.rows.get(run_id)
        event_name = path.name.removesuffix(".attempt.json") + ".jsonl"
        if (row is None or run_id in journaled or attempt.get("status") != "completed"
                or attempt.get("prompt_sha256") != row.get("prompt_sha256")
                or attempt.get("raw_event_log") != event_name or row.get("raw_event_log") != event_name):
            raise Error(f"additional or unjournaled attempt; automatic repetition refused: {run_id}")
        journaled.add(run_id)
    for run_id, row in raw.rows.items():
        if run_id not in journaled:
            raise Error(f"journaled response has no matching completed attempt: {run_id}")
        if run_id in approved and (row.get("response_recovery") != approved[run_id]
                or row.get("status") not in {"generated", "extraction_error"}
                or row.get("passed") is not False or row.get("backend") != "codex_subscription"
                or row.get("tool_calls") != 0 or not row.get("response_id")
                or row.get("input_tokens") is None or row.get("codex_cli_version") != CLI_VERSION
                or row.get("subscription_login") != "ChatGPT subscription"):
            raise Error(f"completed recovery metadata does not match approval: {run_id}")
        if run_id in approved:
            identity = {key: value for key, value in plans[run_id].items()
                        if key not in {"instructions", "prompt", "status", "model_snapshot"}}
            event = directory / "events-initial" / staging.string_field(row, "raw_event_log", run_id)
            metrics = bench.codex_event_metrics(bench.parse_codex_events(event.read_text(encoding="utf-8")))
            if any(row.get(key) != value for key, value in {**identity, **metrics}.items()):
                raise Error(f"completed recovery frozen identity or event metrics changed: {run_id}")
    return raw.rows


def recover(directory: Path, decision: str, *, confirm_subscription_usage: bool = False) -> int:
    if not confirm_subscription_usage:
        raise Error("explicit --confirm-subscription-usage is required")
    directory = directory.resolve(strict=True)
    with campaign.campaign_lock(directory):
        campaign.verify_snapshot(directory)
        plans = staging.journal(directory / "initial-plan.jsonl").rows
        approved = approved_requests(directory, decision, plans)
        prior = check_journal(directory, plans, approved)
        pending = [row for run_id, row in plans.items() if run_id in approved and run_id not in prior]
        if not pending:
            return 0
        if any(bool(os.environ.get(key)) for key in API_KEYS):
            raise Error("API-key environment value present; subscription-only recovery refused")
        backend = bench.codex_backend_info()
        if backend.get("login") != "ChatGPT subscription" or backend.get("version") != CLI_VERSION:
            raise Error("recovery requires ChatGPT subscription and the exact approved CLI version")

        def work(run: Row) -> Row:
            output, metrics, latency, event_path = bench.call_codex_subscription(
                run, backend, directory / "events-initial")
            result = bench.result_from_codex_subscription(run, backend, output, metrics, latency, event_path)
            result["response_recovery"] = approved[staging.string_field(run, "run_id", "frozen plan")]
            if metrics.get("tool_calls") != 0 or not metrics.get("response_id") or metrics.get("input_tokens") is None:
                bench.append_jsonl(Path(event_path).with_suffix(".invalid.jsonl"), result)
                raise Error("invalid isolated response or missing accounting; event and response preserved")
            return result

        return campaign.bounded_work(pending, work, directory / "initial-raw.jsonl", 1)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--decision", required=True)
    parser.add_argument("--confirm-subscription-usage", action="store_true")
    args = parser.parse_args()
    try:
        count = recover(args.directory, args.decision, confirm_subscription_usage=args.confirm_subscription_usage)
        print(f"journaled {count} explicitly approved replacement responses; prior completion and usage remain unknown")
    except (Error, OSError, ValueError) as error:
        print(f"error: {error}", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
