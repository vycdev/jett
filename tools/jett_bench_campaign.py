#!/usr/bin/env python3
"""Freeze, resume, grade, and report a subscription-only four-cell benchmark."""

from __future__ import annotations

import argparse
from concurrent.futures import FIRST_COMPLETED, Future, ThreadPoolExecutor, wait
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from typing import Any, Callable, Iterator
import uuid

try:
    from tools import jett_bench as bench
except ModuleNotFoundError:
    import jett_bench as bench

Row = dict[str, Any]
TRACKS = ("zero_shot", "skill_assisted")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_rows(path: Path) -> list[Row]:
    if not path.exists():
        return []
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    ids = [row["run_id"] for row in rows]
    if len(ids) != len(set(ids)):
        raise bench.BenchmarkError(f"duplicate run IDs in {path}")
    return rows


def write_json(path: Path, value: Row) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")


def snapshot_inputs() -> dict[str, str]:
    paths = set((bench.ROOT / "crates").rglob("*.rs"))
    paths.update((bench.ROOT / "crates").rglob("Cargo.toml"))
    paths.update((bench.ROOT / "stdlib").rglob("*.jett"))
    for directory in (bench.BENCHMARKS / "tasks", bench.SKILL_ROOT, bench.BENCHMARKS / "references"):
        paths.update(path for path in directory.rglob("*")
                     if path.is_file() and "__pycache__" not in path.parts
                     and path.suffix not in {".pyc", ".pyo"})
    paths.update((bench.ROOT / "tools").glob("jett_bench*.py"))
    paths.update([bench.ROOT / "Cargo.toml", bench.BENCHMARKS / "sandbox" / "Dockerfile"])
    lockfile = bench.ROOT / "Cargo.lock"
    if lockfile.is_file():
        paths.add(lockfile)
    return {path.relative_to(bench.ROOT).as_posix(): digest(path) for path in sorted(paths)}


def verify_snapshot(directory: Path) -> Row:
    manifest = bench.read_json(directory / "campaign.json")
    current = snapshot_inputs()
    if current != manifest["input_hashes"]:
        changed = sorted(key for key in current.keys() | manifest["input_hashes"].keys()
                         if current.get(key) != manifest["input_hashes"].get(key))
        raise bench.BenchmarkError("campaign inputs changed: " + ", ".join(changed[:8]))
    if digest(directory / "initial-plan.jsonl") != manifest["initial_plan_sha256"]:
        raise bench.BenchmarkError("frozen initial plan changed")
    if digest(directory / "config.json") != manifest["config_sha256"]:
        raise bench.BenchmarkError("frozen config changed")
    return manifest


def require_coverage(rows: list[Row], expected: list[Row], label: str) -> None:
    actual = {row["run_id"]: row for row in rows}
    wanted = {row["run_id"]: row for row in expected}
    if actual.keys() != wanted.keys():
        raise bench.BenchmarkError(
            f"{label}: expected {len(wanted)} rows, found {len(actual)}; "
            f"missing {len(wanted.keys() - actual.keys())}, extra {len(actual.keys() - wanted.keys())}"
        )
    for key, row in actual.items():
        if row.get("prompt_sha256") != wanted[key].get("prompt_sha256"):
            raise bench.BenchmarkError(f"{label}: prompt mismatch for {key}")


def row_digest(row: Row) -> str:
    return bench.sha256_text(json.dumps(row, sort_keys=True, ensure_ascii=False))


@contextmanager
def campaign_lock(directory: Path) -> Iterator[None]:
    path = directory / "active.lock"
    try:
        with path.open("x", encoding="utf-8") as handle:
            json.dump({"pid": os.getpid(), "started_at": datetime.now(timezone.utc).isoformat()}, handle)
    except FileExistsError as error:
        raise bench.BenchmarkError(f"campaign locked: {path}; inspect its PID before recovering a stale lock") from error
    try:
        yield
    finally:
        path.unlink()


