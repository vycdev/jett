from contextlib import ExitStack, redirect_stderr, redirect_stdout
from io import StringIO
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from tools import bench_campaign_audit as auditor

campaign, bench, recovery = auditor.campaign, auditor.bench, auditor.recovery
Row = dict[str, object]


class CampaignAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.image = "sha256:" + "a" * 64
        self.backend = {"version": recovery.CLI_VERSION, "login": "ChatGPT subscription"}
        inputs = patch.object(campaign, "snapshot_inputs", return_value={})
        inputs.start()
        self.addCleanup(inputs.stop)
        self.build()

    def build(self, task_count: int = 2, languages: tuple[str, ...] = ("jett", "python"),
              tracks: tuple[str, ...] = ("zero_shot", "skill_assisted"), *, failures: bool = True) -> None:
        self.plans: list[Row] = []
        for task in range(task_count):
            for language in languages:
                for track in tracks:
                    task_id = f"task{task:03}"
                    prompt = f"Public fixture prompt {task_id} {language} {track}"
                    self.plans.append({
                        "run_id": f"fixture:1:{task_id}:1.0.0:{language}:{track}:medium:01",
                        "benchmark_version": "fixture:1", "task_id": task_id, "task_version": "1.0.0",
                        "adapter_version": "1.0.0", "language": language, "track": track,
                        "model": "gpt-5.6-luna", "model_snapshot": "overwritten-by-subscription",
                        "reasoning_effort": "medium", "git_revision": "frozen-revision",
                        "instructions": "Instructions", "prompt": prompt,
                        "prompt_sha256": bench.sha256_text("Instructions\n" + prompt),
                        "status": "planned", "passed": False, "repair_attempt": 0, "repetition": 1,
                        "evaluation_mode": "one_shot", "sequence": len(self.plans) + 1,
                    })
        bench.write_jsonl(self.root / "initial-plan.jsonl", self.plans)
        campaign.write_json(self.root / "config.json", {
            "model": "gpt-5.6-luna", "benchmark_version": "fixture:1", "languages": list(languages)})
        campaign.write_json(self.root / "campaign.json", {
            "input_hashes": {}, "initial_plan_sha256": campaign.digest(self.root / "initial-plan.jsonl"),
            "config_sha256": campaign.digest(self.root / "config.json"), "task_count": task_count,
            "initial_count": len(self.plans), "max_repairs_per_failure": 1,
            "languages": list(languages), "tracks": list(tracks), "model": "gpt-5.6-luna",
            "benchmark_version": "fixture:1", "git_revision": "frozen-revision", "reasoning_effort": "medium",
        })
        campaign.write_json(self.root / "grading-image.json", {"id": self.image})
        initial = self.write_stage("initial", self.plans, failures=failures)
        repairs = [bench.codex_repair_run(plan, grade) for plan, grade in zip(self.plans, initial)
                   if not grade["passed"]]
        self.write_stage("repair", repairs)

    def grade(self, raw: Row, passed: bool = True) -> Row:
        return {**raw, "status": "passed" if passed else "compile_error", "passed": passed,
                "generation_status": raw["status"], "diagnostic": "" if passed else "fixture compile diagnostic",
                "generation_sha256": campaign.row_digest(raw), "grading_image": self.image,
                "compile_succeeded": passed, "grader_runtime_ms": 1.0, "command_logs": []}

    def write_stage(self, stage: str, plans: list[Row], *, failures: bool = False) -> list[Row]:
        events = self.root / f"events-{stage}"
        events.mkdir(exist_ok=True)
        raw: list[Row] = []
        for index, plan in enumerate(plans):
            event = events / f"response-{index}.jsonl"
            output = "fixture source\n"
            trace: list[Row] = [
                {"type": "thread.started", "thread_id": f"response-{stage}-{index}"},
                {"type": "turn.started"},
                {"type": "item.completed", "item": {"type": "agent_message", "id": "message", "text": output}},
                {"type": "turn.completed", "usage": {"input_tokens": 10, "cached_input_tokens": 2,
                                                       "output_tokens": 5, "reasoning_output_tokens": 1}},
            ]
            bench.write_jsonl(event, trace)
            campaign.write_json(event.with_suffix(".attempt.json"), {
                "run_id": plan["run_id"], "prompt_sha256": plan["prompt_sha256"],
                "status": "completed", "raw_event_log": event.name})
            metrics = bench.codex_event_metrics(trace)
            raw.append(bench.result_from_codex_subscription(plan, self.backend, output, metrics, 2.0, str(event)))
        graded = [self.grade(row, not (failures and index % 4 == 0)) for index, row in enumerate(raw)]
        bench.write_jsonl(self.root / f"{stage}-raw.jsonl", raw)
        bench.write_jsonl(self.root / f"{stage}-graded.jsonl", graded)
        return graded

    def rows(self, stage: str = "initial", kind: str = "raw") -> list[Row]:
        return campaign.read_rows(self.root / f"{stage}-{kind}.jsonl")

    def write_rows(self, rows: list[Row], stage: str = "initial", kind: str = "raw") -> None:
        bench.write_jsonl(self.root / f"{stage}-{kind}.jsonl", rows)

    def rebind_grades(self, stage: str = "initial") -> None:
        self.write_rows([self.grade(row, bool(grade["passed"])) for row, grade in zip(self.rows(stage), self.rows(stage, "graded"))],
                        stage, "graded")

    def rebind_event_hash(self, index: int = 0, stage: str = "initial") -> None:
        rows = self.rows(stage)
        rows[index]["raw_event_log_sha256"] = campaign.digest(self.root / f"events-{stage}/response-{index}.jsonl")
        self.write_rows(rows, stage)
        self.rebind_grades(stage)

    def contents(self) -> dict[str, bytes | None]:
        return {path.relative_to(self.root).as_posix(): path.read_bytes() if path.is_file() else None
                for path in self.root.rglob("*")}

    def checked_audit(self) -> Row:
        before = self.contents()
        try:
            with ExitStack() as stack:
                for module, name in ((bench, "call_codex_subscription"), (bench, "codex_backend_info"),
                                     (bench, "grade_source"), (bench, "append_jsonl"), (bench, "write_jsonl"),
                                     (campaign, "campaign_lock"), (campaign, "write_json"), (campaign, "report"),
                                     (campaign.subprocess, "run")):
                    stack.enter_context(patch.object(module, name, side_effect=AssertionError(f"forbidden audit call: {name}")))
                return auditor.audit(self.root)
        finally:
            self.assertEqual(self.contents(), before)

    def rejected(self, pattern: str) -> None:
        with self.assertRaisesRegex((bench.BenchmarkError, OSError, ValueError), pattern):
            self.checked_audit()

    def add_recovery(self) -> None:
        raw = self.rows()
        requests: list[Row] = []
        interrupted: list[Row] = []
        for index, row in enumerate(raw[:2]):
            receipt = self.root / f"events-initial/original-{index}.attempt.json"
            campaign.write_json(receipt, {"run_id": row["run_id"], "prompt_sha256": row["prompt_sha256"], "status": "started"})
            requests.append({"run_id": row["run_id"], "prompt_sha256": row["prompt_sha256"],
                             "prior_attempt_filename": receipt.name, "prior_attempt_sha256": campaign.digest(receipt)})
            interrupted.append({"run_id": row["run_id"], "prompt_sha256": row["prompt_sha256"],
                                "receipt": "events-initial/" + receipt.name, "receipt_sha256": campaign.digest(receipt)})
        campaign.write_json(self.root / "recovery/interruption.json", {"interrupted_attempts": interrupted})
        campaign.write_json(self.root / "recovery/decision.json", {
            "schema_version": 1, "action": "repeat_lost_initial_responses", "approved_by": "user",
            "approval_text": "Yes, repeat and disclose", "recorded_at_utc": "2026-09-12T10:00:00Z",
            "expected_cli_version": recovery.CLI_VERSION, "requests": requests,
            "interrupted_receipt": "recovery/interruption.json",
            "interrupted_receipt_sha256": campaign.digest(self.root / "recovery/interruption.json")})
        plans = {str(plan["run_id"]): plan for plan in self.plans}
        approved = recovery.approved_requests(self.root, "recovery/decision.json", plans)
        for row in raw[:2]:
            row["response_recovery"] = approved[str(row["run_id"])]
        self.write_rows(raw)
        self.rebind_grades()

    def test_complete_campaign_has_unique_accounting_and_all_cells(self) -> None:
        result = self.checked_audit()
        self.assertEqual(result["audit"], "passed")
        usage = result["completed_response_usage"]
        self.assertIsInstance(usage, dict)
        if isinstance(usage, dict):
            self.assertEqual(usage["all"]["responses"], 10)
            self.assertEqual(usage["all"]["input_tokens"]["known_sum"], 100)
        cells = result["four_cells"]
        self.assertIsInstance(cells, list)
        if isinstance(cells, list):
            self.assertEqual(len(cells), 8)
            self.assertEqual(sum(cell["input_tokens"] for cell in cells), 180)
        self.assertEqual(result["authorized_replacements"], 0)
        self.assertEqual(result["total_campaign_consumption"], {
            "complete": True, "input_tokens": 100, "cached_input_tokens": 20,
            "output_tokens": 50, "reasoning_tokens": 10, "latency_ms": 20.0})

    def test_100_row_jett_skill_followup_without_repairs_or_recoveries(self) -> None:
        with tempfile.TemporaryDirectory() as name:
            self.root = Path(name).resolve()
            self.build(100, ("jett",), ("skill_assisted",), failures=False)
            (self.root / "repair-raw.jsonl").unlink()
            (self.root / "repair-graded.jsonl").unlink()
            result = self.checked_audit()
            self.assertEqual(result["authorized_replacements"], 0)
            cells = result["four_cells"]
            if not isinstance(cells, list):
                self.fail("missing cells")
            self.assertEqual(len(cells), 2)
            self.assertTrue(all(cell["n"] == 100 for cell in cells))

    def test_recovery_keeps_unknown_overhead_null_and_counts_replacements_once(self) -> None:
        self.add_recovery()
        result = self.checked_audit()
        self.assertEqual(result["authorized_replacements"], 2)
        self.assertEqual(result["interrupted_overhead"], {"attempts": 2, "server_completion": "unknown",
                         "tool_calls": None, **{field: None for field in auditor.USAGE_FIELDS}})
        self.assertEqual(result["total_campaign_consumption"], {
            "complete": False, **{field: None for field in auditor.USAGE_FIELDS}})
        metadata = result["recovery_metadata"]
        self.assertIsInstance(metadata, dict)
        if isinstance(metadata, dict):
            self.assertEqual(len(metadata), 2)

    def test_missing_initial_or_repair_rows_fail(self) -> None:
        for stage, kind in (("initial", "raw"), ("initial", "graded"), ("repair", "raw"), ("repair", "graded")):
            with self.subTest(stage=stage, kind=kind):
                rows = self.rows(stage, kind)
                self.write_rows(rows[:-1], stage, kind)
                self.rejected("expected")
                self.write_rows(rows, stage, kind)

    def test_matrix_rejects_missing_duplicate_cell_with_unchanged_row_count(self) -> None:
        self.plans[1]["language"] = self.plans[0]["language"]
        self.plans[1]["track"] = self.plans[0]["track"]
        bench.write_jsonl(self.root / "initial-plan.jsonl", self.plans)
        manifest = auditor.object_file(self.root / "campaign.json")
        manifest["initial_plan_sha256"] = campaign.digest(self.root / "initial-plan.jsonl")
        campaign.write_json(self.root / "campaign.json", manifest)
        self.rejected("duplicate, foreign")

    def test_manifest_matrix_counts_and_types_are_strict(self) -> None:
        original = auditor.object_file(self.root / "campaign.json")
        for field, value in (("task_count", True), ("initial_count", 7), ("languages", ["jett", "jett"]),
                             ("tracks", ["onboarding"]), ("max_repairs_per_failure", 2)):
            with self.subTest(field=field):
                campaign.write_json(self.root / "campaign.json", {**original, field: value})
                self.rejected("integer|matrix|duplicate|unsupported")

    def test_grades_require_completed_status_boolean_pass_and_pinned_image(self) -> None:
        original = self.rows("repair", "graded")
        for field, value in (("status", "generated"), ("passed", 1), ("grading_image", "sha256:" + "b" * 64)):
            with self.subTest(field=field):
                self.write_rows([{**original[0], field: value}, *original[1:]], "repair", "graded")
                self.rejected("completed assessment|grading image")

    def test_grade_cannot_change_identity_or_usage_with_valid_raw_hash(self) -> None:
        original = self.rows(kind="graded")
        for field, value in (("language", "rust"), ("input_tokens", 999), ("raw_output", "different"),
                             ("generation_status", "other")):
            with self.subTest(field=field):
                self.write_rows([{**original[0], field: value}, *original[1:]], kind="graded")
                self.rejected("grade changed|generation status")

    def test_repair_parent_cannot_be_changed_to_an_initial_pass(self) -> None:
        rows = self.rows("repair")
        rows[0]["parent_run_id"] = self.plans[1]["run_id"]
        self.write_rows(rows, "repair")
        self.rebind_grades("repair")
        self.rejected("raw response identity")

    def test_raw_usage_identity_source_and_subscription_backend_are_bound(self) -> None:
        original = self.rows()
        for field, value in (("input_tokens", 11), ("input_tokens", True), ("model", "other"),
                             ("extracted_source", "changed"), ("subscription_login", "API key"),
                             ("backend", "other"), ("latency_ms", -1)):
            with self.subTest(field=field, value=value):
                self.write_rows([{**original[0], field: value}, *original[1:]])
                self.rebind_grades()
                self.rejected("raw response identity|invalid saved-response latency")

    def test_unexplained_attempt_or_event_sidecars_are_rejected(self) -> None:
        for filename in ("extra.attempt.json", "extra.jsonl", "response-0.invalid.jsonl"):
            with self.subTest(filename=filename):
                path = self.root / "events-repair" / filename
                path.write_bytes(b"{}\n")
                self.rejected("unexplained")
                path.unlink()

    def test_attempt_receipt_status_and_mapping_are_required(self) -> None:
        path = self.root / "events-initial/response-0.attempt.json"
        original = auditor.object_file(path)
        for field, value in (("status", "started"), ("run_id", "foreign"), ("raw_event_log", "other.jsonl")):
            with self.subTest(field=field):
                campaign.write_json(path, {**original, field: value})
                self.rejected("completed attempt receipt")

    def test_strict_events_reject_bad_json_missing_completion_and_tools(self) -> None:
        path = self.root / "events-initial/response-0.jsonl"
        original = path.read_bytes()
        variants = [b'{"type":"thread.started","type":"turn.completed"}\n', b'not-json\n',
                    b'{"type":"thread.started"}\n', original.rstrip(b"\n")]
        tool: Row = {"type": "item.completed", "item": {"type": "command_execution", "id": "tool"}}
        variants.append(original + (json.dumps(tool) + "\n").encode())
        for content in variants:
            with self.subTest(content=content[:35]):
                path.write_bytes(content)
                self.rebind_event_hash()
                self.rejected("malformed JSON|completed response|truncated|tool use")

    def test_event_accounting_requires_integer_tokens_and_response_id(self) -> None:
        path = self.root / "events-initial/response-0.jsonl"
        original = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        for field, value in (("input_tokens", None), ("input_tokens", True), ("output_tokens", -1),
                             ("cached_input_tokens", 11), ("reasoning_output_tokens", 6)):
            with self.subTest(field=field, value=value):
                trace = [*original[:-1], {**original[-1], "usage": {**original[-1]["usage"], field: value}}]
                bench.write_jsonl(path, trace)
                self.rebind_event_hash()
                self.rejected("invalid integer|subset exceeds")
        bench.write_jsonl(path, [{**original[0], "thread_id": ""}, *original[1:]])
        self.rebind_event_hash()
        self.rejected("response_id")

    def test_raw_answer_must_match_retained_final_event(self) -> None:
        rows = self.rows()
        rows[0]["raw_output"] = "different output"
        self.write_rows(rows)
        self.rebind_grades()
        self.rejected("final event message")

    def test_missing_optional_metric_preserves_known_subtotal_and_null_total(self) -> None:
        path = self.root / "events-initial/response-0.jsonl"
        trace = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        for field in ("cached_input_tokens", "output_tokens", "reasoning_output_tokens"):
            del trace[-1]["usage"][field]
        bench.write_jsonl(path, trace)
        raw = self.rows()
        for field in ("cached_input_tokens", "output_tokens", "reasoning_tokens"):
            raw[0][field] = None
        self.write_rows(raw)
        self.rebind_event_hash()
        result = self.checked_audit()
        totals = result["total_campaign_consumption"]
        if not isinstance(totals, dict):
            self.fail("missing accounting")
        for field in ("cached_input_tokens", "output_tokens", "reasoning_tokens"):
            self.assertIsNone(totals[field])
        self.assertIs(totals["complete"], False)
        self.assertEqual(totals["input_tokens"], 100)
        usage = result["completed_response_usage"]
        if not isinstance(usage, dict):
            self.fail("missing completed usage")
        self.assertEqual(usage["all"]["reasoning_tokens"], {
            "known_sum": 9, "known_responses": 9, "expected_responses": 10})

    def extraction_error(self) -> None:
        path = self.root / "events-initial/response-0.jsonl"
        trace = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        output = "```text\nfirst block\n```\n```text\nsecond block\n```\n"
        trace[2]["item"]["text"] = output
        bench.write_jsonl(path, trace)
        raw, grades = self.rows(), self.rows(kind="graded")
        raw[0] = bench.result_from_codex_subscription(
            self.plans[0], self.backend, output, bench.codex_event_metrics(trace), 2.0, str(path))
        grades[0] = {**raw[0], "generation_sha256": campaign.row_digest(raw[0]), "grading_image": self.image}
        self.write_rows(raw)
        self.write_rows(grades, kind="graded")
        repairs = [bench.codex_repair_run(plan, grade) for plan, grade in zip(self.plans, grades) if not grade["passed"]]
        self.write_stage("repair", repairs)

    def test_legitimate_extraction_error_has_no_measured_source_but_retains_usage(self) -> None:
        self.extraction_error()
        result = self.checked_audit()
        self.assertEqual(self.rows()[0]["status"], "extraction_error")
        self.assertNotIn("extracted_source", self.rows(kind="graded")[0])
        cells = result["four_cells"]
        if not isinstance(cells, list):
            self.fail("missing four cells")
        cell = next(row for row in cells if row["language"] == "jett" and row["track"] == "zero_shot"
                    and row["mode"] == "one_shot")
        self.assertEqual(cell["code_chars"], len("fixture source\n"))
        self.assertEqual(cell["code_bytes"], len(b"fixture source\n"))
        self.assertEqual(cell["input_tokens"], 20)

    def test_grade_cannot_add_source_measurements_to_an_extraction_error(self) -> None:
        self.extraction_error()
        grades = self.rows(kind="graded")
        for field, value in (("extracted_source", "invented source"), ("source_bytes", 99)):
            with self.subTest(field=field):
                self.write_rows([{**grades[0], field: value}, *grades[1:]], kind="graded")
                self.rejected("grade added unexpected generation fields")

    def test_duplicate_response_ids_are_rejected(self) -> None:
        path = self.root / "events-initial/response-1.jsonl"
        trace = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
        trace[0]["thread_id"] = "response-initial-0"
        bench.write_jsonl(path, trace)
        rows = self.rows()
        rows[1]["response_id"] = "response-initial-0"
        self.write_rows(rows)
        self.rebind_event_hash(1)
        self.rejected("duplicate response ID")

    def test_recovery_approval_receipt_and_metadata_changes_are_rejected(self) -> None:
        self.add_recovery()
        path = self.root / "recovery/decision.json"
        original = path.read_bytes()
        decision = auditor.object_file(path)
        decision["approval_text"] = "not approved"
        campaign.write_json(path, decision)
        self.rejected("approval")
        path.write_bytes(original)
        rows = self.rows()
        rows[0]["response_recovery"] = {}
        self.write_rows(rows)
        self.rebind_grades()
        self.rejected("decision_file")

    def test_recovery_original_bytes_and_missing_event_must_remain_intact(self) -> None:
        self.add_recovery()
        path = self.root / "events-initial/original-0.attempt.json"
        original = path.read_bytes()
        path.write_bytes(original + b" ")
        self.rejected("receipt or frozen initial plan mismatch")
        path.write_bytes(original)
        (self.root / "events-initial/original-0.jsonl").write_bytes(b"{}\n")
        self.rejected("existing evidence")

    def test_active_lock_or_changed_snapshot_is_rejected_without_writes(self) -> None:
        path = self.root / "active.lock"
        path.write_bytes(b"owner")
        self.rejected("active lock")
        path.unlink()
        with patch.object(campaign, "snapshot_inputs", return_value={"changed": "hash"}):
            self.rejected("campaign inputs changed")
        before = auditor.inventory(self.root)
        with patch.object(auditor, "inventory", side_effect=[before, {**before, "new": "hash"}]):
            self.rejected("evidence changed")

    def test_cli_outputs_json_only_and_errors_only_on_stderr(self) -> None:
        before = self.contents()
        stdout, stderr = StringIO(), StringIO()
        with patch.object(sys, "argv", ["audit", str(self.root)]), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(auditor.main(), 0)
        self.assertEqual(json.loads(stdout.getvalue())["audit"], "passed")
        self.assertEqual(stderr.getvalue(), "")
        self.assertEqual(self.contents(), before)
        (self.root / "active.lock").write_bytes(b"owner")
        stdout, stderr = StringIO(), StringIO()
        with patch.object(sys, "argv", ["audit", str(self.root)]), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(auditor.main(), 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("audit failed", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
