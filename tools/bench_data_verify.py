"""Run repository-owned structured-data baselines and faulty starter controls."""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import jett_bench as bench


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--language", action="append")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--starters", action="store_true")
    parser.add_argument("--output", type=Path, default=bench.DEFAULT_TARGET / "data-baseline-verification.jsonl")
    args = parser.parse_args()
    jobs = []
    for directory, task in bench.load_tasks():
        if not task["id"].startswith("data_"):
            continue
        for language, adapter in task["adapters"].items():
            if args.language and language not in args.language:
                continue
            if args.starters and "starter" not in adapter:
                continue
            jobs.append((directory, task, language, "starter" if args.starters else "baseline"))

    def grade(job: tuple) -> dict:
        directory, task, language, kind = job
        source = (directory / task["adapters"][language][kind]).read_text(encoding="utf-8")
        outcome = bench.grade_source(directory, task, language, source, 180)
        semantic_rejection = outcome["status"] == "test_failure" or (
            language == "jett" and outcome["status"] == "compile_error"
            and "error[E9000]: comptime verify failed" in outcome["diagnostic"]
        )
        return {"task_id": task["id"], "language": language, "track": kind,
                "source_sha256": bench.sha256_text(source),
                "fixture_sha256": bench.sha256_text((directory / "cases.json").read_text(encoding="utf-8")),
                "control_ok": semantic_rejection if args.starters else outcome["passed"],
                **outcome}

    outcomes = []
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        pending = [pool.submit(grade, job) for job in jobs]
        for future in as_completed(pending):
            row = future.result()
            outcomes.append(row)
            print(f"{row['task_id']}/{row['language']}: {row['status']}", flush=True)
            if not row["control_ok"]:
                print(row["diagnostic"][:2500], flush=True)
    bench.write_jsonl(args.output, sorted(outcomes, key=lambda row: (row["task_id"], row["language"])))
    succeeded = sum(row["control_ok"] for row in outcomes)
    print(f"{succeeded}/{len(outcomes)} expected outcomes; {args.output}")
    return 0 if succeeded == len(outcomes) else 1


if __name__ == "__main__":
    raise SystemExit(main())