def prepare(directory: Path, config_path: Path, expected_tasks: int,
            languages: list[str] | None = None, tracks: list[str] | None = None,
            baseline_directory: Path | None = None) -> None:
    config = bench.require_valid(config_path)
    tasks = bench.load_tasks()
    if len(tasks) != expected_tasks:
        raise bench.BenchmarkError(f"expected {expected_tasks} distinct tasks, found {len(tasks)}")
    if directory.exists() and any(directory.iterdir()):
        raise bench.BenchmarkError("prepare requires a new empty campaign directory")
    if config["model"] != "gpt-5.6-luna":
        raise bench.BenchmarkError("campaign model must be the requested gpt-5.6-luna")
    selected_languages = languages or config["languages"]
    selected_tracks = tracks or list(TRACKS)
    if (not set(selected_languages).issubset(config["languages"]) or
            not set(selected_tracks).issubset(TRACKS) or
            len(set(selected_languages)) != len(selected_languages) or
            len(set(selected_tracks)) != len(selected_tracks)):
        raise bench.BenchmarkError("invalid or duplicated language/context selection")
    if baseline_directory is None:
        raise bench.BenchmarkError("prepare requires --baselines from verify-baselines")
    baseline_manifest = bench.read_json(baseline_directory / "baseline-manifest.json")
    if baseline_manifest["input_hashes"] != snapshot_inputs():
        raise bench.BenchmarkError("baselines were verified against different inputs")
    baseline_results = read_rows(baseline_directory / "baseline-results.jsonl")
    baseline_cells = {(row["task_id"], row["language"]) for row in baseline_results}
    expected_baselines = {(task["id"], language) for _, task in tasks for language in config["languages"]}
    if (baseline_cells != expected_baselines or len(baseline_results) != len(expected_baselines) or
            not all(row.get("passed") for row in baseline_results) or
            digest(baseline_directory / "baseline-results.jsonl") != baseline_manifest["results_sha256"]):
        raise bench.BenchmarkError("baseline gate requires exactly one passing result per task/language")
    plans = [row for row in bench.codex_calibration_runs(config_path)
             if row["track"] in selected_tracks and row["language"] in selected_languages]
    expected = len(tasks) * len(selected_languages) * len(selected_tracks)
    if len(plans) != expected:
        raise bench.BenchmarkError(f"expected {expected} initial prompts, found {len(plans)}")
    for sequence, row in enumerate(plans, 1):
        row["sequence"] = sequence
    directory.mkdir(parents=True, exist_ok=True)
    bench.write_jsonl(directory / "initial-plan.jsonl", plans)
    write_json(directory / "config.json", config)
    write_json(directory / "campaign.json", {
        "benchmark_version": config["benchmark_version"],
        "created_at_utc": datetime.now(timezone.utc).isoformat(),
        "git_revision": bench.git_revision(), "task_count": len(tasks),
        "languages": selected_languages, "tracks": selected_tracks,
        "baseline_results_sha256": baseline_manifest["results_sha256"],
        "initial_count": expected, "max_repairs_per_failure": 1,
        "model": config["model"], "reasoning_effort": config["codex_subscription_calibration"]["reasoning_effort"],
        "input_hashes": snapshot_inputs(),
        "initial_plan_sha256": digest(directory / "initial-plan.jsonl"),
        "config_sha256": digest(directory / "config.json"),
    })
    print(f"froze {len(tasks)} tasks, {expected} initial prompts in {directory}", flush=True)


