from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools import jett_bench as bench


class JettGradingPhaseTests(unittest.TestCase):
    def assess(self, first_error: str = "", test_error: str = "") -> tuple[dict, list[str]]:
        observed: list[str] = []
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            (directory / "hidden.jett").write_text("PRIVATE FIXTURE", encoding="utf-8")
            task = {"id": "phase_check", "adapters": {"jett": {
                "candidate": "solution.jett", "hidden": "hidden.jett", "grade_mode": "append",
                "commands": [["{jett}", "build", "{candidate}"], ["{jett}", "test", "{candidate}"]],
            }}}

            def execute(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                observed.append(Path(command[-1]).read_text(encoding="utf-8"))
                error = first_error if len(observed) == 1 else test_error
                return subprocess.CompletedProcess(command, int(bool(error)), "", error)

            with patch.object(bench, "jett_executable", return_value=Path("jett")), \
                    patch.object(bench, "DEFAULT_TARGET", directory / "scratch"), \
                    patch.object(bench.subprocess, "run", side_effect=execute):
                assessed = bench.grade_source(directory, task, "jett", "PUBLIC SOURCE\n", 30)
        return assessed, observed

    def test_build_reads_only_candidate_and_test_reads_appended_fixture(self) -> None:
        assessed, observed = self.assess()
        self.assertTrue(assessed["passed"])
        self.assertEqual(observed, ["PUBLIC SOURCE\n", "PUBLIC SOURCE\nPRIVATE FIXTURE"])

    def test_source_error_stops_before_any_private_fixture_is_used(self) -> None:
        assessed, observed = self.assess(first_error="error[E1001]: invalid source")
        self.assertEqual(assessed["status"], "compile_error")
        self.assertFalse(assessed["compile_succeeded"])
        self.assertEqual(observed, ["PUBLIC SOURCE\n"])

    def test_comptime_assertion_is_a_private_test_failure(self) -> None:
        assessed, _ = self.assess(test_error="error[E9000]: comptime verify failed in 'PRIVATE': assertion failed")
        self.assertEqual(assessed["status"], "test_failure")
        self.assertTrue(assessed["compile_succeeded"])
        kind, feedback = bench.repair_feedback(assessed)
        self.assertEqual(kind, "private_test_summary")
        self.assertNotIn("PRIVATE", feedback)

    def test_interface_mismatch_remains_a_compile_error(self) -> None:
        assessed, _ = self.assess(test_error="error[E3001]: argument type mismatch")
        self.assertEqual(assessed["status"], "compile_error")
        self.assertFalse(assessed["compile_succeeded"])

    def test_other_comptime_errors_are_not_hidden_assertions(self) -> None:
        assessed, _ = self.assess(test_error="error[E9000]: invalid comptime expression")
        self.assertEqual(assessed["status"], "compile_error")

    def test_mixed_compiler_messages_never_expose_relative_private_files(self) -> None:
        for location in ("hidden.rs:6:12", "hidden.ts(6,12)", "hidden.py:6:12",
                         "./hidden_test.go:6:12", "grader.rs:6:12"):
            with self.subTest(location=location):
                assessed = {"status": "compile_error", "extracted_source": "PUBLIC SOURCE\n",
                            "diagnostic": f"error: mismatch\n  --> {location}\n"
                                          "assert_eq!(solve(SECRET_INPUT), SECRET_EXPECTED);\n"
                                          "note: function defined at solution.rs:1:1"}
                kind, feedback = bench.repair_feedback(assessed)
                self.assertEqual(kind, "normalized_compile")
                self.assertNotIn("SECRET", feedback)


if __name__ == "__main__":
    unittest.main()
