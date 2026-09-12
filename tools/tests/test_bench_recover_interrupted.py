import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from tools import bench_recover_interrupted as recovery

campaign, bench = recovery.campaign, recovery.bench
Row = dict[str, object]


class RecoveryTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.events = self.root / "events-initial"
        self.events.mkdir()
        self.decision_file = "recovery/approved.json"
        self.plans: list[Row] = [
            {"run_id": key, "instructions": "Instructions", "prompt": "Public prompt " + key,
             "prompt_sha256": bench.sha256_text("Instructions\nPublic prompt " + key),
             "repair_attempt": 0, "language": "python", "task_id": key, "model": "frozen",
             "reasoning_effort": "medium", "track": "zero_shot", "status": "planned", "passed": False}
            for key in "abcd"
        ]
        bench.write_jsonl(self.root / "initial-plan.jsonl", self.plans)
        campaign.write_json(self.root / "config.json", {"model": "frozen"})
        campaign.write_json(self.root / "campaign.json", {
            "input_hashes": {}, "initial_plan_sha256": campaign.digest(self.root / "initial-plan.jsonl"),
            "config_sha256": campaign.digest(self.root / "config.json"),
        })
        self.requests: list[Row] = []
        interrupted: list[Row] = []
        for plan in self.plans[:2]:
            receipt = self.events / f"original-{plan['run_id']}.attempt.json"
            campaign.write_json(receipt, {"run_id": plan["run_id"], "status": "started",
                                          "prompt_sha256": plan["prompt_sha256"]})
            self.requests.append({"run_id": plan["run_id"], "prior_attempt_filename": receipt.name,
                                  "prior_attempt_sha256": campaign.digest(receipt),
                                  "prompt_sha256": plan["prompt_sha256"]})
            interrupted.append({"run_id": plan["run_id"], "receipt": "events-initial/" + receipt.name,
                                "receipt_sha256": campaign.digest(receipt), "prompt_sha256": plan["prompt_sha256"]})
        campaign.write_json(self.root / "recovery/interrupted.json", {"interrupted_attempts": interrupted})
        self.decision: Row = {
            "schema_version": 1, "action": "repeat_lost_initial_responses", "approved_by": "user",
            "approval_text": "Yes, repeat and disclose", "recorded_at_utc": "2026-09-12T06:30:00Z",
            "expected_cli_version": recovery.CLI_VERSION, "interrupted_receipt": "recovery/interrupted.json",
            "interrupted_receipt_sha256": campaign.digest(self.root / "recovery/interrupted.json"),
            "requests": self.requests,
        }
        self.save_decision()
        self.backend_info = {"executable": "mock-codex", "version": recovery.CLI_VERSION,
                             "login": "ChatGPT subscription"}
        self.calls: list[Row] = []
        self.metrics_override: Row = {}
        output, metrics, latency, event = self.fake_call(self.plans[2], self.backend_info, self.events)
        bench.write_jsonl(self.root / "initial-raw.jsonl", [bench.result_from_codex_subscription(
            self.plans[2], self.backend_info, output, metrics, latency, event)])
        self.calls.clear()
        for mocked in (patch.object(campaign, "snapshot_inputs", return_value={}),
                       patch.dict(os.environ, {key: "" for key in recovery.API_KEYS})):
            mocked.start()
            self.addCleanup(mocked.stop)
        backend = patch.object(bench, "codex_backend_info", return_value=self.backend_info)
        self.backend = backend.start()
        self.addCleanup(backend.stop)
        call = patch.object(bench, "call_codex_subscription", side_effect=self.fake_call)
        self.call = call.start()
        self.addCleanup(call.stop)

    def save_decision(self) -> None:
        campaign.write_json(self.root / self.decision_file, self.decision)

    def fake_call(self, run: Row, backend: dict[str, str], events: Path) -> tuple[str, Row, float, str]:
        self.assertEqual(backend, self.backend_info)
        self.calls.append(dict(run))
        event = events / f"replacement-{run['run_id']}.jsonl"
        bench.write_jsonl(event, [
            {"type": "thread.started", "thread_id": "response-" + str(run["run_id"])},
            {"type": "turn.completed", "usage": {"input_tokens": 10, "output_tokens": 5,
                                                   "cached_input_tokens": 0}},
        ])
        campaign.write_json(event.with_suffix(".attempt.json"), {
            "run_id": run["run_id"], "prompt_sha256": run["prompt_sha256"], "status": "completed",
            "raw_event_log": event.name,
        })
        metrics = bench.codex_event_metrics(bench.parse_codex_events(event.read_text(encoding="utf-8")))
        return "print(1)\n", {**metrics, **self.metrics_override}, 1.0, str(event)

    def recover(self) -> int:
        return recovery.recover(self.root, self.decision_file, confirm_subscription_usage=True)

    def contents(self) -> dict[str, bytes]:
        return {str(path.relative_to(self.root)): path.read_bytes()
                for path in self.root.rglob("*") if path.is_file()}

    def rejected(self, pattern: str, *, before_backend: bool = True) -> None:
        before = self.contents()
        self.backend.reset_mock()
        self.call.reset_mock()
        with self.assertRaisesRegex(bench.BenchmarkError, pattern):
            self.recover()
        self.assertEqual(self.contents(), before)
        self.call.assert_not_called()
        if before_backend:
            self.backend.assert_not_called()

    def test_success_preserves_evidence_uses_frozen_runs_and_is_idempotent(self) -> None:
        before = self.contents()
        with patch.object(campaign, "bounded_work", wraps=campaign.bounded_work) as work:
            self.assertEqual(self.recover(), 2)
            self.assertEqual(work.call_args.args[3], 1)
        self.assertEqual(self.calls, self.plans[:2])
        after = self.contents()
        for filename, content in before.items():
            if filename == "initial-raw.jsonl":
                self.assertTrue(after[filename].startswith(content))
            else:
                self.assertEqual(after[filename], content)
        rows = campaign.read_rows(self.root / "initial-raw.jsonl")
        self.assertEqual([row["run_id"] for row in rows], ["c", "a", "b"])
        metadata = rows[1]["response_recovery"]
        self.assertEqual(metadata["decision_sha256"], campaign.digest(self.root / self.decision_file))
        self.assertEqual(metadata["prior_attempt_sha256"], self.requests[0]["prior_attempt_sha256"])
        self.assertEqual(metadata["prior_server_completion"], "unknown")
        self.assertIsNone(metadata["prior_usage"])
        self.backend.reset_mock()
        self.call.reset_mock()
        self.assertEqual(self.recover(), 0)
        self.backend.assert_not_called()
        self.call.assert_not_called()
        self.assertEqual(self.contents(), after)
        remaining = campaign.remaining_rows(self.plans, self.root / "initial-raw.jsonl", None)
        self.assertEqual([row["run_id"] for row in remaining], ["d"])
        campaign.require_unattempted(remaining, self.events)

    def test_missing_confirmation_and_cli_decision_are_rejected(self) -> None:
        with self.assertRaisesRegex(bench.BenchmarkError, "confirm-subscription"):
            recovery.recover(self.root, self.decision_file)
        with patch.object(sys, "argv", ["recovery", str(self.root), "--decision", self.decision_file]):
            self.assertEqual(recovery.main(), 1)
        with patch.object(sys, "argv", ["recovery", str(self.root), "--confirm-subscription-usage"]):
            with self.assertRaises(SystemExit) as error:
                recovery.main()
            self.assertEqual(error.exception.code, 2)
        self.backend.assert_not_called()
        self.call.assert_not_called()

    def test_unapproved_or_wrong_decisions_fail_before_backend(self) -> None:
        for field, value in (("approved_by", "agent"), ("approval_text", "maybe"),
                             ("schema_version", True), ("action", "retry"),
                             ("expected_cli_version", "other"), ("interrupted_receipt_sha256", "wrong")):
            with self.subTest(field=field):
                original = self.decision[field]
                self.decision[field] = value
                self.save_decision()
                self.rejected("approval|decision|hash mismatch")
                self.decision[field] = original

    def test_wrong_last_request_receipt_prompt_or_hash_blocks_all_calls(self) -> None:
        for field, value in (("run_id", "d"), ("prior_attempt_filename", "original-a.attempt.json"),
                             ("prior_attempt_sha256", "wrong"), ("prompt_sha256", "wrong")):
            with self.subTest(field=field):
                original = self.requests[1][field]
                self.requests[1][field] = value
                self.save_decision()
                self.rejected("set must exactly match|mismatch")
                self.requests[1][field] = original

    def test_request_set_must_be_exact_and_unique(self) -> None:
        self.decision["requests"] = self.requests[:1]
        self.save_decision()
        self.rejected("set must exactly match")
        self.decision["requests"] = [*self.requests, self.requests[0]]
        self.save_decision()
        self.rejected("duplicate run ID")
        self.decision["requests"] = [*self.requests, {**self.requests[0], "run_id": "foreign"}]
        self.save_decision()
        self.rejected("set must exactly match")

    def test_available_original_event_and_changed_receipt_are_rejected(self) -> None:
        event = self.events / "original-a.jsonl"
        event.write_bytes(b"")
        self.rejected("existing evidence")
        event.unlink()
        receipt = self.events / "original-a.attempt.json"
        receipt.write_bytes(receipt.read_bytes() + b" ")
        self.rejected("mismatch")

    def test_additional_approved_foreign_and_out_of_scope_attempts_are_rejected(self) -> None:
        for run_id in ("a", "d", "foreign", "c"):
            with self.subTest(run_id=run_id):
                campaign.write_json(self.events / "extra.attempt.json", {"run_id": run_id, "status": "started"})
                self.rejected("additional or unjournaled")

    def test_paths_cannot_escape_recovery_or_prior_stage(self) -> None:
        with self.assertRaisesRegex(bench.BenchmarkError, "beneath campaign recovery"):
            recovery.recover(self.root, "config.json", confirm_subscription_usage=True)
        with self.assertRaisesRegex(bench.BenchmarkError, "beneath campaign recovery"):
            recovery.recover(self.root, str(self.root / self.decision_file), confirm_subscription_usage=True)
        self.requests[1]["prior_attempt_filename"] = "../config.json"
        self.save_decision()
        self.rejected("stage-local")

    def test_snapshot_and_active_lock_are_enforced(self) -> None:
        lock = self.root / "active.lock"
        lock.write_bytes(b"existing owner")
        self.rejected("locked")
        lock.unlink()
        (self.root / "config.json").write_bytes(b"changed")
        self.rejected("frozen config changed")

    def test_cli_login_and_api_key_environment_are_guarded(self) -> None:
        for field, value in (("version", "different"), ("login", "API key")):
            with self.subTest(field=field):
                original = self.backend_info[field]
                self.backend_info[field] = value
                self.rejected("exact approved CLI version", before_backend=False)
                self.backend_info[field] = original
        for key in recovery.API_KEYS:
            with self.subTest(key=key), patch.dict(os.environ, {key: "never-print-this-value"}):
                self.rejected("API-key environment value present")

    def test_invalid_response_is_retained_and_never_automatically_repeated(self) -> None:
        variants: list[Row] = [{"tool_calls": 1}, {"response_id": None}, {"input_tokens": None}]
        for metrics in variants:
            with self.subTest(metrics=metrics):
                self.metrics_override = metrics
                with self.assertRaisesRegex(bench.BenchmarkError, "response preserved"):
                    self.recover()
                self.assertEqual(self.calls, self.plans[:1])
                self.assertTrue((self.events / "replacement-a.invalid.jsonl").is_file())
                self.rejected("additional or unjournaled")
                # Isolate each failure variant inside this temporary fixture only.
                for path in self.events.glob("replacement-a.*"):
                    path.unlink()
                self.calls.clear()

    def test_partial_completed_recovery_skips_only_its_approved_row(self) -> None:
        plans = {str(row["run_id"]): row for row in self.plans}
        approved = recovery.approved_requests(self.root, self.decision_file, plans)
        output, metrics, latency, event = self.fake_call(self.plans[0], self.backend_info, self.events)
        row = bench.result_from_codex_subscription(self.plans[0], self.backend_info, output, metrics, latency, event)
        bench.append_jsonl(self.root / "initial-raw.jsonl", {**row, "response_recovery": approved["a"]})
        self.calls.clear()
        self.assertEqual(self.recover(), 1)
        self.assertEqual(self.calls, self.plans[1:2])

    def test_backend_failure_receipt_prevents_a_third_attempt(self) -> None:
        def fail(run: Row, backend: dict[str, str], events: Path) -> tuple[str, Row, float, str]:
            self.calls.append(dict(run))
            campaign.write_json(events / "failed-replacement.attempt.json", {
                "run_id": run["run_id"], "prompt_sha256": run["prompt_sha256"], "status": "started"})
            raise bench.BenchmarkError("interrupted backend")

        self.call.side_effect = fail
        with self.assertRaisesRegex(bench.BenchmarkError, "interrupted backend"):
            self.recover()
        self.assertEqual(self.calls, self.plans[:1])
        self.rejected("additional or unjournaled")

    def test_completed_skip_rejects_changed_metadata_identity_metrics_and_events(self) -> None:
        self.recover()
        path = self.root / "initial-raw.jsonl"
        rows = campaign.read_rows(path)
        for field, value in (("response_recovery", {}), ("language", "changed"),
                             ("input_tokens", 99), ("backend", "other"), ("passed", True)):
            with self.subTest(field=field):
                modified = [rows[0], {**rows[1], field: value}, rows[2]]
                bench.write_jsonl(path, modified)
                self.rejected("completed recovery")
        bench.write_jsonl(path, rows)
        (self.events / "replacement-a.jsonl").write_bytes(b"changed event\n")
        self.rejected("raw event evidence changed")


if __name__ == "__main__":
    unittest.main()