def verify_baselines(directory: Path, config_path: Path, jobs: int) -> None:
    """Execute trusted repository fixtures and bind the gate to their exact inputs."""
    config = bench.require_valid(config_path)
    before = snapshot_inputs()
    manifest_path = directory / "baseline-manifest.json"
    pending_path = directory / "baseline-inputs.json"
    if pending_path.exists() and bench.read_json(pending_path)["input_hashes"] != before:
        raise bench.BenchmarkError("baseline inputs changed; use a new baseline directory")
    write_json(pending_path, {"input_hashes": before})
    tasks = {task["id"]: (folder, task) for folder, task in bench.load_tasks()}
    plans = [{"run_id": f"baseline:{task['id']}:{task['version']}:{language}",
              "task_id": task["id"], "language": language,
              "prompt_sha256": digest(folder / task["adapters"][language]["baseline"])}
             for folder, task in tasks.values() for language in config["languages"]]
    path = directory / "baseline-results.jsonl"

    def work(row: Row) -> Row:
        folder, task = tasks[row["task_id"]]
        source = (folder / task["adapters"][row["language"]]["baseline"]).read_text(encoding="utf-8")
        assessed = bench.grade_source(folder, task, row["language"], source, config["grader_timeout_seconds"])
        if assessed["status"] == "harness_error":
            raise bench.BenchmarkError(assessed["diagnostic"])
        return {**row, **assessed}

    bounded_work(remaining_rows(plans, path, None), work, path, jobs)
    rows = read_rows(path)
    require_coverage(rows, plans, "baseline gate")
    if before != snapshot_inputs():
        raise bench.BenchmarkError("inputs changed while checking baselines")
    failed = [row for row in rows if not row.get("passed")]
    if failed:
        raise bench.BenchmarkError(f"{len(failed)}/{len(rows)} baselines failed; inspect baseline-results.jsonl")
    write_json(manifest_path, {"input_hashes": before, "results_sha256": digest(path),
                              "row_count": len(rows), "toolchains": bench.toolchain_versions()})
    print(f"baseline gate passed: {len(rows)}/{len(rows)}", flush=True)


def bounded_work(rows: list[Row], work: Callable[[Row], Row], output: Path, jobs: int) -> int:
    """Journal finished work once; stop new dispatch after an infrastructure failure."""
    if jobs < 1 or jobs > 4:
        raise bench.BenchmarkError("jobs must be between 1 and 4")
    pending: dict[Future[Row], Row] = {}
    iterator = iter(rows)
    completed = 0
    failures: list[str] = []
    with ThreadPoolExecutor(max_workers=jobs) as executor:
        def dispatch() -> None:
            row = next(iterator, None)
            if row is not None:
                pending[executor.submit(work, row)] = row

        for _ in range(jobs):
            dispatch()
        while pending:
            finished, _ = wait(pending, return_when=FIRST_COMPLETED)
            for future in finished:
                row = pending.pop(future)
                try:
                    result = future.result()
                except Exception as error:
                    failure = {"run_id": row["run_id"], "error": str(error),
                               "failed_at_utc": datetime.now(timezone.utc).isoformat()}
                    bench.append_jsonl(output.with_suffix(".errors.jsonl"), failure)
                    failures.append(str(error))
                    print(f"STOP {row['run_id']}: {error}", flush=True)
                else:
                    bench.append_jsonl(output, result)
                    completed += 1
                    print(f"[{completed}/{len(rows)}] {result['run_id']}: {result['status']}", flush=True)
            if not failures:
                for _ in finished:
                    dispatch()
    if failures:
        raise bench.BenchmarkError(f"stopped after infrastructure failure; completed rows retained: {failures[0]}")
    return completed


def remaining_rows(plans: list[Row], output: Path, limit: int | None) -> list[Row]:
    if limit is not None and limit < 1:
        raise bench.BenchmarkError("limit must be positive")
    prior = read_rows(output)
    ids = {row["run_id"] for row in prior}
    by_id = {row["run_id"]: row for row in plans}
    for row in prior:
        plan = by_id.get(row["run_id"])
        if plan is None or plan.get("prompt_sha256") != row.get("prompt_sha256"):
            raise bench.BenchmarkError("existing results do not match frozen plan")
    return [row for row in plans if row["run_id"] not in ids][:limit]


def require_unattempted(rows: list[Row], event_directory: Path) -> None:
    """Do not silently resample a call whose result was never journaled."""
    pending_ids = {row["run_id"] for row in rows}
    for path in sorted(event_directory.glob("*.attempt.json")):
        attempt = bench.read_json(path)
        if attempt.get("run_id") in pending_ids:
            raise bench.BenchmarkError(
                f"prior generation attempt has no journaled result for {attempt['run_id']}: {path}; "
                "inspect and recover the existing evidence before resuming; automatic retry refused"
            )


