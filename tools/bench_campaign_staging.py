#!/usr/bin/env python3
"""Safely snapshot campaign responses and merge staging grades; never run models or graders."""

from __future__ import annotations

import argparse
from contextlib import ExitStack
from dataclasses import dataclass
import json
import math
import os
from pathlib import Path
import re
import tempfile
from typing import Literal

try:
    from tools import jett_bench_campaign as campaign
except ModuleNotFoundError:
    import jett_bench_campaign as campaign

Row = dict[str, object]
Stage = Literal["initial", "repair"]
Error = campaign.bench.BenchmarkError
COMPLETED = {"passed", "compile_error", "policy_error", "timeout", "test_failure", "extraction_error"}


@dataclass(frozen=True)
class Journal:
    content: bytes
    rows: dict[str, Row]


def unique_object(pairs: list[tuple[str, object]]) -> Row:
    result: Row = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def finite_float(value: str) -> float:
    number = float(value)
    if not math.isfinite(number):
        raise ValueError("non-finite JSON number")
    return number


def reject_constant(value: str) -> object:
    raise ValueError(f"invalid JSON constant: {value}")


def json_object(content: bytes, label: str) -> Row:
    try:
        value: object = json.loads(content.decode("utf-8"), object_pairs_hook=unique_object,
                                   parse_float=finite_float, parse_constant=reject_constant)
    except (UnicodeError, ValueError) as error:
        raise Error(f"malformed JSON in {label}: {error}") from error
    if not isinstance(value, dict):
        raise Error(f"expected JSON object in {label}")
    return {key: item for key, item in value.items()}


def string_field(row: Row, field: str, label: str) -> str:
    value = row.get(field)
    if not isinstance(value, str) or not value:
        raise Error(f"missing or invalid {field} in {label}")
    return value


def journal(path: Path, *, optional: bool = False) -> Journal:
    """Capture bytes once, including a newline boundary while generation appends."""
    try:
        content = path.read_bytes()
    except FileNotFoundError:
        if not optional:
            raise
        content = b""
    if content and not content.endswith(b"\n"):
        raise Error(f"truncated JSONL (missing final newline): {path}")
    rows: dict[str, Row] = {}
    for number, line in enumerate(content.split(b"\n")[:-1], 1):
        label = f"{path}:{number}"
        row = json_object(line, label)
        run_id = string_field(row, "run_id", label)
        string_field(row, "prompt_sha256", label)
        if run_id in rows:
            raise Error(f"duplicate run ID in {path}: {run_id}")
        rows[run_id] = row
    return Journal(content, rows)


def directories(source: Path, staging: Path) -> tuple[Path, Path]:
    source, staging = source.resolve(strict=True), staging.resolve(strict=True)
    if not source.is_dir() or not staging.is_dir():
        raise Error("source and staging must be existing campaign directories")
    if source == staging or source in staging.parents or staging in source.parents:
        raise Error("source and staging must be distinct, nonnested directories")
    return source, staging


def same_rows(actual: dict[str, Row], expected: dict[str, Row], label: str) -> None:
    for run_id, row in actual.items():
        prior = expected.get(run_id)
        if prior is None or campaign.row_digest(row) != campaign.row_digest(prior):
            raise Error(f"{label}: changed or foreign row {run_id}")


def completed_grade(row: Row, label: str) -> None:
    status = string_field(row, "status", label)
    if status not in COMPLETED or row.get("passed") is not (status == "passed"):
        raise Error(f"{label}: grade is not a completed assessment with consistent passed/status")


def frozen_plan(source: Path, staging: Path, stage: Stage) -> tuple[dict[str, Row], str]:
    if stage not in {"initial", "repair"}:
        raise Error(f"unknown stage: {stage}")
    for filename in ("campaign.json", "config.json", "initial-plan.jsonl", "grading-image.json"):
        content = (source / filename).read_bytes()
        if content != (staging / filename).read_bytes():
            raise Error(f"frozen metadata mismatch: {filename}")
        if filename.endswith(".json"):
            metadata = json_object(content, str(source / filename))
            if filename == "campaign.json":
                hashes = metadata.get("input_hashes")
                if not isinstance(hashes, dict) or not all(isinstance(value, str) for value in hashes.values()):
                    raise Error("malformed frozen manifest: input_hashes must map paths to hashes")
                for field in ("initial_plan_sha256", "config_sha256"):
                    string_field(metadata, field, "malformed frozen manifest")
    for directory in (source, staging):
        try:
            campaign.verify_snapshot(directory)
        except (KeyError, TypeError) as error:
            raise Error(f"malformed frozen manifest in {directory}: {error}") from error
        journal(directory / "initial-plan.jsonl")
    image = string_field(json_object((source / "grading-image.json").read_bytes(), "grading image"),
                         "id", "grading image")
    if re.fullmatch(r"sha256:[0-9a-f]{64}", image) is None:
        raise Error("grading image must be pinned to a sha256 image ID")
    if stage == "repair":
        initial = journal(source / "initial-graded.jsonl")
        staged = journal(staging / "initial-graded.jsonl")
        for row in [*initial.rows.values(), *staged.rows.values()]:
            completed_grade(row, "initial grading before repair")
        if initial.rows.keys() != staged.rows.keys():
            raise Error("initial grading before repair differs between campaigns")
        same_rows(staged.rows, initial.rows, "initial grading before repair")
    plans: list[dict[str, Row]] = []
    for directory in (source, staging):
        try:
            rows = campaign.stage_plan(directory, stage)
        except (KeyError, TypeError) as error:
            raise Error(f"malformed {stage} plan in {directory}: {error}") from error
        plans.append({string_field(row, "run_id", "stage plan"): row for row in rows})
    if plans[0].keys() != plans[1].keys():
        raise Error("stage plans differ between campaigns")
    same_rows(plans[1], plans[0], "stage plan")
    return plans[0], image


