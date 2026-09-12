from contextlib import contextmanager
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Iterator
import unittest
from unittest.mock import patch

from tools import bench_campaign_staging as staging
from tools import jett_bench_campaign as campaign

Row = dict[str, object]


class StagingTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / "canonical"
        self.staging = self.root / "staging"
        self.image = "sha256:" + "a" * 64
        self.stage: staging.Stage = "initial"
        self.plans: list[Row] = [
            {"run_id": key, "prompt_sha256": "prompt-" + key,
             "instructions": "Instructions", "prompt": "Public prompt " + key,
             "language": "jett", "track": "skill_assisted", "status": "planned", "passed": False}
            for key in ("a", "b")
        ]
        inputs = {"frozen/input": "unchanged"}
        mocked_inputs = patch.object(campaign, "snapshot_inputs", return_value=inputs)
        mocked_inputs.start()
        self.addCleanup(mocked_inputs.stop)
        for directory in (self.source, self.staging):
            directory.mkdir()
            campaign.bench.write_jsonl(directory / "initial-plan.jsonl", self.plans)
            campaign.write_json(directory / "config.json", {"model": "frozen"})
            campaign.write_json(directory / "campaign.json", {
                "input_hashes": inputs,
                "initial_plan_sha256": campaign.digest(directory / "initial-plan.jsonl"),
                "config_sha256": campaign.digest(directory / "config.json"),
            })
            campaign.write_json(directory / "grading-image.json", {"id": self.image, "tag": "frozen"})
        self.raw = self.make_raw(self.plans)
        self.write(self.source, "raw", self.raw)
        self.write(self.staging, "raw", self.raw)
        self.write(self.source, "graded", [self.grade(self.raw[0])])
        self.write(self.staging, "graded", [self.grade(row) for row in self.raw])

    def make_raw(self, plans: list[Row]) -> list[Row]:
        rows: list[Row] = []
        events = self.source / f"events-{self.stage}"
        events.mkdir()
        for index, plan in enumerate(plans):
            event = events / f"response-{index}.jsonl"
            event.write_bytes(b'{"type":"turn.completed"}\n')
            rows.append({
                **{key: value for key, value in plan.items() if key not in {"instructions", "prompt"}},
                "status": "generated", "passed": False, "extracted_source": "public source",
                "raw_event_log": event.name, "raw_event_log_sha256": campaign.digest(event),
            })
        return rows

    def grade(self, row: Row, **fields: object) -> Row:
        return {**row, "status": "test_failure", "passed": False, "diagnostic": "private feedback",
                "grading_image": self.image, "generation_sha256": campaign.row_digest(row), **fields}

    def path(self, directory: Path, kind: str) -> Path:
        return directory / f"{self.stage}-{kind}.jsonl"

    def write(self, directory: Path, kind: str, rows: list[Row]) -> None:
        campaign.bench.write_jsonl(self.path(directory, kind), rows)

    def contents(self) -> dict[str, bytes]:
        return {str(path.relative_to(self.root)): path.read_bytes()
                for path in self.root.rglob("*") if path.is_file()}

    def rejected(self, command: str, pattern: str) -> None:
        before = self.contents()
        operation = staging.snapshot if command == "snapshot" else staging.merge
        with self.assertRaisesRegex(campaign.bench.BenchmarkError, pattern):
            operation(self.source, self.staging, self.stage)
        self.assertEqual(self.contents(), before)

    def repair_stage(self) -> None:
        grades = [self.grade(row) for row in self.raw]
        for directory in (self.source, self.staging):
            self.write(directory, "graded", grades)
        self.stage = "repair"
        self.plans = campaign.stage_plan(self.source, self.stage)
        self.raw = self.make_raw(self.plans)
        self.write(self.source, "raw", self.raw)
        self.write(self.staging, "raw", self.raw)
        self.write(self.source, "graded", [self.grade(self.raw[0])])
        self.write(self.staging, "graded", [self.grade(row) for row in self.raw])

    def test_snapshot_copies_exact_bytes_without_locking_source(self) -> None:
        self.write(self.staging, "raw", self.raw[:1])
        self.write(self.staging, "graded", [self.grade(self.raw[0])])
        # A live generator's lock is not owned or removed by snapshot.
        lock = self.source / "active.lock"
        lock.write_bytes(b"live generator")
        before = self.contents()
        self.assertEqual(staging.snapshot(self.source, self.staging, self.stage), 2)
        after = self.contents()
        raw_key = str(self.path(self.staging, "raw").relative_to(self.root))
        expected = {**before, raw_key: self.path(self.source, "raw").read_bytes()}
        self.assertEqual(after, expected)

    def test_snapshot_accepts_new_empty_staging_journals(self) -> None:
        self.path(self.staging, "raw").unlink()
        self.path(self.staging, "graded").unlink()
        self.assertEqual(staging.snapshot(self.source, self.staging, self.stage), 2)
        self.assertFalse(self.path(self.staging, "graded").exists())

    def test_snapshot_reads_source_raw_once_even_if_it_grows(self) -> None:
        source_raw = self.path(self.source, "raw")
        self.write(self.source, "raw", self.raw[:1])
        self.write(self.staging, "raw", [])
        self.write(self.staging, "graded", [])
        original = Path.read_bytes
        reads = 0

        def read(path: Path) -> bytes:
            nonlocal reads
            content = original(path)
            if path == source_raw:
                reads += 1
                self.write(self.source, "raw", self.raw)
            return content

        with patch.object(Path, "read_bytes", read):
            self.assertEqual(staging.snapshot(self.source, self.staging, self.stage), 1)
        self.assertEqual(reads, 1)
        self.assertEqual(len(campaign.read_rows(self.path(self.staging, "raw"))), 1)

    def test_snapshot_requires_prior_snapshot_fully_graded(self) -> None:
        self.write(self.staging, "graded", [self.grade(self.raw[0])])
        self.rejected("snapshot", "fully graded")

    def test_snapshot_refuses_changed_or_missing_previous_raw(self) -> None:
        for rows in ([{**self.raw[0], "extracted_source": "changed"}, self.raw[1]], self.raw[1:]):
            with self.subTest(rows=rows):
                self.write(self.source, "raw", rows)
                self.rejected("snapshot", "previous staging raw.*changed or foreign")

    def test_stale_staging_lock_is_untouched(self) -> None:
        (self.staging / "active.lock").write_bytes(b"stale lock: inspect manually")
        for command in ("snapshot", "merge"):
            with self.subTest(command=command):
                self.rejected(command, "locked")

    def test_merge_refuses_source_lock_and_preserves_it(self) -> None:
        (self.source / "active.lock").write_bytes(b"live or stale source lock")
        self.rejected("merge", "locked")

    def test_merge_locks_both_directories_in_deterministic_order(self) -> None:
        original = campaign.campaign_lock
        acquired: list[Path] = []

        @contextmanager
        def record(directory: Path) -> Iterator[None]:
            acquired.append(directory)
            with original(directory):
                yield

        with patch.object(campaign, "campaign_lock", record):
            staging.merge(self.source, self.staging, self.stage)
        self.assertEqual(acquired, [self.source, self.staging])
        acquired.clear()
        with patch.object(campaign, "campaign_lock", record):
            # This reverse-direction attempt fails event evidence, but still locks in the same order.
            with self.assertRaises(campaign.bench.BenchmarkError):
                staging.merge(self.staging, self.source, self.stage)
        self.assertEqual(acquired, [self.source, self.staging])

    def test_same_and_nested_directories_are_rejected_before_locking(self) -> None:
        nested = self.source / "nested"
        nested.mkdir()
        for source, target in ((self.source, self.source), (self.source, self.source / ".." / "canonical"),
                               (self.source, nested), (nested, self.source)):
            for operation in (staging.snapshot, staging.merge):
                with self.subTest(source=source, target=target, operation=operation.__name__):
                    with patch.object(campaign, "campaign_lock") as lock:
                        with self.assertRaisesRegex(campaign.bench.BenchmarkError, "distinct, nonnested"):
                            operation(source, target, self.stage)
                        lock.assert_not_called()

    def test_frozen_files_must_match_byte_for_byte(self) -> None:
        for filename in ("campaign.json", "config.json", "initial-plan.jsonl", "grading-image.json"):
            path = self.staging / filename
            original = path.read_bytes()
            for command in ("snapshot", "merge"):
                with self.subTest(filename=filename, command=command):
                    path.write_bytes(original + b"\n")
                    self.rejected(command, "frozen metadata mismatch")
            path.write_bytes(original)

    def test_matching_but_changed_frozen_config_is_rejected(self) -> None:
        for directory in (self.source, self.staging):
            campaign.write_json(directory / "config.json", {"model": "changed"})
        self.rejected("snapshot", "frozen config changed")
        self.rejected("merge", "frozen config changed")

    def test_matching_but_malformed_manifests_are_rejected(self) -> None:
        for content in (b"{}\n", b'{"input_hashes":null}\n',
                        b'{"input_hashes":{},"input_hashes":{}}\n'):
            with self.subTest(content=content):
                for directory in (self.source, self.staging):
                    (directory / "campaign.json").write_bytes(content)
                self.rejected("snapshot", "malformed")
                self.rejected("merge", "malformed")

    def test_duplicate_frozen_plan_rows_are_rejected(self) -> None:
        for directory in (self.source, self.staging):
            plan = directory / "initial-plan.jsonl"
            campaign.bench.write_jsonl(plan, [self.plans[0], self.plans[0]])
            manifest = campaign.bench.read_json(directory / "campaign.json")
            manifest["initial_plan_sha256"] = campaign.digest(plan)
            campaign.write_json(directory / "campaign.json", manifest)
        self.rejected("snapshot", "duplicate")
        self.rejected("merge", "duplicate")

    def test_changed_repository_inputs_are_rejected(self) -> None:
        with patch.object(campaign, "snapshot_inputs", return_value={"frozen/input": "changed"}):
            self.rejected("snapshot", "campaign inputs changed")
            self.rejected("merge", "campaign inputs changed")

    def test_mutable_image_tag_is_rejected_even_when_matching(self) -> None:
        for directory in (self.source, self.staging):
            campaign.write_json(directory / "grading-image.json", {"id": "image:latest"})
        self.rejected("snapshot", "pinned")
        self.rejected("merge", "pinned")

    def test_malformed_and_truncated_jsonl_never_mutates_journals(self) -> None:
        valid = (json.dumps(self.raw[0]) + "\n").encode()
        invalid = [b"{\n", b"[]\n", b"null\n", b"\xff\n", valid.rstrip(b"\n"),
                   valid + b'{"run_id":', b"\n", b'{"run_id":1,"prompt_sha256":"a"}\n',
                   b'{"run_id":"a","prompt_sha256":"a","metric":NaN}\n',
                   b'{"run_id":"a","prompt_sha256":"a","metric":1e999}\n',
                   b'{"run_id":"a","run_id":"b","prompt_sha256":"a"}\n',
                   b'{"run_id":"a","prompt_sha256":"a","nested":{"x":1,"x":2}}\n']
        for command in ("snapshot", "merge"):
            for directory, kind in ((self.source, "raw"), (self.staging, "raw"), (self.staging, "graded")):
                path = self.path(directory, kind)
                original = path.read_bytes()
                for content in invalid:
                    with self.subTest(command=command, kind=kind, directory=directory, content=content):
                        path.write_bytes(content)
                        self.rejected(command, "malformed|expected JSON object|truncated|invalid")
                path.write_bytes(original)

    def test_duplicate_and_foreign_raw_rows_are_rejected(self) -> None:
        for command in ("snapshot", "merge"):
            for directory in (self.source, self.staging):
                for extra in (self.raw[0], {**self.raw[0], "run_id": "foreign"}):
                    with self.subTest(command=command, directory=directory, extra=extra):
                        self.write(directory, "raw", [*self.raw, extra])
                        self.rejected(command, "duplicate|extra 1|foreign")
                        self.write(directory, "raw", self.raw)

    def test_raw_prompt_changes_are_rejected(self) -> None:
        self.write(self.source, "raw", [{**self.raw[0], "prompt_sha256": "changed"}, self.raw[1]])
        self.rejected("snapshot", "prompt mismatch")
        self.rejected("merge", "prompt mismatch")

    def test_grade_identity_image_and_status_are_validated(self) -> None:
        changes: list[Row] = [
            {"run_id": "foreign"}, {"prompt_sha256": "changed"}, {"generation_sha256": "changed"},
            {"grading_image": "sha256:" + "b" * 64}, {"status": "generated"}, {"status": "harness_error"},
            {"status": "backend_error"}, {"status": "api_error"}, {"status": "planned"},
            {"status": "unknown"}, {"status": "passed", "passed": False}, {"passed": 0},
        ]
        for fields in changes:
            with self.subTest(fields=fields):
                self.write(self.staging, "graded", [self.grade(self.raw[0], **fields), self.grade(self.raw[1])])
                for command in ("snapshot", "merge"):
                    self.rejected(command, "staging grades")

    def test_every_completed_grader_outcome_is_accepted(self) -> None:
        for status in staging.COMPLETED:
            with self.subTest(status=status):
                self.write(self.staging, "graded", [
                    self.grade(row, status=status, passed=status == "passed") for row in self.raw
                ])
                self.assertEqual(staging.snapshot(self.source, self.staging, self.stage), 2)

    def test_duplicate_grades_are_rejected(self) -> None:
        for directory in (self.source, self.staging):
            with self.subTest(directory=directory):
                original = self.path(directory, "graded").read_bytes()
                self.write(directory, "graded", [self.grade(self.raw[0]), self.grade(self.raw[0])])
                self.rejected("merge", "duplicate")
                self.path(directory, "graded").write_bytes(original)

    def test_merge_validates_existing_canonical_grades(self) -> None:
        for fields in ({"generation_sha256": "changed"}, {"grading_image": "different"},
                       {"prompt_sha256": "changed"}, {"run_id": "foreign"}, {"status": "generated"}):
            with self.subTest(fields=fields):
                self.write(self.source, "graded", [self.grade(self.raw[0], **fields)])
                self.rejected("merge", "canonical grades")

    def test_altered_overlap_prevents_all_merging_without_cherry_picking(self) -> None:
        for fields in ({"diagnostic": "different"}, {"grader_runtime_ms": 123},
                       {"status": "passed", "passed": True}):
            with self.subTest(fields=fields):
                # A new grade comes first so validation cannot append it before finding the conflict.
                self.write(self.staging, "graded", [self.grade(self.raw[1]), self.grade(self.raw[0], **fields)])
                self.rejected("merge", "overlapping grades")

    def test_merge_refuses_changed_staging_raw_even_if_grade_hash_is_updated(self) -> None:
        changed = {**self.raw[0], "extracted_source": "changed"}
        self.write(self.staging, "raw", [changed, self.raw[1]])
        self.write(self.staging, "graded", [self.grade(changed), self.grade(self.raw[1])])
        self.rejected("merge", "staging raw.*changed")

    def test_merge_requires_complete_canonical_generation(self) -> None:
        self.write(self.source, "raw", self.raw[:1])
        self.write(self.staging, "raw", self.raw[:1])
        self.write(self.staging, "graded", [self.grade(self.raw[0])])
        self.rejected("merge", "complete canonical generation")

    def test_merge_requires_canonical_event_evidence_for_even_ungraded_rows(self) -> None:
        event = self.source / "events-initial" / "response-1.jsonl"
        original = event.read_bytes()
        for defect in ("missing", "changed"):
            with self.subTest(defect=defect):
                self.write(self.staging, "graded", [self.grade(self.raw[0])])
                if defect == "missing":
                    event.unlink()
                else:
                    event.write_bytes(b"changed")
                self.rejected("merge", "raw event evidence")
                event.write_bytes(original)

    def test_merge_rejects_missing_event_metadata(self) -> None:
        del self.raw[1]["raw_event_log_sha256"]
        self.write(self.source, "raw", self.raw)
        self.rejected("merge", "raw event evidence metadata missing")

    def test_partial_grade_merge_preserves_original_bytes_as_prefix(self) -> None:
        self.write(self.staging, "graded", [self.grade(self.raw[1])])
        path = self.path(self.source, "graded")
        # Noncanonical formatting and CRLF must survive, without normalizing prior rows.
        original = (json.dumps(self.grade(self.raw[0]), separators=(",", ":")) + "\r\n").encode()
        path.write_bytes(original)
        before = self.contents()
        self.assertEqual(staging.merge(self.source, self.staging, self.stage), 1)
        self.assertTrue(path.read_bytes().startswith(original))
        self.assertEqual(campaign.read_rows(path), [self.grade(row) for row in self.raw])
        after = self.contents()
        key = str(path.relative_to(self.root))
        self.assertEqual({k: v for k, v in before.items() if k != key},
                         {k: v for k, v in after.items() if k != key})
        with patch.object(staging, "atomic_write") as write:
            self.assertEqual(staging.merge(self.source, self.staging, self.stage), 0)
            write.assert_not_called()

    def test_semantically_equal_overlap_accepts_different_key_order(self) -> None:
        original = self.grade(self.raw[0])
        self.write(self.staging, "graded", [dict(reversed(list(original.items()))), self.grade(self.raw[1])])
        self.assertEqual(staging.merge(self.source, self.staging, self.stage), 1)

    def test_merge_allows_staging_raw_subset_and_absent_canonical_grades(self) -> None:
        self.path(self.source, "graded").unlink()
        self.write(self.staging, "raw", self.raw[1:])
        self.write(self.staging, "graded", [self.grade(self.raw[1])])
        self.assertEqual(staging.merge(self.source, self.staging, self.stage), 1)
        self.assertEqual(campaign.read_rows(self.path(self.source, "graded")), [self.grade(self.raw[1])])

    def test_replacement_failure_preserves_journals_and_cleans_temporary_file(self) -> None:
        for operation in (staging.snapshot, staging.merge):
            with self.subTest(operation=operation.__name__):
                before = self.contents()
                with patch.object(staging.os, "replace", side_effect=OSError("replace failed")):
                    with self.assertRaisesRegex(OSError, "replace failed"):
                        operation(self.source, self.staging, self.stage)
                self.assertEqual(self.contents(), before)

    def test_repair_snapshot_and_merge_use_derived_frozen_plan(self) -> None:
        self.repair_stage()
        self.assertEqual(staging.snapshot(self.source, self.staging, self.stage), 2)
        self.assertEqual(staging.merge(self.source, self.staging, self.stage), 1)
        self.assertEqual(len(campaign.read_rows(self.path(self.source, "graded"))), 2)

    def test_repair_refuses_changed_initial_grading(self) -> None:
        self.repair_stage()
        path = self.staging / "initial-graded.jsonl"
        grades = campaign.read_rows(path)
        grades[0]["diagnostic"] = "changed"
        campaign.bench.write_jsonl(path, grades)
        self.rejected("snapshot", "initial grading before repair")
        self.rejected("merge", "initial grading before repair")

    def test_repair_merge_requires_canonical_repair_event_evidence(self) -> None:
        self.repair_stage()
        (self.source / "events-repair" / "response-1.jsonl").unlink()
        self.rejected("merge", "raw event evidence missing")

    def test_repair_refuses_incomplete_initial_grading(self) -> None:
        self.repair_stage()
        for directory in (self.source, self.staging):
            path = directory / "initial-graded.jsonl"
            campaign.bench.write_jsonl(path, campaign.read_rows(path)[:1])
        self.rejected("snapshot", "initial grading before repair")
        self.rejected("merge", "initial grading before repair")

    def test_cli_failure_is_readable_and_help_is_available(self) -> None:
        with patch.object(sys, "argv", ["bench_campaign_staging.py", "snapshot",
                                      str(self.source), str(self.source), "--stage", "initial"]), \
                patch("builtins.print") as output:
            self.assertEqual(staging.main(), 1)
            self.assertIn("distinct, nonnested", str(output.call_args))
        result = subprocess.run([sys.executable, str(Path(staging.__file__)), "--help"],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("snapshot,merge", result.stdout)


if __name__ == "__main__":
    unittest.main()