def verify_event_evidence(directory: Path, stage: str, rows: list[Row]) -> None:
    """Require the retained event trace bound to every generated response."""
    event_directory = (directory / f"events-{stage}").resolve()
    for row in rows:
        filename = row.get("raw_event_log")
        expected_hash = row.get("raw_event_log_sha256")
        if not isinstance(filename, str) or not filename or not isinstance(expected_hash, str):
            raise bench.BenchmarkError(f"raw event evidence metadata missing for {row['run_id']}")
        path = (event_directory / filename).resolve()
        if path.parent != event_directory or Path(filename).name != filename:
            raise bench.BenchmarkError(f"raw event evidence path is not a stage-local file: {filename}")
        if not path.is_file():
            raise bench.BenchmarkError(f"raw event evidence missing for {row['run_id']}: {path}")
        if digest(path) != expected_hash:
            raise bench.BenchmarkError(f"raw event evidence changed for {row['run_id']}: {path}")


def stage_plan(directory: Path, stage: str) -> list[Row]:
    initial = read_rows(directory / "initial-plan.jsonl")
    if stage == "initial":
        return initial
    graded = read_rows(directory / "initial-graded.jsonl")
    require_coverage(graded, initial, "initial grading before repair")
    invalid = {"planned", "generated", "backend_error", "harness_error", "api_error"}
    if any(row["status"] in invalid for row in graded):
        raise bench.BenchmarkError("initial grading contains unfinished/infrastructure rows")
    by_id = {row["run_id"]: row for row in initial}
    failures = {row["run_id"]: row for row in graded if not row.get("passed")}
    return [bench.codex_repair_run(by_id[key], failures[key]) for key in by_id if key in failures]


def generate(directory: Path, stage: str, jobs: int, limit: int | None) -> None:
    verify_snapshot(directory)
    plans = stage_plan(directory, stage)
    output_path = directory / f"{stage}-raw.jsonl"
    remaining = remaining_rows(plans, output_path, limit)
    # Check the entire unfinished stage, even when --limit selects an earlier
    # clean row. A prior call requires evidence recovery before further sampling.
    require_unattempted(remaining_rows(plans, output_path, None), directory / f"events-{stage}")
    if not remaining:
        print("generation complete; no remaining prompts", flush=True)
        return
    backend = bench.codex_backend_info()

    def work(run: Row) -> Row:
        output, metrics, latency, event_path = bench.call_codex_subscription(
            run, backend, directory / f"events-{stage}",
        )
        result = bench.result_from_codex_subscription(run, backend, output, metrics, latency, event_path)
        if metrics["tool_calls"]:
            bench.append_jsonl(Path(event_path).with_suffix(".invalid.jsonl"), result)
            raise bench.BenchmarkError("tool use invalidated the isolated treatment; event and response preserved")
        if not metrics.get("response_id") or metrics.get("input_tokens") is None:
            raise bench.BenchmarkError("response ID or input-token accounting missing")
        return result

    bounded_work(remaining, work, output_path, jobs)


def docker_grade(row: Row, image: str, directory: Path) -> Row:
    with tempfile.TemporaryDirectory(prefix="jett-grade-", dir=directory) as name:
        work = Path(name)
        bench.write_jsonl(work / "input.jsonl", [row])
        write_json(work / "config.json", bench.read_json(directory / "config.json"))
        container = f"jett-bench-{uuid.uuid4().hex}"
        command = [
            "docker", "run", "--rm", "--name", container, "--network", "none",
            "--memory", "2g", "--cpus", "2", "--pids-limit", "256",
            "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
            "--mount", f"type=bind,source={work.resolve()},target=/results",
            image, "--config", "/results/config.json", "grade-results", "/results/input.jsonl", "--output",
            "/results/output.jsonl", "--allow-unsafe-local",
        ]
        try:
            result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8",
                                    errors="replace", timeout=150, check=False)
        except subprocess.TimeoutExpired as error:
            subprocess.run(["docker", "rm", "--force", container], capture_output=True, timeout=30)
            raise bench.BenchmarkError("isolated grader container timed out") from error
        if result.returncode != 0:
            raise bench.BenchmarkError("isolated grader failed: " + (result.stderr or result.stdout)[-2000:])
        rows = read_rows(work / "output.jsonl")
        require_coverage(rows, [row], "single-container grading")
        if rows[0]["status"] in {"generated", "harness_error", "backend_error"}:
            raise bench.BenchmarkError("grader did not produce a valid assessment")
        return {**rows[0], "grading_image": image, "generation_sha256": row_digest(row)}


