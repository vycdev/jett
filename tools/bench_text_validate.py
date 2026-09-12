"""Run current baseline graders for only the text task authoring batch."""
from __future__ import annotations
import argparse
import concurrent.futures
import json
from pathlib import Path
import jett_bench as bench

def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--language", nargs="*", default=["jett", "python", "typescript", "rust", "go"])
    parser.add_argument("--limit", type=int, default=30)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--output", type=Path, default=bench.ROOT / "target/jett-bench/text-baseline-validation.json")
    args = parser.parse_args()
    jobs = [(directory, task, lang) for directory, task in bench.load_tasks() if task["id"].startswith("text_") for lang in args.language]
    selected = set(sorted({task["id"] for _, task, _ in jobs})[:args.limit])
    jobs = [job for job in jobs if job[1]["id"] in selected]
    def run(job: tuple[Path, dict, str]) -> dict:
        directory, task, lang = job
        source = (directory / task["adapters"][lang]["baseline"]).read_text(encoding="utf-8")
        outcome = bench.grade_source(directory, task, lang, source, 120)
        return {"task_id":task["id"], "language":lang, **outcome}
    outcomes = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        for outcome in pool.map(run, jobs):
            outcomes.append(outcome)
            print(outcome["task_id"], outcome["language"], outcome["status"], flush=True)
            if not outcome["passed"]:
                print(outcome["diagnostic"][-2500:], flush=True)
    output = args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(outcomes, indent=2), encoding="utf-8")
    print(f"{sum(row['passed'] for row in outcomes)}/{len(outcomes)} passed; {output}")

if __name__ == "__main__":
    main()
