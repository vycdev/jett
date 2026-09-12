from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools import jett_bench as bench
from tools import jett_bench_campaign as campaign


class CampaignTests(unittest.TestCase):
    def row(self, name: str, **fields: object) -> dict:
        return {"run_id": name, "prompt_sha256": name, "language": "jett",
                "track": "skill_assisted", "status": "compile_error", "passed": False,
                "input_tokens": 100, "cached_input_tokens": 30, "output_tokens": 20,
                "reasoning_tokens": 10, "source_bytes": 3, "extracted_source": "é\n",
                "latency_ms": 2.0, **fields}

    def attach_trace(self, directory: Path, stage: str, row: dict) -> dict:
        events = directory / f"events-{stage}"
        events.mkdir(parents=True, exist_ok=True)
        trace = events / (bench.sha256_text(row["run_id"]) + ".jsonl")
        trace.write_text('{"type":"turn.completed"}\n', encoding="utf-8", newline="\n")
        return {**row, "raw_event_log": trace.name, "raw_event_log_sha256": campaign.digest(trace)}

    def test_duplicate_rows_cannot_silently_change_scores(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "rows.jsonl"
            bench.write_jsonl(path, [self.row("a"), self.row("a")])
            with self.assertRaisesRegex(bench.BenchmarkError, "duplicate"):
                campaign.read_rows(path)

    def test_resume_keeps_failed_candidates_but_rejects_prompt_changes(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "rows.jsonl"
            plans = [self.row("a"), self.row("b")]
            bench.write_jsonl(path, [plans[0]])
            self.assertEqual(campaign.remaining_rows(plans, path, None), [plans[1]])
            plans[0]["prompt_sha256"] = "edited"
            with self.assertRaisesRegex(bench.BenchmarkError, "frozen plan"):
                campaign.remaining_rows(plans, path, None)

    def test_generation_refuses_unjournaled_attempts_before_any_new_call(self) -> None:
        for status in ("started", "completed", "timeout", "backend_error", "launch_error"):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as name:
                root = Path(name)
                events = root / "events-initial"
                events.mkdir()
                campaign.write_json(events / "prior.attempt.json", {
                    "run_id": "b", "prompt_sha256": "b", "status": status,
                })
                with patch.object(campaign, "verify_snapshot", return_value={}), \
                        patch.object(campaign, "stage_plan", return_value=[self.row("a"), self.row("b")]), \
                        patch.object(bench, "codex_backend_info") as backend, \
                        patch.object(bench, "call_codex_subscription") as call:
                    # b is beyond this invocation's limit; it must still stop a.
                    with self.assertRaisesRegex(bench.BenchmarkError, "automatic retry refused"):
                        campaign.generate(root, "initial", 1, 1)
                    backend.assert_not_called()
                    call.assert_not_called()
                self.assertFalse((root / "initial-raw.jsonl").exists())

    def test_journaled_attempt_does_not_prevent_continuing_other_rows(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            events = root / "events-initial"
            events.mkdir()
            campaign.write_json(events / "prior.attempt.json", {"run_id": "a", "status": "completed"})
            path = root / "initial-raw.jsonl"
            bench.write_jsonl(path, [self.row("a")])
            remaining = campaign.remaining_rows([self.row("a"), self.row("b")], path, None)
            campaign.require_unattempted(remaining, events)
            self.assertEqual([row["run_id"] for row in remaining], ["b"])

    def test_bounded_dispatch_stops_on_backend_error_and_journals_success(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "rows.jsonl"
            called = []

            def worker(row: dict) -> dict:
                called.append(row["run_id"])
                if row["run_id"] == "b":
                    raise bench.BenchmarkError("subscription limit")
                return row

            with self.assertRaisesRegex(bench.BenchmarkError, "infrastructure"):
                campaign.bounded_work([self.row(key) for key in "abcd"], worker, path, 1)
            self.assertEqual(called, ["a", "b"])
            self.assertEqual([row["run_id"] for row in campaign.read_rows(path)], ["a"])
            self.assertTrue(path.with_suffix(".errors.jsonl").exists())

    def test_parallel_journal_contains_each_row_once(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / "rows.jsonl"
            rows = [self.row(str(index)) for index in range(12)]
            self.assertEqual(campaign.bounded_work(rows, lambda row: row, path, 3), 12)
            self.assertEqual({row["run_id"] for row in campaign.read_rows(path)},
                             {row["run_id"] for row in rows})

    def test_one_shot_and_repair_usage_are_cumulative_and_count_unicode(self) -> None:
        initial = [self.row("a", passed=True), self.row("b")]
        repairs = [self.row("b:repair01", parent_run_id="b", passed=True, input_tokens=150)]
        cells = campaign.four_cells(initial, repairs)
        self.assertEqual([cell["passed"] for cell in cells], [1, 2])
        self.assertEqual([cell["input_tokens"] for cell in cells], [200, 350])
        self.assertEqual([cell["code_chars"] for cell in cells], [4, 6])
        self.assertEqual([cell["code_bytes"] for cell in cells], [6, 9])
        self.assertEqual(cells[1]["repair_attempts"], 1)

    def test_missing_token_usage_is_not_reported_as_zero(self) -> None:
        stats = campaign.usage([self.row("a"), self.row("b", reasoning_tokens=None)])
        self.assertIsNone(stats["reasoning_tokens"])
        self.assertEqual(stats["reasoning_tokens_known_rows"], 1)
        self.assertEqual(stats["input_tokens"], 200)

    def test_repair_plan_requires_complete_initial_grading(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            bench.write_jsonl(directory / "initial-plan.jsonl", [self.row("a"), self.row("b")])
            bench.write_jsonl(directory / "initial-graded.jsonl", [self.row("a")])
            with self.assertRaisesRegex(bench.BenchmarkError, "expected 2"):
                campaign.stage_plan(directory, "repair")

    def test_repair_plan_reuses_frozen_original_prompt(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            original = self.row("a", instructions="Instructions", prompt="Frozen public prompt")
            bench.write_jsonl(directory / "initial-plan.jsonl", [original])
            bench.write_jsonl(directory / "initial-graded.jsonl", [self.row("a")])
            repair = campaign.stage_plan(directory, "repair")[0]
            self.assertTrue(repair["prompt"].startswith("Frozen public prompt"))
            self.assertEqual(repair["parent_run_id"], "a")
            self.assertEqual(repair["repair_attempt"], 1)

    def test_snapshot_rejects_changed_compiler_or_skill(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            bench.write_jsonl(directory / "initial-plan.jsonl", [self.row("a")])
            campaign.write_json(directory / "config.json", {})
            campaign.write_json(directory / "campaign.json", {
                "input_hashes": {"stdlib/list.jett": "old"},
                "initial_plan_sha256": campaign.digest(directory / "initial-plan.jsonl"),
                "config_sha256": campaign.digest(directory / "config.json"),
            })
            with patch.object(campaign, "snapshot_inputs", return_value={"stdlib/list.jett": "new"}):
                with self.assertRaisesRegex(bench.BenchmarkError, "stdlib/list.jett"):
                    campaign.verify_snapshot(directory)

    def test_active_lock_prevents_overlapping_writers(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            with campaign.campaign_lock(directory):
                with self.assertRaisesRegex(bench.BenchmarkError, "locked"):
                    with campaign.campaign_lock(directory):
                        self.fail("overlapping lock acquired")
            self.assertFalse((directory / "active.lock").exists())

    def test_source_change_invalidates_grading_identity(self) -> None:
        row = self.row("a")
        changed = {**row, "extracted_source": "other source"}
        self.assertNotEqual(campaign.row_digest(row), campaign.row_digest(changed))

    def test_prepare_requires_verified_baselines(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name) / "campaign"
            config = {"model": "gpt-5.6-luna", "languages": ["jett"]}
            with patch.object(bench, "require_valid", return_value=config), \
                    patch.object(bench, "load_tasks", return_value=[(Path(name), {"id": "a"})]):
                with self.assertRaisesRegex(bench.BenchmarkError, "requires --baselines"):
                    campaign.prepare(directory, Path("unused"), 1)

    def test_baseline_gate_rejects_missing_language_even_with_matching_hash(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            baseline = root / "baseline"
            baseline.mkdir()
            bench.write_jsonl(baseline / "baseline-results.jsonl", [self.row("a", task_id="a", passed=True)])
            campaign.write_json(baseline / "baseline-manifest.json", {
                "input_hashes": {}, "results_sha256": campaign.digest(baseline / "baseline-results.jsonl"),
            })
            config = {"model": "gpt-5.6-luna", "languages": ["jett", "python"]}
            with patch.object(bench, "require_valid", return_value=config), \
                    patch.object(bench, "load_tasks", return_value=[(root, {"id": "a"})]), \
                    patch.object(campaign, "snapshot_inputs", return_value={}):
                with self.assertRaisesRegex(bench.BenchmarkError, "exactly one passing"):
                    campaign.prepare(root / "campaign", root / "unused", 1, baseline_directory=baseline)

    def test_report_refuses_missing_repairs(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            initial = self.row("a", instructions="instruction", prompt="prompt")
            bench.write_jsonl(root / "initial-plan.jsonl", [initial])
            bench.write_jsonl(root / "initial-graded.jsonl", [initial])
            with patch.object(campaign, "verify_snapshot", return_value={}):
                with self.assertRaisesRegex(bench.BenchmarkError, "completed repair cell"):
                    campaign.report(root)
            self.assertFalse((root / "REPORT.md").exists())

    def test_report_shows_cached_tokens_and_model_latency(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            initial = self.attach_trace(root, "initial", self.row("a", status="passed", passed=True, latency_ms=2500.0))
            bench.write_jsonl(root / "initial-plan.jsonl", [initial])
            bench.write_jsonl(root / "initial-raw.jsonl", [initial])
            bench.write_jsonl(root / "initial-graded.jsonl", [
                {**initial, "generation_sha256": campaign.row_digest(initial)},
            ])
            manifest = {"task_count": 1, "model": "gpt-5.6-luna", "reasoning_effort": "medium"}
            with patch.object(campaign, "verify_snapshot", return_value=manifest), \
                    patch.object(bench, "aggregate", return_value={}):
                campaign.report(root)
            report = (root / "REPORT.md").read_text(encoding="utf-8")
            self.assertIn("Cached input", report)
            self.assertIn("Latency (s)", report)
            self.assertIn("| 100 | 30 | 20 | 10 | 2.5 | 2 | 3 |", report)

    def test_report_rejects_missing_changed_or_unbound_raw_traces(self) -> None:
        for defect in ("missing", "changed", "metadata"):
            with self.subTest(defect=defect), tempfile.TemporaryDirectory() as name:
                root = Path(name)
                initial = self.attach_trace(root, "initial", self.row("a", status="passed", passed=True))
                trace = root / "events-initial" / initial["raw_event_log"]
                if defect == "missing":
                    trace.unlink()
                elif defect == "changed":
                    trace.write_text('{"type":"different"}\n', encoding="utf-8", newline="\n")
                else:
                    del initial["raw_event_log_sha256"]
                bench.write_jsonl(root / "initial-plan.jsonl", [initial])
                bench.write_jsonl(root / "initial-raw.jsonl", [initial])
                bench.write_jsonl(root / "initial-graded.jsonl", [
                    {**initial, "generation_sha256": campaign.row_digest(initial)},
                ])
                with patch.object(campaign, "verify_snapshot", return_value={}):
                    with self.assertRaisesRegex(bench.BenchmarkError, "raw event evidence"):
                        campaign.report(root)
                self.assertFalse((root / "REPORT.md").exists())
                self.assertFalse((root / "summary.json").exists())

    def test_event_evidence_is_required_for_repairs_and_cannot_escape_stage(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            repair = self.attach_trace(root, "repair", self.row("a:repair01"))
            campaign.verify_event_evidence(root, "repair", [repair])
            with self.assertRaisesRegex(bench.BenchmarkError, "missing"):
                campaign.verify_event_evidence(root, "initial", [repair])
            repair["raw_event_log"] = "../events-repair/" + repair["raw_event_log"]
            with self.assertRaisesRegex(bench.BenchmarkError, "stage-local"):
                campaign.verify_event_evidence(root, "initial", [repair])

    def test_snapshot_excludes_python_caches_like_docker_context(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            benchmarks = root / "benchmarks"
            task = benchmarks / "tasks" / "sample"
            cache = task / "__pycache__"
            cache.mkdir(parents=True)
            (cache / "baseline.cpython.pyc").write_bytes(b"cache")
            (task / "baseline.pyc").write_bytes(b"legacy cache")
            (task / "baseline.py").write_text("source", encoding="utf-8")
            (root / "Cargo.toml").write_text("manifest", encoding="utf-8")
            (benchmarks / "sandbox").mkdir()
            (benchmarks / "sandbox/Dockerfile").write_text("image", encoding="utf-8")
            with patch.object(bench, "ROOT", root), patch.object(bench, "BENCHMARKS", benchmarks), \
                    patch.object(bench, "SKILL_ROOT", root / "skills"):
                paths = campaign.snapshot_inputs()
            self.assertIn("benchmarks/tasks/sample/baseline.py", paths)
            self.assertEqual(len(paths), 3)

    def test_isolated_grader_receives_frozen_config_not_image_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            config = {"grader_timeout_seconds": 17, "benchmark_version": "frozen"}
            campaign.write_json(root / "config.json", config)
            row = self.row("a", status="generated")

            def execute(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                self.assertIn("/results/config.json", command)
                self.assertLess(command.index("--config"), command.index("grade-results"))
                mount = command[command.index("--mount") + 1]
                work = Path(mount.removeprefix("type=bind,source=").removesuffix(",target=/results"))
                self.assertEqual(bench.read_json(work / "config.json"), config)
                bench.write_jsonl(work / "output.jsonl", [{**row, "status": "passed", "passed": True}])
                return subprocess.CompletedProcess(command, 0, "", "")

            with patch.object(campaign.subprocess, "run", side_effect=execute):
                assessed = campaign.docker_grade(row, "frozen-image", root)
            self.assertTrue(assessed["passed"])


if __name__ == "__main__":
    unittest.main()