def grade(directory: Path, stage: str, image: str, jobs: int, limit: int | None) -> None:
    manifest = verify_snapshot(directory)
    inspection = subprocess.run(["docker", "image", "inspect", image, "--format", "{{.Id}}"],
                                capture_output=True, text=True, check=True, timeout=15)
    immutable_image = inspection.stdout.strip()
    image_record = directory / "grading-image.json"
    if image_record.exists():
        if bench.read_json(image_record)["id"] != immutable_image:
            raise bench.BenchmarkError("grading image changed within campaign")
    else:
        check = subprocess.run([
            "docker", "run", "--rm", "--network", "none", "--memory", "2g",
            "--cpus", "2", "--pids-limit", "256", "--cap-drop", "ALL",
            "--security-opt", "no-new-privileges", "--entrypoint", "python", immutable_image,
            "-c", "import json; from tools.jett_bench_campaign import snapshot_inputs; print(json.dumps(snapshot_inputs()))",
        ], capture_output=True, text=True, check=True, timeout=60)
        if json.loads(check.stdout) != manifest["input_hashes"]:
            raise bench.BenchmarkError("grading image does not contain frozen campaign inputs")
        write_json(image_record, {"id": immutable_image, "tag": image})
    plans = stage_plan(directory, stage)
    raw = read_rows(directory / f"{stage}-raw.jsonl")
    plan_ids = {row["run_id"]: row for row in plans}
    require_coverage(raw, [plan_ids[row["run_id"]] for row in raw if row["run_id"] in plan_ids], "raw grading input")
    output = directory / f"{stage}-graded.jsonl"
    raw_by_id = {row["run_id"]: row for row in raw}
    for prior in read_rows(output):
        if prior.get("generation_sha256") != row_digest(raw_by_id.get(prior["run_id"], {})):
            raise bench.BenchmarkError("generated source changed after grading")
    todo = remaining_rows(raw, output, limit)
    bounded_work(todo, lambda row: docker_grade(row, immutable_image, directory), output, jobs)


def source_characters(row: Row) -> int:
    return len(row.get("extracted_source") or "")


def usage(rows: list[Row]) -> Row:
    output: Row = {"responses": len(rows),
                   "code_bytes": sum(row.get("source_bytes", 0) for row in rows),
                   "code_chars": sum(source_characters(row) for row in rows)}
    for field in ("input_tokens", "cached_input_tokens", "output_tokens", "reasoning_tokens", "latency_ms"):
        known = [row[field] for row in rows if row.get(field) is not None]
        output[field] = sum(known) if len(known) == len(rows) else None
        output[f"{field}_known_rows"] = len(known)
    return output


def four_cells(initial: list[Row], repairs: list[Row]) -> list[Row]:
    cells = []
    for language in sorted({row["language"] for row in initial}):
        for track in TRACKS:
            group = [row for row in initial if row["language"] == language and row["track"] == track]
            if not group:
                continue
            parent_ids = {row["run_id"] for row in group if not row.get("passed")}
            attempts = [row for row in repairs if row.get("parent_run_id") in parent_ids]
            passed = sum(bool(row.get("passed")) for row in group)
            for mode in ("one_shot", "compile_repair"):
                included = group + attempts if mode == "compile_repair" else group
                final = passed + (sum(bool(row.get("passed")) for row in attempts) if mode == "compile_repair" else 0)
                cells.append({"language": language, "track": track, "mode": mode,
                              "passed": final, "n": len(group), "pass_rate": final / len(group),
                              "repair_attempts": len(attempts) if mode == "compile_repair" else 0,
                              **usage(included)})
    return cells