def raw_matches_plan(raw: Journal, plans: dict[str, Row], label: str, *, full: bool = False) -> None:
    expected = list(plans.values()) if full else [plans[key] for key in raw.rows if key in plans]
    campaign.require_coverage(list(raw.rows.values()), expected, label)


def valid_grades(grades: Journal, raw: Journal, image: str, label: str) -> None:
    for run_id, row in grades.rows.items():
        source = raw.rows.get(run_id)
        if source is None or row.get("prompt_sha256") != source.get("prompt_sha256"):
            raise Error(f"{label}: foreign row or prompt mismatch for {run_id}")
        if row.get("generation_sha256") != campaign.row_digest(source):
            raise Error(f"{label}: generated source changed for {run_id}")
        if row.get("grading_image") != image:
            raise Error(f"{label}: grading image mismatch for {run_id}")
        completed_grade(row, f"{label}: {run_id}")


def atomic_write(path: Path, content: bytes) -> None:
    """Replace one journal after validation; keep temporary data on the same filesystem."""
    with tempfile.NamedTemporaryFile(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent,
                                     delete=False) as output:
        temporary = Path(output.name)
        try:
            output.write(content)
            output.flush()
            os.fsync(output.fileno())
        except BaseException:
            output.close()
            temporary.unlink()
            raise
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def snapshot(source: Path, staging: Path, stage: Stage) -> int:
    source, staging = directories(source, staging)
    # Generation may hold the source lock; its journal is read exactly once.
    with campaign.campaign_lock(staging):
        plans, image = frozen_plan(source, staging, stage)
        raw = journal(source / f"{stage}-raw.jsonl")
        prior = journal(staging / f"{stage}-raw.jsonl", optional=True)
        grades = journal(staging / f"{stage}-graded.jsonl", optional=True)
        raw_matches_plan(raw, plans, "source raw snapshot")
        raw_matches_plan(prior, plans, "previous staging raw")
        same_rows(prior.rows, raw.rows, "previous staging raw")
        valid_grades(grades, prior, image, "staging grades")
        campaign.require_coverage(list(grades.rows.values()), list(prior.rows.values()),
                                  "previous staging snapshot must be fully graded")
        atomic_write(staging / f"{stage}-raw.jsonl", raw.content)
    return len(raw.rows)


def merge(source: Path, staging: Path, stage: Stage) -> int:
    source, staging = directories(source, staging)
    with ExitStack() as locks:
        for directory in sorted((source, staging), key=lambda path: os.path.normcase(str(path))):
            locks.enter_context(campaign.campaign_lock(directory))
        plans, image = frozen_plan(source, staging, stage)
        raw = journal(source / f"{stage}-raw.jsonl")
        staged_raw = journal(staging / f"{stage}-raw.jsonl")
        canonical = journal(source / f"{stage}-graded.jsonl", optional=True)
        staged = journal(staging / f"{stage}-graded.jsonl", optional=True)
        raw_matches_plan(raw, plans, "complete canonical generation", full=True)
        campaign.verify_event_evidence(source, stage, list(raw.rows.values()))
        same_rows(staged_raw.rows, raw.rows, "staging raw")
        valid_grades(canonical, raw, image, "canonical grades")
        valid_grades(staged, staged_raw, image, "staging grades")
        overlap = {key: row for key, row in staged.rows.items() if key in canonical.rows}
        same_rows(overlap, canonical.rows, "overlapping grades")
        missing = [row for key, row in staged.rows.items() if key not in canonical.rows]
        if missing:
            suffix = "".join(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n" for row in missing)
            atomic_write(source / f"{stage}-graded.jsonl", canonical.content + suffix.encode("utf-8"))
    return len(missing)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("snapshot", "merge"))
    parser.add_argument("source", type=Path)
    parser.add_argument("staging", type=Path)
    parser.add_argument("--stage", required=True, choices=("initial", "repair"))
    args = parser.parse_args()
    try:
        if args.command == "snapshot":
            count = snapshot(args.source, args.staging, args.stage)
            print(f"snapshotted {count} {args.stage} raw rows; no grading performed", flush=True)
        else:
            count = merge(args.source, args.staging, args.stage)
            print(f"merged {count} missing {args.stage} grades; canonical raw/events unchanged", flush=True)
    except (Error, OSError, ValueError) as error:
        print(f"error: {error}", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
