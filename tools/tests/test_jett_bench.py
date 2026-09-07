import json
import tempfile
import unittest
from pathlib import Path

from tools import jett_bench


class BenchmarkTests(unittest.TestCase):
    def test_repository_configuration_is_valid(self) -> None:
        self.assertEqual(jett_bench.validate(), [])

    def test_pilot_matrix_has_expected_size_and_unique_ids(self) -> None:
        runs = list(jett_bench.planned_runs())
        self.assertEqual(len(runs), 1350)
        self.assertEqual(len({run["run_id"] for run in runs}), len(runs))
        self.assertEqual({run["reasoning_effort"] for run in runs}, {"low", "medium", "high"})

    def test_codex_calibration_is_the_balanced_medium_slice(self) -> None:
        runs = jett_bench.codex_calibration_runs()
        self.assertEqual(len(runs), 150)
        self.assertEqual({run["reasoning_effort"] for run in runs}, {"medium"})
        self.assertEqual({run["repetition"] for run in runs}, {1})
        self.assertEqual({run["sequence"] for run in runs}, set(range(1, 151)))
        cells = {(run["task_id"], run["language"], run["track"]) for run in runs}
        self.assertEqual(len(cells), 150)

    def test_codex_calibration_can_select_one_language_track(self) -> None:
        runs = jett_bench.codex_calibration_runs(
            language="jett", track="skill_assisted"
        )
        self.assertEqual(len(runs), 10)
        self.assertEqual({run["language"] for run in runs}, {"jett"})
        self.assertEqual({run["track"] for run in runs}, {"skill_assisted"})
        self.assertEqual({run["reasoning_effort"] for run in runs}, {"medium"})
        self.assertEqual({run["sequence"] for run in runs}, set(range(1, 11)))
        self.assertEqual(len({run["task_id"] for run in runs}), 10)

    def test_programming_skills_have_parity_and_no_task_material(self) -> None:
        tasks = jett_bench.load_tasks()
        task_ids = {task["id"] for _, task in tasks}
        for language in jett_bench.LANGUAGE_SKILLS:
            bundle = jett_bench.skill_bundle(language)
            self.assertIn("## Workflow", bundle)
            self.assertIn("## Boundary", bundle)
            self.assertIn("## Verification loop", bundle)
            self.assertIn("## Provenance", bundle)
            for task_id in task_ids:
                self.assertNotIn(task_id, bundle.lower())
            for directory, task in tasks:
                adapter = task["adapters"][language]
                self.assertNotIn(task["statement"].strip(), bundle)
                self.assertNotIn(adapter["signature"].strip(), bundle)
                for constraint in task["constraints"]:
                    if len(constraint) >= 30:
                        self.assertNotIn(constraint, bundle)
                for field in ("baseline", "hidden", "starter"):
                    filename = adapter.get(field)
                    if filename:
                        source = (directory / filename).read_text(encoding="utf-8").strip()
                        self.assertNotIn(source, bundle)

            _, prompt = jett_bench.render_prompt(
                tasks[0][1], language, "skill_assisted"
            )
            self.assertIn(bundle.strip(), prompt)
            self.assertEqual(prompt.count(jett_bench.TYPE_DRIVEN_GUIDANCE), 1)

    def test_maintenance_starter_is_prompted_and_hashed(self) -> None:
        directory, task = next(
            pair for pair in jett_bench.load_tasks() if pair[1]["id"] == "account_state_evolution"
        )
        starter = (directory / task["adapters"]["jett"]["starter"]).read_text(
            encoding="utf-8"
        )
        _, prompt = jett_bench.render_prompt(task, "jett", "zero_shot")
        self.assertIn(starter.strip(), prompt)
        run = next(
            run
            for run in jett_bench.planned_runs()
            if run["task_id"] == task["id"] and run["language"] == "jett"
        )
        self.assertEqual(run["starter_sha256"], jett_bench.sha256_text(starter))

    def test_type_policy_rejects_escape_hatches_before_execution(self) -> None:
        directory, task = next(
            pair for pair in jett_bench.load_tasks() if pair[1]["id"] == "order_lifecycle"
        )
        source = (directory / "baseline.ts").read_text(encoding="utf-8")
        self.assertIsNone(
            jett_bench.source_policy_diagnostic(task["adapters"]["typescript"], source)
        )
        diagnostic = jett_bench.source_policy_diagnostic(
            task["adapters"]["typescript"], source + "\nconst bypass: any = 1;\n"
        )
        self.assertIn("type bypasses", diagnostic or "")

    def test_all_type_task_baselines_pass_source_policy(self) -> None:
        directory, task = next(
            pair for pair in jett_bench.load_tasks() if pair[1]["id"] == "order_lifecycle"
        )
        for language, adapter in task["adapters"].items():
            source = (directory / adapter["baseline"]).read_text(encoding="utf-8")
            self.assertIsNone(
                jett_bench.source_policy_diagnostic(adapter, source), language
            )

    def test_each_type_task_adapter_rejects_a_bypass(self) -> None:
        _, task = next(
            pair for pair in jett_bench.load_tasks() if pair[1]["id"] == "order_lifecycle"
        )
        bypasses = {
            "jett": "match value:\n    other:\n        return value\n",
            "python": "from typing import Any\n",
            "typescript": "const value: any = 1;\n",
            "go": "var value any\n",
            "rust": "unsafe { unreachable_unchecked() }\n",
        }
        for language, source in bypasses.items():
            self.assertIsNotNone(
                jett_bench.source_policy_diagnostic(task["adapters"][language], source),
                language,
            )

    def test_lifecycle_task_rejects_panic_and_throw_shortcuts(self) -> None:
        _, task = next(
            pair for pair in jett_bench.load_tasks() if pair[1]["id"] == "order_lifecycle"
        )
        shortcuts = {
            "python": 'raise RuntimeError("unreachable")\n',
            "typescript": 'throw new Error("unreachable");\n',
            "go": 'panic("unreachable")\n',
            "rust": 'panic!("unreachable");\n',
        }
        for language, source in shortcuts.items():
            self.assertIsNotNone(
                jett_bench.source_policy_diagnostic(task["adapters"][language], source),
                language,
            )

    def test_hidden_graders_never_enter_prompts(self) -> None:
        for directory, task in jett_bench.load_tasks():
            for language in task["adapters"]:
                hidden = (directory / task["adapters"][language]["hidden"]).read_text(encoding="utf-8")
                for track in ("zero_shot", "onboarding"):
                    _, prompt = jett_bench.render_prompt(task, language, track)
                    self.assertNotIn(hidden.strip(), prompt)

    def test_type_driven_guidance_is_equal_across_onboarding_prompts(self) -> None:
        for _, task in jett_bench.load_tasks():
            for language in task["adapters"]:
                _, onboarding = jett_bench.render_prompt(task, language, "onboarding")
                _, zero_shot = jett_bench.render_prompt(task, language, "zero_shot")
                self.assertEqual(onboarding.count(jett_bench.TYPE_DRIVEN_GUIDANCE), 1)
                self.assertNotIn(jett_bench.TYPE_DRIVEN_GUIDANCE, zero_shot)
                self.assertIn("## Type-driven development", onboarding)

    def test_extracts_one_fence_or_plain_source(self) -> None:
        self.assertEqual(jett_bench.extract_source("```rust\nfn x() {}\n```"), "fn x() {}\n")
        self.assertEqual(jett_bench.extract_source("fn x() {}"), "fn x() {}\n")
        with self.assertRaises(jett_bench.BenchmarkError):
            jett_bench.extract_source("```rust\na\n```\n```rust\nb\n```")

    def test_pass_at_k(self) -> None:
        self.assertEqual(jett_bench.pass_at_k(3, 0, 1), 0.0)
        self.assertEqual(jett_bench.pass_at_k(3, 3, 3), 1.0)
        self.assertAlmostEqual(jett_bench.pass_at_k(3, 1, 1), 1 / 3)
        self.assertIsNone(jett_bench.pass_at_k(3, 1, 10))

    def test_cost_uses_cached_and_uncached_rates(self) -> None:
        config = jett_bench.read_json(jett_bench.DEFAULT_CONFIG)
        response = {
            "usage": {
                "input_tokens": 1_000_000,
                "input_tokens_details": {"cached_tokens": 250_000},
                "output_tokens": 1_000_000,
            }
        }
        self.assertAlmostEqual(jett_bench.estimated_cost(response, config), 1.355)

    def test_repair_prompt_uses_only_normalized_feedback(self) -> None:
        config = jett_bench.read_json(jett_bench.DEFAULT_CONFIG)
        envelope = next(iter(jett_bench.request_rows()))
        prior = {
            "status": "test_failure",
            "raw_output": "source",
            "diagnostic": "SECRET_EXPECTED_VALUE",
        }
        repair = jett_bench.repair_envelope(envelope, prior, 1, config)
        self.assertNotIn("SECRET_EXPECTED_VALUE", repair["run"]["prompt"])
        self.assertIn("private tests are intentionally withheld", repair["run"]["prompt"])
        self.assertEqual(repair["run"]["repair_attempt"], 1)

    def test_codex_repair_prompt_includes_only_public_compiler_feedback(self) -> None:
        original = next(iter(jett_bench.planned_runs()))
        prior = {
            "run_id": original["run_id"],
            "response_id": "response-1",
            "status": "compile_error",
            "extracted_source": "function answer() returns int64:\n    return missing\n",
            "diagnostic": "/tmp/submission/solution.jett:2:12: undefined name: missing",
        }
        repair = jett_bench.codex_repair_run(original, prior)
        self.assertEqual(repair["evaluation_mode"], "compile_repair")
        self.assertEqual(repair["parent_run_id"], original["run_id"])
        self.assertEqual(repair["repair_feedback_kind"], "compiler_diagnostic")
        self.assertIn("undefined name: missing", repair["prompt"])

    def test_codex_repair_hides_private_grader_diagnostics(self) -> None:
        private_file = {
            "status": "compile_error",
            "extracted_source": "fn answer() {}\n",
            "diagnostic": "hidden.rs:4: SECRET_EXPECTED_VALUE",
        }
        appended_private = {
            "status": "compile_error",
            "extracted_source": "function answer():\n    return 1\n",
            "diagnostic": "solution.jett:9:4: SECRET_EXPECTED_VALUE",
        }
        for prior in (private_file, appended_private):
            kind, feedback = jett_bench.repair_feedback(prior)
            self.assertEqual(kind, "normalized_compile")
            self.assertNotIn("SECRET_EXPECTED_VALUE", feedback)

    def test_counts_response_tool_calls(self) -> None:
        response = {"output": [{"type": "message"}, {"type": "function_call"}]}
        self.assertEqual(jett_bench.response_tool_calls(response), 1)

    def test_parses_codex_subscription_metrics(self) -> None:
        events = [
            {"type": "thread.started", "thread_id": "thread-1"},
            {"type": "item.completed", "item": {"id": "tool-1", "type": "command_execution"}},
            {"type": "turn.completed", "usage": {
                "input_tokens": 100, "cached_input_tokens": 20, "output_tokens": 30
            }},
        ]
        metrics = jett_bench.codex_event_metrics(events)
        self.assertEqual(metrics["response_id"], "thread-1")
        self.assertEqual(metrics["input_tokens"], 100)
        self.assertEqual(metrics["tool_calls"], 1)
        self.assertEqual(metrics["tool_types"], ["command_execution"])

    def test_codex_environment_removes_api_billing_credentials(self) -> None:
        import os

        previous = os.environ.get("OPENAI_API_KEY")
        os.environ["OPENAI_API_KEY"] = "not-a-real-key"
        try:
            self.assertNotIn("OPENAI_API_KEY", jett_bench.codex_environment())
        finally:
            if previous is None:
                del os.environ["OPENAI_API_KEY"]
            else:
                os.environ["OPENAI_API_KEY"] = previous

    def test_aggregate_keeps_metrics_separate(self) -> None:
        rows = [
            {
                "task_id": "task",
                "language": "jett",
                "track": "zero_shot",
                "reasoning_effort": "low",
                "status": "passed",
                "passed": True,
                "compile_succeeded": True,
                "input_tokens": 100,
                "grader_runtime_ms": 2,
            },
            {
                "task_id": "task",
                "language": "jett",
                "track": "zero_shot",
                "reasoning_effort": "low",
                "status": "test_failure",
                "passed": False,
                "compile_succeeded": True,
                "input_tokens": 200,
                "grader_runtime_ms": 4,
            },
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "results.jsonl"
            path.write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
            summary = jett_bench.aggregate([path])
        group = summary["groups"][0]
        self.assertEqual(group["pass_at_1"], 0.5)
        self.assertEqual(group["compile_rate"], 1.0)
        self.assertEqual(group["mean_input_tokens"], 150.0)
        self.assertEqual(group["mean_grader_runtime_ms"], 3.0)
        rollup = summary["rollups"]["by_language_track"][0]
        self.assertEqual(rollup["n"], 2)
        self.assertEqual(rollup["passed"], 1)
        self.assertEqual(rollup["total_input_tokens"], 300)

    def test_aggregate_reports_paired_pass_after_repair(self) -> None:
        initial = {
            "run_id": "initial",
            "backend": "codex_subscription",
            "task_id": "task",
            "language": "jett",
            "track": "onboarding",
            "reasoning_effort": "medium",
            "repair_attempt": 0,
            "status": "compile_error",
            "passed": False,
            "input_tokens": 100,
            "output_tokens": 20,
        }
        repair = {
            **initial,
            "run_id": "initial:repair01",
            "parent_run_id": "initial",
            "repair_attempt": 1,
            "status": "passed",
            "passed": True,
            "input_tokens": 150,
            "output_tokens": 30,
        }
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "results.jsonl"
            path.write_text(
                json.dumps(initial) + "\n" + json.dumps(repair) + "\n",
                encoding="utf-8",
            )
            summary = jett_bench.aggregate([path])
        paired = summary["paired_repair_rollups"]["overall"][0]
        self.assertEqual(paired["initial_pass_rate"], 0.0)
        self.assertEqual(paired["repair_success_rate"], 1.0)
        self.assertEqual(paired["pass_after_repair_rate"], 1.0)
        self.assertEqual(paired["total_repair_input_tokens"], 150)


if __name__ == "__main__":
    unittest.main()