def report(directory: Path) -> None:
    manifest = verify_snapshot(directory)
    initial = read_rows(directory / "initial-graded.jsonl")
    repairs = read_rows(directory / "repair-graded.jsonl")
    require_coverage(initial, stage_plan(directory, "initial"), "completed initial cell")
    require_coverage(repairs, stage_plan(directory, "repair"), "completed repair cell")
    for stage, graded in (("initial", initial), ("repair", repairs)):
        raw = read_rows(directory / f"{stage}-raw.jsonl")
        require_coverage(raw, stage_plan(directory, stage), f"completed {stage} generation")
        verify_event_evidence(directory, stage, raw)
        raw_by_id = {row["run_id"]: row for row in raw}
        for row in graded:
            if row.get("generation_sha256") != row_digest(raw_by_id[row["run_id"]]):
                raise bench.BenchmarkError("generated source changed after grading")
    cells = four_cells(initial, repairs)
    summary = bench.aggregate([directory / "initial-graded.jsonl"] +
                              ([directory / "repair-graded.jsonl"] if repairs else []))
    summary["four_cells"] = cells
    write_json(directory / "summary.json", summary)
    lines = [f"# {manifest['task_count']}-problem four-cell benchmark", "",
             f"Model: {manifest['model']}; reasoning: {manifest['reasoning_effort']}; backend: Codex subscription.", "",
             "| Language | Context | Budget | Pass | Input tokens | Cached input | Output tokens | Reasoning tokens | Latency (s) | Code chars | Code bytes |",
             "| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for cell in cells:
        def count(field: str) -> str:
            value = cell[field]
            return "unavailable" if value is None else f"{value:,}"
        latency = "unavailable" if cell["latency_ms"] is None else f"{cell['latency_ms'] / 1000:,.1f}"
        lines.append(f"| {cell['language']} | {cell['track']} | {cell['mode']} | {cell['passed']}/{cell['n']} | "
                     f"{count('input_tokens')} | {count('cached_input_tokens')} | {count('output_tokens')} | "
                     f"{count('reasoning_tokens')} | {latency} | "
                     f"{count('code_chars')} | {count('code_bytes')} |")
    lines += ["", "Repair budgets include initial and repair usage; passing initial responses receive no second prompt.",
              "Code counts include every generated candidate, including failures. Reasoning tokens are included in output tokens.",
              "Cached input is a subset of input tokens. Latency sums model-call durations, not parallel campaign wall time.",
              "", "Each task/context has one observation. The model alias can change; these are calibration results.",
              "The task set is used for skill development; repeated evaluation is not an untouched held-out estimate.",
              "No API-billed requests were used; subscription allowance was consumed."]
    (directory / "REPORT.md").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote complete four-cell report: {directory / 'REPORT.md'}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("verify-baselines", "prepare", "generate", "grade", "report"))
    parser.add_argument("directory", type=Path)
    parser.add_argument("--config", type=Path, default=bench.DEFAULT_CONFIG)
    parser.add_argument("--expected-tasks", type=int, default=100)
    parser.add_argument("--baselines", type=Path)
    parser.add_argument("--language", action="append")
    parser.add_argument("--track", action="append", choices=TRACKS)
    parser.add_argument("--stage", choices=("initial", "repair"), default="initial")
    parser.add_argument("--jobs", type=int, default=1)
    parser.add_argument("--limit", type=int)
    parser.add_argument("--image")
    parser.add_argument("--confirm-subscription-usage", action="store_true")
    args = parser.parse_args()
    try:
        if args.command == "prepare":
            prepare(args.directory, args.config, args.expected_tasks, args.language, args.track, args.baselines)
        else:
            if args.command == "verify-baselines":
                args.directory.mkdir(parents=True, exist_ok=True)
            with campaign_lock(args.directory):
                if args.command == "verify-baselines":
                    verify_baselines(args.directory, args.config, args.jobs)
                elif args.command == "generate":
                    if not args.confirm_subscription_usage:
                        raise bench.BenchmarkError("generation requires --confirm-subscription-usage")
                    generate(args.directory, args.stage, args.jobs, args.limit)
                elif args.command == "grade":
                    if not args.image:
                        raise bench.BenchmarkError("grading requires --image")
                    grade(args.directory, args.stage, args.image, args.jobs, args.limit)
                else:
                    report(args.directory)
    except (bench.BenchmarkError, OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"error: {error}", flush=True)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
