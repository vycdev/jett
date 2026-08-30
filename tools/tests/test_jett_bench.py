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
        self.assertEqual(len(runs), 270)
        self.assertEqual(len({run["run_id"] for run in runs}), len(runs))
        self.assertEqual({run["reasoning_effort"] for run in runs}, {"low", "medium", "high"})

    def test_hidden_graders_never_enter_prompts(self) -> None:
        for directory, task in jett_bench.load_tasks():
            for language in task["adapters"]:
                hidden = (directory / task["adapters"][language]["hidden"]).read_text(encoding="utf-8")
                for track in ("zero_shot", "onboarding"):
                    _, prompt = jett_bench.render_prompt(task, language, track)
                    self.assertNotIn(hidden.strip(), prompt)

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

    def test_counts_response_tool_calls(self) -> None:
        response = {"output": [{"type": "message"}, {"type": "function_call"}]}
        self.assertEqual(jett_bench.response_tool_calls(response), 1)

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


if __name__ == "__main__":
    unittest.main()
