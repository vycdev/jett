"""Check generated numerical fixtures through the repository grading API."""
from __future__ import annotations

import argparse
import concurrent.futures
import json
from collections import Counter
from pathlib import Path

import jett_bench


def check(item: tuple[Path, str], timeout: int) -> dict[str, object]:
    path, language = item
    task = json.loads((path / "task.json").read_text(encoding="utf-8"))
    source = (path / task["adapters"][language]["baseline"]).read_text(encoding="utf-8")
    result = jett_bench.grade_source(path, task, language, source, timeout)
    result.update(task_id=task["id"], language=language)
    print(f"{task['id']} {language} {result['status']}", flush=True)
    if not result["passed"]:
        print(str(result.get("diagnostic", ""))[:2500], flush=True)
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--languages", default="jett,python,typescript,go,rust")
    parser.add_argument("--tasks", default="")
    parser.add_argument("--workers", type=int, default=5)
    parser.add_argument("--timeout", type=int, default=120)
    parser.add_argument("--output", default="target/jett-bench/numeric-baselines.json")
    parser.add_argument("--check-grader-mutations", action="store_true")
    args = parser.parse_args()
    if args.check_grader_mutations:
        checks = [
            ("num_collatz_steps", "go", "return MaybeInt{}", "return MaybeInt{Found: false, Value: -1}", True),
            ("num_unique_sorted", "typescript", "return output;", "if (values.length === 0) return new Array<bigint>(1);\n    return output;", False),
        ]
        for task_id, language, old, new, should_pass in checks:
            path = jett_bench.ROOT / "benchmarks/tasks" / task_id
            task = json.loads((path / "task.json").read_text(encoding="utf-8"))
            source = (path / task["adapters"][language]["baseline"]).read_text(encoding="utf-8")
            assert old in source, f"Mutation anchor changed: {task_id}"
            result = jett_bench.grade_source(path, task, language, source.replace(old, new), args.timeout)
            print(f"{task_id} {language} mutation: {result['status']}", flush=True)
            assert result["compile_succeeded"], result["diagnostic"]
            assert result["passed"] == should_pass, result["diagnostic"]
        print("Both grader mutation checks passed")
        return
    names = set(args.tasks.split(",")) if args.tasks else set()
    paths = [path for path in sorted((jett_bench.ROOT / "benchmarks/tasks").glob("num_*"))
             if not names or path.name.removeprefix("num_") in names]
    jobs = [(path, language) for path in paths for language in args.languages.split(",")]
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        results = list(pool.map(lambda job: check(job, args.timeout), jobs))
    output = jett_bench.ROOT / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print(Counter(str(result["status"]) for result in results))
    if not all(result["passed"] for result in results):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
