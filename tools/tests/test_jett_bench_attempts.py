from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools import jett_bench as bench


class AttemptEvidenceTests(unittest.TestCase):
    def test_resumed_backend_failures_preserve_both_event_logs(self) -> None:
        run = {"run_id": "run", "prompt_sha256": "hash", "instructions": "instructions",
               "prompt": "prompt", "model": "gpt-5.6-luna", "reasoning_effort": "medium"}
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            with patch.object(bench.subprocess, "run", side_effect=[
                subprocess.CompletedProcess([], 1, "first event\n", "first failure"),
                subprocess.CompletedProcess([], 1, "second event\n", "second failure"),
            ]):
                for _ in range(2):
                    with self.assertRaisesRegex(bench.BenchmarkError, "exited 1"):
                        bench.call_codex_subscription(run, {"executable": "unused"}, directory)
            logs = {path.read_text(encoding="utf-8") for path in directory.glob("*.jsonl")}
            self.assertEqual(logs, {"first event\n", "second event\n"})
            attempts = [bench.read_json(path) for path in directory.glob("*.attempt.json")]
            self.assertEqual(len(attempts), 2)
            self.assertTrue(all(row["run_id"] == "run" and row["status"] == "backend_error"
                                for row in attempts))

    def test_timeout_retains_partial_usage_events_and_failure_identity(self) -> None:
        run = {"run_id": "run", "prompt_sha256": "hash", "instructions": "instructions",
               "prompt": "prompt", "model": "gpt-5.6-luna", "reasoning_effort": "medium"}
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            timeout = subprocess.TimeoutExpired("unused", 1, output=b"partial event\n", stderr=b"detail")
            with patch.object(bench.subprocess, "run", side_effect=timeout):
                with self.assertRaisesRegex(bench.BenchmarkError, "evidence"):
                    bench.call_codex_subscription(run, {"executable": "unused"}, directory)
            event = next(directory.glob("*.jsonl"))
            self.assertEqual(event.read_text(encoding="utf-8"), "partial event\n")
            attempt = bench.read_json(next(directory.glob("*.attempt.json")))
            self.assertEqual(attempt["status"], "timeout")
            self.assertEqual(attempt["prompt_sha256"], "hash")
            self.assertEqual(attempt["stderr"], "detail")


if __name__ == "__main__":
    unittest.main()
