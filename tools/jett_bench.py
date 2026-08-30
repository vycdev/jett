#!/usr/bin/env python3
"""Offline-first runner for the Jett cross-language LLM benchmark."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import math
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
BENCHMARKS = ROOT / "benchmarks"
DEFAULT_CONFIG = BENCHMARKS / "config" / "pilot.json"
DEFAULT_TARGET = ROOT / "target" / "jett-bench"
API_URL = "https://api.openai.com/v1/responses"


class BenchmarkError(RuntimeError):
    pass


def read_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BenchmarkError(f"cannot read {path}: {error}") from error


def write_jsonl(path: Path, rows: Iterable[dict[str, Any]]) -> int:
    path.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with path.open("w", encoding="utf-8", newline="\n") as output:
        for row in rows:
            output.write(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n")
            count += 1
    return count


def append_jsonl(path: Path, row: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8", newline="\n") as output:
        output.write(json.dumps(row, sort_keys=True, ensure_ascii=False) + "\n")


def load_tasks() -> list[tuple[Path, dict[str, Any]]]:
    tasks = []
    for path in sorted((BENCHMARKS / "tasks").glob("*/task.json")):
        tasks.append((path.parent, read_json(path)))
    return tasks


def validate(config_path: Path = DEFAULT_CONFIG) -> list[str]:
    config = read_json(config_path)
    errors: list[str] = []
    required_config = {
        "benchmark_version",
        "subset",
        "model",
        "reasoning_efforts",
        "tracks",
        "languages",
        "repetitions",
        "max_output_tokens",
        "grader_timeout_seconds",
        "prices_usd_per_million_tokens",
    }
    missing = required_config - config.keys()
    if missing:
        errors.append(f"config missing: {', '.join(sorted(missing))}")

    languages = config.get("languages", [])
    task_ids: set[str] = set()
    tasks = load_tasks()
    if not tasks:
        errors.append("no tasks found")

    for directory, task in tasks:
        task_id = task.get("id")
        if not isinstance(task_id, str) or not task_id:
            errors.append(f"{directory}: missing task id")
            continue
        if task_id in task_ids:
            errors.append(f"duplicate task id: {task_id}")
        task_ids.add(task_id)
        if directory.name != task_id:
            errors.append(f"{task_id}: directory must match task id")
        for field in ("version", "title", "statement", "constraints", "adapters"):
            if field not in task:
                errors.append(f"{task_id}: missing {field}")
        adapters = task.get("adapters", {})
        if set(adapters) != set(languages):
            errors.append(f"{task_id}: adapters do not match configured languages")
        for language in languages:
            adapter = adapters.get(language, {})
            for field in ("signature", "baseline", "hidden", "candidate", "grade_mode", "commands"):
                if field not in adapter:
                    errors.append(f"{task_id}/{language}: missing {field}")
            for filename_field in ("baseline", "hidden"):
                filename = adapter.get(filename_field)
                if filename and not (directory / filename).is_file():
                    errors.append(f"{task_id}/{language}: missing file {filename}")
            if adapter.get("grade_mode") not in {"append", "separate"}:
                errors.append(f"{task_id}/{language}: invalid grade_mode")
            commands = adapter.get("commands", [])
            if not commands or any(not isinstance(command, list) or not command for command in commands):
                errors.append(f"{task_id}/{language}: commands must be non-empty argv arrays")

    for language in languages:
        reference = BENCHMARKS / "references" / f"{language}.md"
        if not reference.is_file():
            errors.append(f"missing onboarding reference: {reference}")

    for schema_name in ("result.schema.json", "task.schema.json"):
        schema = BENCHMARKS / "schemas" / schema_name
        if not schema.is_file():
            errors.append(f"missing schema: {schema_name}")
        else:
            read_json(schema)

    if config.get("repetitions", 0) < 1:
        errors.append("repetitions must be positive")
    if config.get("max_repair_attempts", -1) < 0:
        errors.append("max_repair_attempts must be non-negative")
    allowed_efforts = {"low", "medium", "high"}
    if not set(config.get("reasoning_efforts", [])).issubset(allowed_efforts):
        errors.append("pilot reasoning efforts must be low, medium, or high")
    if set(config.get("tracks", [])) != {"zero_shot", "onboarding"}:
        errors.append("pilot must contain zero_shot and onboarding tracks")
    return errors


def require_valid(config_path: Path) -> dict[str, Any]:
    errors = validate(config_path)
    if errors:
        raise BenchmarkError("validation failed:\n- " + "\n- ".join(errors))
    return read_json(config_path)


def render_prompt(task: dict[str, Any], language: str, track: str) -> tuple[str, str]:
    adapter = task["adapters"][language]
    instructions = (
        "Solve the programming task in the requested language. Return exactly one complete "
        "source file. A single Markdown code fence is accepted, but explanatory prose, tests, "
        "I/O, and additional files are not. Preserve the required signature exactly."
    )
    constraint_text = "\n".join(f"- {item}" for item in task["constraints"])
    prompt = (
        f"Task: {task['title']}\nTask version: {task['version']}\n"
        f"Language: {language}\n\n{task['statement']}\n\n"
        f"Required declaration:\n```\n{adapter['signature']}\n```\n\n"
        f"Constraints:\n{constraint_text}"
    )
    if track == "onboarding":
        reference = (BENCHMARKS / "references" / f"{language}.md").read_text(encoding="utf-8")
        prompt += f"\n\nLanguage reference for this track:\n\n{reference}"
    elif track != "zero_shot":
        raise BenchmarkError(f"unknown track: {track}")
    return instructions, prompt


def sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def git_revision() -> str | None:
    try:
        return subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, check=True, capture_output=True, text=True
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None


def planned_runs(config_path: Path = DEFAULT_CONFIG) -> Iterable[dict[str, Any]]:
    config = require_valid(config_path)
    revision = git_revision()
    for (_, task), language, track, effort, repetition in itertools.product(
        load_tasks(),
        config["languages"],
        config["tracks"],
        config["reasoning_efforts"],
        range(1, config["repetitions"] + 1),
    ):
        instructions, prompt = render_prompt(task, language, track)
        run_id = f"{config['benchmark_version']}:{task['id']}:{task['version']}:{language}:{track}:{effort}:{repetition:02d}"
        yield {
            "run_id": run_id,
            "benchmark_version": config["benchmark_version"],
            "subset": config["subset"],
            "task_id": task["id"],
            "task_version": task["version"],
            "adapter_version": task["version"],
            "language": language,
            "track": track,
            "model": config["model"],
            "model_snapshot": config.get("model_snapshot"),
            "reasoning_effort": effort,
            "repetition": repetition,
            "repair_attempt": 0,
            "prompt_sha256": sha256_text(instructions + "\n" + prompt),
            "reference_bytes": 0 if track == "zero_shot" else len(
                (BENCHMARKS / "references" / f"{language}.md").read_bytes()
            ),
            "git_revision": revision,
            "status": "planned",
            "passed": False,
            "instructions": instructions,
            "prompt": prompt,
        }


def response_request(run: dict[str, Any], config: dict[str, Any]) -> dict[str, Any]:
    model = config.get("model_snapshot") or config["model"]
    return {
        "model": model,
        "instructions": run["instructions"],
        "input": run["prompt"],
        "reasoning": {"effort": run["reasoning_effort"]},
        "max_output_tokens": config["max_output_tokens"],
        "store": False,
        "metadata": {
            "benchmark_version": run["benchmark_version"],
            "task": run["task_id"],
            "language": run["language"],
            "track": run["track"],
            "repetition": str(run["repetition"]),
        },
    }


def request_rows(config_path: Path = DEFAULT_CONFIG) -> Iterable[dict[str, Any]]:
    config = require_valid(config_path)
    for run in planned_runs(config_path):
        yield {"run": run, "request": response_request(run, config)}


FENCE = re.compile(r"```[^\n`]*\n(.*?)```", re.DOTALL)


def extract_source(output: str) -> str:
    blocks = FENCE.findall(output)
    if len(blocks) > 1:
        raise BenchmarkError("response contains multiple fenced blocks")
    source = blocks[0] if blocks else output
    source = source.strip()
    if not source:
        raise BenchmarkError("response contains no source")
    return source + "\n"


def source_metrics(source: str) -> dict[str, int]:
    nonempty = [line for line in source.splitlines() if line.strip()]
    branch_words = re.findall(r"\b(if|elif|else|for|while|match|case)\b|&&|\|\|", source)
    return {
        "source_bytes": len(source.encode("utf-8")),
        "source_lines": len(nonempty),
        "complexity_proxy": 1 + len(branch_words),
    }


def safe_process_environment() -> dict[str, str]:
    allowed = {
        "PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "LOCALAPPDATA",
        "APPDATA", "USERPROFILE", "CARGO_HOME", "RUSTUP_HOME", "GOROOT", "GOPATH",
        "INCLUDE", "LIB", "LIBPATH",
    }
    return {key: value for key, value in os.environ.items() if key.upper() in allowed}


def windows_rust_link_environment() -> tuple[str | None, str | None]:
    """Find the installed x64 MSVC linker and import libraries without vcvars."""
    if os.name != "nt":
        return None, None
    program_files = Path(os.environ.get("ProgramFiles", r"C:\Program Files"))
    program_files_x86 = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"))
    linkers = sorted(
        program_files.glob("Microsoft Visual Studio/2022/*/VC/Tools/MSVC/*/bin/Hostx64/x64/link.exe"),
        reverse=True,
    )
    msvc_libs = sorted(
        program_files.glob("Microsoft Visual Studio/2022/*/VC/Tools/MSVC/*/lib/x64"),
        reverse=True,
    )
    kit_versions = sorted((program_files_x86 / "Windows Kits" / "10" / "Lib").glob("10.*"), reverse=True)
    kit_libs: list[Path] = []
    if kit_versions:
        kit_libs = [kit_versions[0] / "ucrt" / "x64", kit_versions[0] / "um" / "x64"]
    libraries = [path for path in [*msvc_libs[:1], *kit_libs] if path.is_dir()]
    linker = str(linkers[0]) if linkers else None
    return linker, os.pathsep.join(str(path) for path in libraries) if libraries else None


def jett_executable() -> Path:
    configured = os.environ.get("JETT_BENCH_JETT")
    candidates = [
        Path(configured) if configured else None,
        ROOT / "target" / "debug" / ("jett.exe" if os.name == "nt" else "jett"),
    ]
    installed = shutil.which("jett")
    if installed:
        candidates.append(Path(installed))
    for candidate in candidates:
        if candidate and candidate.is_file():
            return candidate.resolve()
    raise BenchmarkError("Jett executable not found; run `cargo build -q -p jett_cli` first")


def grade_source(
    task_directory: Path,
    task: dict[str, Any],
    language: str,
    source: str,
    timeout_seconds: int,
) -> dict[str, Any]:
    adapter = task["adapters"][language]
    temp_root = DEFAULT_TARGET / "tmp"
    temp_root.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix=f"{task['id']}-{language}-", dir=temp_root) as name:
        work = Path(name)
        candidate = work / adapter["candidate"]
        hidden_source = (task_directory / adapter["hidden"]).read_text(encoding="utf-8")
        if adapter["grade_mode"] == "append":
            candidate.write_text(source.rstrip() + "\n" + hidden_source, encoding="utf-8", newline="\n")
        else:
            candidate.write_text(source, encoding="utf-8", newline="\n")
            shutil.copy2(task_directory / adapter["hidden"], work / adapter["hidden"])

        executable = work / ("hidden_test.exe" if os.name == "nt" else "hidden_test")
        replacements = {
            "{candidate}": str(candidate),
            "{executable}": str(executable),
            "{repo}": str(ROOT),
        }
        if any("{jett}" in command for command in adapter["commands"]):
            replacements["{jett}"] = str(jett_executable())
        logs = []
        compile_succeeded: bool | None = None
        for index, template in enumerate(adapter["commands"]):
            command = [replacements.get(part, part) for part in template]
            process_environment = safe_process_environment()
            if command[0] == "go":
                go_cache = work / "go-cache"
                go_cache.mkdir(exist_ok=True)
                process_environment["GOCACHE"] = str(go_cache)
            if os.name == "nt" and command[0] == "rustc" and shutil.which("link.exe") is None:
                linker, libraries = windows_rust_link_environment()
                if linker:
                    command[1:1] = ["-C", f"linker={linker}"]
                elif shutil.which("lld-link.exe"):
                    command[1:1] = ["-C", "linker=lld-link"]
                if libraries:
                    process_environment["LIB"] = libraries
            if os.name == "nt" and command[0] in {"npx", "tsc"}:
                wrapper = shutil.which(command[0]) or shutil.which(command[0] + ".cmd")
                if wrapper and wrapper.lower().endswith((".cmd", ".bat")):
                    command = [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/c", wrapper, *command[1:]]
            command_cwd = ROOT if command[0] == "cargo" else work
            command_started = time.perf_counter()
            try:
                completed = subprocess.run(
                    command,
                    cwd=command_cwd,
                    env=process_environment,
                    capture_output=True,
                    text=True,
                    encoding="utf-8",
                    errors="replace",
                    timeout=timeout_seconds,
                    check=False,
                )
            except subprocess.TimeoutExpired as error:
                return {
                    "status": "timeout",
                    "passed": False,
                    "compile_succeeded": compile_succeeded,
                    "grader_runtime_ms": (time.perf_counter() - started) * 1000,
                    "diagnostic": str(error),
                    "command_logs": logs,
                }
            except OSError as error:
                return {
                    "status": "harness_error",
                    "passed": False,
                    "compile_succeeded": compile_succeeded,
                    "grader_runtime_ms": (time.perf_counter() - started) * 1000,
                    "diagnostic": str(error),
                    "command_logs": logs,
                }
            logs.append({
                "argv": command,
                "exit_code": completed.returncode,
                "runtime_ms": (time.perf_counter() - command_started) * 1000,
                "stdout": (completed.stdout or "")[-8000:],
                "stderr": (completed.stderr or "")[-8000:],
            })
            if completed.returncode != 0:
                is_compile = index < len(adapter["commands"]) - 1
                return {
                    "status": "compile_error" if is_compile else "test_failure",
                    "passed": False,
                    "compile_succeeded": False if is_compile else compile_succeeded,
                    "grader_runtime_ms": (time.perf_counter() - started) * 1000,
                    "diagnostic": (completed.stderr or completed.stdout or "")[-8000:],
                    "command_logs": logs,
                }
            if index < len(adapter["commands"]) - 1:
                compile_succeeded = True
        return {
            "status": "passed",
            "passed": True,
            "compile_succeeded": True,
            "grader_runtime_ms": (time.perf_counter() - started) * 1000,
            "diagnostic": "",
            "command_logs": logs,
        }


def toolchain_versions() -> dict[str, str | None]:
    commands = {
        "python": ["python", "--version"],
        "node": ["node", "--version"],
        "typescript": ["tsc", "--version"],
        "go": ["go", "version"],
        "rust": ["rustc", "--version"],
    }
    versions: dict[str, str | None] = {"platform": platform.platform()}
    for name, command in commands.items():
        try:
            if os.name == "nt" and command[0] in {"npx", "tsc"}:
                wrapper = shutil.which(command[0]) or shutil.which(command[0] + ".cmd")
                if wrapper and wrapper.lower().endswith((".cmd", ".bat")):
                    command = [os.environ.get("COMSPEC", "cmd.exe"), "/d", "/c", wrapper, *command[1:]]
            result = subprocess.run(
                command, capture_output=True, text=True, encoding="utf-8", errors="replace",
                timeout=15, check=False
            )
            versions[name] = (result.stdout or result.stderr).strip() if result.returncode == 0 else None
        except (OSError, subprocess.TimeoutExpired):
            versions[name] = None
    return versions


def baseline_rows(config_path: Path) -> Iterable[dict[str, Any]]:
    config = require_valid(config_path)
    versions = toolchain_versions()
    revision = git_revision()
    for directory, task in load_tasks():
        for language in config["languages"]:
            adapter = task["adapters"][language]
            source = (directory / adapter["baseline"]).read_text(encoding="utf-8")
            result = grade_source(directory, task, language, source, config["grader_timeout_seconds"])
            yield {
                "run_id": f"baseline:{task['id']}:{task['version']}:{language}",
                "benchmark_version": config["benchmark_version"],
                "task_id": task["id"],
                "task_version": task["version"],
                "adapter_version": task["version"],
                "language": language,
                "track": "baseline",
                "model": None,
                "model_snapshot": None,
                "reasoning_effort": "none",
                "repetition": 1,
                "repair_attempt": 0,
                "prompt_sha256": "",
                "git_revision": revision,
                "toolchains": versions,
                **source_metrics(source),
                **result,
            }


def response_output(response: dict[str, Any]) -> str:
    parts = []
    for item in response.get("output", []):
        if item.get("type") != "message":
            continue
        for content in item.get("content", []):
            if content.get("type") == "output_text":
                parts.append(content.get("text", ""))
    return "".join(parts)


def response_tool_calls(response: dict[str, Any]) -> int:
    return sum(
        item.get("type") in {"function_call", "computer_call", "web_search_call", "code_interpreter_call"}
        for item in response.get("output", [])
    )


def estimated_cost(response: dict[str, Any], config: dict[str, Any]) -> float | None:
    usage = response.get("usage")
    if not usage:
        return None
    prices = config["prices_usd_per_million_tokens"]
    input_tokens = usage.get("input_tokens", 0)
    cached = usage.get("input_tokens_details", {}).get("cached_tokens", 0)
    output_tokens = usage.get("output_tokens", 0)
    uncached = max(0, input_tokens - cached)
    return (
        uncached * prices["input"] + cached * prices["cached_input"] + output_tokens * prices["output"]
    ) / 1_000_000


def result_from_response(
    envelope: dict[str, Any],
    response: dict[str, Any],
    latency_ms: float,
    config: dict[str, Any],
    grade: bool,
) -> dict[str, Any]:
    run = {key: value for key, value in envelope["run"].items() if key not in {"instructions", "prompt"}}
    usage = response.get("usage", {})
    output = response_output(response)
    result = {
        **run,
        "response_id": response.get("id"),
        "raw_output": output,
        "raw_response": response,
        "latency_ms": latency_ms,
        "input_tokens": usage.get("input_tokens"),
        "output_tokens": usage.get("output_tokens"),
        "reasoning_tokens": usage.get("output_tokens_details", {}).get("reasoning_tokens"),
        "tool_calls": response_tool_calls(response),
        "estimated_cost_usd": estimated_cost(response, config),
        "status": "generated",
        "passed": False,
    }
    try:
        source = extract_source(output)
    except BenchmarkError as error:
        return {**result, "status": "extraction_error", "diagnostic": str(error)}
    result.update(source_metrics(source))
    result["extracted_source"] = source
    if grade:
        task_directory, task = next(
            pair for pair in load_tasks() if pair[1]["id"] == run["task_id"]
        )
        result.update(grade_source(
            task_directory,
            task,
            run["language"],
            source,
            config["grader_timeout_seconds"],
        ))
    return result


def repair_envelope(
    original: dict[str, Any], prior_result: dict[str, Any], attempt: int, config: dict[str, Any]
) -> dict[str, Any]:
    """Create a fresh repair request without exposing private grader details."""
    run = dict(original["run"])
    parent_run_id = run["run_id"]
    source = prior_result.get("extracted_source", prior_result.get("raw_output", ""))
    if prior_result.get("status") == "compile_error":
        feedback = "The source did not compile against the required declaration. Re-check syntax, types, exports, and language-specific policy."
    elif prior_result.get("status") == "extraction_error":
        feedback = "The response was not one complete source file. Return only one complete source file."
    else:
        feedback = "The source compiled but failed private tests. Re-check every public requirement, boundary case, and exact output spelling."
    run["run_id"] = f"{parent_run_id}:repair{attempt:02d}"
    run["parent_run_id"] = parent_run_id
    run["repair_attempt"] = attempt
    run["prompt"] = (
        original["run"]["prompt"]
        + "\n\nRepair attempt. Here is your previous source:\n```\n"
        + source.strip()
        + "\n```\n\nNormalized feedback (private tests are intentionally withheld):\n"
        + feedback
    )
    run["prompt_sha256"] = sha256_text(run["instructions"] + "\n" + run["prompt"])
    return {"run": run, "request": response_request(run, config)}


def call_responses_api(body: dict[str, Any], api_key: str, timeout: int = 300) -> tuple[dict[str, Any], float]:
    request = urllib.request.Request(
        API_URL,
        data=json.dumps(body).encode("utf-8"),
        headers={"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"},
        method="POST",
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise BenchmarkError(f"OpenAI API returned {error.code}: {detail}") from error
    except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as error:
        raise BenchmarkError(f"OpenAI API request failed: {error}") from error
    return payload, (time.perf_counter() - started) * 1000


def pass_at_k(n: int, correct: int, k: int) -> float | None:
    if n < k or n <= 0:
        return None
    if correct == 0:
        return 0.0
    if n - correct < k:
        return 1.0
    return 1.0 - math.comb(n - correct, k) / math.comb(n, k)


def mean_present(rows: list[dict[str, Any]], field: str) -> float | None:
    values = [row[field] for row in rows if isinstance(row.get(field), (int, float))]
    return statistics.fmean(values) if values else None


def aggregate(paths: list[Path]) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    for path in paths:
        with path.open(encoding="utf-8") as source:
            rows.extend(json.loads(line) for line in source if line.strip())
    groups: dict[tuple[str, str, str, str], list[dict[str, Any]]] = defaultdict(list)
    repair_rows = [row for row in rows if row.get("repair_attempt", 0) > 0]
    for row in rows:
        if row.get("track") == "baseline" or row.get("status") == "planned" or row.get("repair_attempt", 0) > 0:
            continue
        key = (row["task_id"], row["language"], row["track"], row["reasoning_effort"])
        groups[key].append(row)
    summaries = []
    for key, group in sorted(groups.items()):
        correct = sum(bool(row.get("passed")) for row in group)
        compile_known = [row for row in group if row.get("compile_succeeded") is not None]
        summaries.append({
            "task_id": key[0],
            "language": key[1],
            "track": key[2],
            "reasoning_effort": key[3],
            "n": len(group),
            "correct": correct,
            "compile_rate": (
                sum(bool(row["compile_succeeded"]) for row in compile_known) / len(compile_known)
                if compile_known else None
            ),
            "pass_at_1": pass_at_k(len(group), correct, 1),
            "pass_at_3": pass_at_k(len(group), correct, 3),
            "pass_at_10": pass_at_k(len(group), correct, 10),
            "mean_input_tokens": mean_present(group, "input_tokens"),
            "mean_output_tokens": mean_present(group, "output_tokens"),
            "mean_reasoning_tokens": mean_present(group, "reasoning_tokens"),
            "mean_estimated_cost_usd": mean_present(group, "estimated_cost_usd"),
            "mean_latency_ms": mean_present(group, "latency_ms"),
            "mean_grader_runtime_ms": mean_present(group, "grader_runtime_ms"),
            "mean_source_lines": mean_present(group, "source_lines"),
            "mean_complexity_proxy": mean_present(group, "complexity_proxy"),
        })
    return {
        "row_count": len(rows),
        "initial_row_count": sum(row.get("repair_attempt", 0) == 0 for row in rows),
        "repair_row_count": len(repair_rows),
        "repair_pass_count": sum(bool(row.get("passed")) for row in repair_rows),
        "group_count": len(summaries),
        "groups": summaries,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("validate")

    plan_parser = subparsers.add_parser("plan")
    plan_parser.add_argument("--output", type=Path, default=DEFAULT_TARGET / "plan.jsonl")

    requests_parser = subparsers.add_parser("requests")
    requests_parser.add_argument("--output", type=Path, default=DEFAULT_TARGET / "requests.jsonl")

    baseline_parser = subparsers.add_parser("baselines")
    baseline_parser.add_argument("--output", type=Path, default=DEFAULT_TARGET / "baselines.jsonl")
    baseline_parser.add_argument("--allow-unsafe-local", action="store_true")

    api_parser = subparsers.add_parser("api-run")
    api_parser.add_argument("--output", type=Path, default=DEFAULT_TARGET / "results.jsonl")
    api_parser.add_argument("--limit", type=int, default=1)
    api_parser.add_argument("--confirm-spend", action="store_true")
    api_parser.add_argument("--allow-unpinned-model", action="store_true")
    api_parser.add_argument("--grade", action="store_true")
    api_parser.add_argument("--allow-unsafe-local", action="store_true")

    aggregate_parser = subparsers.add_parser("aggregate")
    aggregate_parser.add_argument("inputs", nargs="+", type=Path)
    aggregate_parser.add_argument("--output", type=Path, default=DEFAULT_TARGET / "summary.json")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "validate":
            errors = validate(args.config)
            if errors:
                for error in errors:
                    print(f"error: {error}", file=sys.stderr)
                return 1
            config = read_json(args.config)
            print(f"valid: {len(load_tasks())} tasks, {len(config['languages'])} languages")
            return 0

        if args.command == "plan":
            count = write_jsonl(args.output, planned_runs(args.config))
            print(f"wrote {count} planned runs to {args.output}")
            return 0

        if args.command == "requests":
            count = write_jsonl(args.output, request_rows(args.config))
            print(f"wrote {count} dry-run requests to {args.output}; no API calls made")
            return 0

        if args.command == "baselines":
            if not args.allow_unsafe_local:
                raise BenchmarkError("baselines execute code; pass --allow-unsafe-local for repository-owned fixtures")
            rows = []
            for row in baseline_rows(args.config):
                rows.append(row)
                print(f"{row['task_id']}/{row['language']}: {row['status']}")
            write_jsonl(args.output, rows)
            failed = [row for row in rows if not row["passed"]]
            print(f"wrote {len(rows)} baseline results to {args.output}")
            return 1 if failed else 0

        if args.command == "api-run":
            config = require_valid(args.config)
            if not args.confirm_spend:
                raise BenchmarkError("paid API execution requires --confirm-spend")
            if config.get("model_snapshot") is None and not args.allow_unpinned_model:
                raise BenchmarkError("set model_snapshot or pass --allow-unpinned-model for a pilot")
            if args.grade and not args.allow_unsafe_local:
                raise BenchmarkError("local grading requires --allow-unsafe-local; use an isolated runner")
            api_key = os.environ.get("OPENAI_API_KEY")
            if not api_key:
                raise BenchmarkError("OPENAI_API_KEY is not set")
            completed = 0
            for envelope in itertools.islice(request_rows(args.config), args.limit):
                try:
                    response, latency = call_responses_api(envelope["request"], api_key)
                    result = result_from_response(envelope, response, latency, config, args.grade)
                except BenchmarkError as error:
                    run = {key: value for key, value in envelope["run"].items() if key not in {"instructions", "prompt"}}
                    result = {**run, "status": "api_error", "passed": False, "diagnostic": str(error)}
                append_jsonl(args.output, result)
                completed += 1
                print(f"{result['run_id']}: {result['status']}")
                prior = result
                for attempt in range(1, config.get("max_repair_attempts", 0) + 1):
                    if not args.grade or prior.get("passed") or prior.get("status") == "api_error":
                        break
                    repair = repair_envelope(envelope, prior, attempt, config)
                    try:
                        response, latency = call_responses_api(repair["request"], api_key)
                        prior = result_from_response(repair, response, latency, config, True)
                    except BenchmarkError as error:
                        repair_run = {
                            key: value for key, value in repair["run"].items()
                            if key not in {"instructions", "prompt"}
                        }
                        prior = {**repair_run, "status": "api_error", "passed": False, "diagnostic": str(error)}
                    append_jsonl(args.output, prior)
                    completed += 1
                    print(f"{prior['run_id']}: {prior['status']}")
            print(f"appended {completed} results to {args.output}")
            return 0

        if args.command == "aggregate":
            summary = aggregate(args.inputs)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            print(f"wrote {summary['group_count']} groups to {args.output}")
            return 0
    except BenchmarkError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
