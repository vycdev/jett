"""Negative controls for private GNU archive measurement and compiled pin handoff."""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
from typing import BinaryIO
import unittest
from unittest.mock import patch

from tools import measure_resource_native_archives as measure


class NativeLibraryTests(unittest.TestCase):
    def test_actual_order_and_duplicate_arguments_are_retained(self) -> None:
        text = "note: native-static-libs: -lgcc_s -lc -lm -lc\n  -lpthread\nFinished build\n"
        self.assertEqual(measure.native_libraries(text),
                         ["-lgcc_s", "-lc", "-lm", "-lc", "-lpthread"])

    def test_missing_multiple_or_unsafe_observations_are_refused(self) -> None:
        values = [
            "", "note: native-static-libs: -lc\n",
            "note: native-static-libs: -lgcc_s -lc\nnote: native-static-libs: -lc\n",
            "note: native-static-libs: -lgcc_s -lc -ljett_runtime\n",
            "note: native-static-libs: -lgcc_s -lc -ljett_native_launcher\n",
            "note: native-static-libs: -lgcc_s -lc -static\n",
        ]
        for value in values:
            with self.subTest(value=value), self.assertRaises(measure.MeasurementError):
                measure.native_libraries(value)

    def test_private_cfgs_belong_to_one_selected_runtime_command(self) -> None:
        runtime = ("Running `rustc --crate-name jett_runtime --crate-type rlib "
                   "--crate-type staticlib --cfg test --cfg jett_resource_native_test_archive "
                   "--print=native-static-libs`")
        measure.selected_cfgs(runtime)
        values = [
            runtime.replace("jett_runtime ", "dependency "),
            runtime + "\n" + runtime,
            runtime.replace("--cfg test ", ""),
            runtime.replace("--crate-type staticlib ", ""),
        ]
        for value in values:
            with self.subTest(value=value), self.assertRaises(measure.MeasurementError):
                measure.selected_cfgs(value)

    def test_executed_compiler_identity_rejects_wrappers_and_other_compilers(self) -> None:
        compiler = Path("/measured/rustc").absolute()
        command = f"Running `{str(compiler)!r} --crate-name jett_runtime --crate-type staticlib`"
        measure.checked_compiler_commands(command, compiler)
        for invalid in ("", "Running `\"/foreign/rustc\" --crate-name jett_runtime`",
                        f"Running `wrapper {str(compiler)!r} --crate-name jett_runtime`"):
            with self.subTest(command=invalid), self.assertRaises(measure.MeasurementError):
                measure.checked_compiler_commands(invalid, compiler)

    def test_rustup_proxy_invocation_keeps_its_original_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            proxy = root / "cargo"
            proxy.write_bytes(b"proxy")
            with patch.object(measure.shutil, "which", return_value=str(proxy)), \
                 patch.object(Path, "resolve", return_value=root / "rustup") as resolve:
                self.assertEqual(measure.executable("cargo"), proxy)
                resolve.assert_not_called()

    def test_cargo_color_preferences_cannot_change_measured_output(self) -> None:
        with patch.dict(os.environ, {"CARGO_TERM_COLOR": "always"}, clear=True):
            self.assertEqual(measure.build_environment()["CARGO_TERM_COLOR"], "never")

    def test_global_cfgs_and_wrappers_are_refused(self) -> None:
        for name, value in [
            ("RUSTFLAGS", "--cfg test"),
            ("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1ftest"),
            ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS", "-Ctarget-feature=+crt-static"),
            ("RUSTC_WRAPPER", "unmeasured-rustc"),
            ("RUSTC", "unmeasured-rustc"),
            ("CARGO_BUILD_RUSTC", "unmeasured-rustc"),
        ]:
            with self.subTest(name=name), patch.dict(os.environ, {name: value}, clear=True):
                with self.assertRaises(measure.MeasurementError):
                    measure.build_environment()
        with patch.dict(os.environ, {measure.PIN_ENV: "old", measure.RECEIPT_ENV: "old"}, clear=True):
            environment = measure.build_environment()
            self.assertNotIn(measure.PIN_ENV, environment)
            self.assertNotIn(measure.RECEIPT_ENV, environment)


class SymbolAndFingerprintTests(unittest.TestCase):
    def setUp(self) -> None:
        self.leaves, self.private = measure.export_names(measure.ROOT)
        self.symbols = "archive[member.o]:\n" + "\n".join(
            f"{name} T 0 10" for name in sorted(self.leaves | self.private | {"main"}))

    def test_complete_current_source_export_contract(self) -> None:
        self.assertEqual((len(self.leaves), len(self.private)), (50, 8))
        self.assertTrue(measure.CARRIER_EXPORTS <= self.leaves)
        measure.check_symbols(self.symbols, self.leaves, self.private)

    def test_missing_duplicate_foreign_or_nontext_exports_are_refused(self) -> None:
        leaf = sorted(self.leaves)[0]
        private = sorted(self.private)[0]
        values = [
            self.symbols.replace(f"{leaf} T 0 10", ""),
            self.symbols + "\nmain T 0 10",
            self.symbols + f"\n{private} T 0 10",
            self.symbols + "\njett_rt_v1_resource_foreign T 0 10",
            self.symbols + "\njett_rt_v1_resource_test_foreign T 0 10",
            self.symbols.replace(f"{leaf} T", f"{leaf} W"),
        ]
        for value in values:
            with self.subTest(value=value), self.assertRaises(measure.MeasurementError):
                measure.check_symbols(value, self.leaves, self.private)

    def test_fingerprints_require_exact_gnu_flags_and_existing_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(measure.MeasurementError):
                measure.fingerprint_records(root)
            directory = root / "jett_runtime-hash"
            directory.mkdir()
            record = directory / "lib-jett_runtime.json"
            record.write_text('{"rustflags":[]}', encoding="utf-8")
            self.assertEqual(len(measure.fingerprint_records(root)), 1)
            record.write_text('{"rustflags":["--cfg","test"]}', encoding="utf-8")
            with self.assertRaises(measure.MeasurementError):
                measure.fingerprint_records(root)


class ReceiptAndGateTests(unittest.TestCase):
    def fixture(self, root: Path, inputs: dict[str, measure.Json]) -> Path:
        helper = root / "tools/measure_resource_native_archives.py"
        helper.parent.mkdir(parents=True)
        helper.write_bytes(Path(measure.__file__).read_bytes())
        registration = root / "crates/jett_runtime/src/resource_custody/registration.rs"
        registration.parent.mkdir(parents=True)
        registration.write_text("pub(crate) const NATIVE_RESOURCE_LAYOUT_WIRE_VERSION: u32 = 3;\n",
                                encoding="utf-8")
        linker = root / "cc"
        linker.write_bytes(b"measured C linker")
        log = root / "observation.log"
        log.write_bytes(b"independent observation")
        archives: list[measure.Json] = []
        for profile in ("debug", "release"):
            archive = root / f"{profile}-runtime.a"
            archive.write_bytes(profile.encode())
            fingerprint = root / f"{profile}-fingerprint.json"
            fingerprint.write_text('{"rustflags":[]}', encoding="utf-8")
            build = root / f"{profile}-build.json"
            measure.write_json(build, {"runtime_sources": inputs, "archive": measure.artifact(archive),
                                      "measurement_script": measure.artifact(Path(measure.__file__))})
            libraries = root / f"{profile}-libraries.json"
            measure.write_json(libraries, {"arguments": ["-lgcc_s", "-lc"],
                                          "actual_print_log": measure.artifact(log)})
            crt = root / f"{profile}-crt.json"
            measure.write_json(crt, {"mode": "dynamic", "linker_path": str(linker),
                "linker_executable": measure.artifact(linker), "cargo_fingerprints": [
                    {**measure.artifact(fingerprint), "raw_json_utf8": fingerprint.read_text(encoding="utf-8")}],
                "native_static_libs": measure.artifact(libraries)})
            archives.append({"profile": profile, "target": measure.TARGET, "runtime_abi": 1,
                "layout_wire": measure.CURRENT_LAYOUT_WIRE, "private_cfgs": list(measure.CFGS), "crt_mode": "dynamic",
                "archive_path": str(archive), "archive_sha256": measure.digest(archive),
                "linker_path": str(linker), "native_library_args": ["-lgcc_s", "-lc"],
                "build_receipt": measure.artifact(build), "native_static_libs_receipt": measure.artifact(libraries),
                "crt_receipt": measure.artifact(crt)})
        receipt = root / "receipt.json"
        measure.write_json(receipt, {"version": 1, "archives": archives})
        return receipt

    def test_stale_sources_changed_provenance_and_untrusted_pin_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs: dict[str, measure.Json] = {"Cargo.lock": "frozen"}
            receipt = self.fixture(root, inputs)
            pin = measure.digest(receipt)
            with patch.object(measure, "sources", return_value=inputs):
                measure.receipt_sources(receipt, pin, root)
                for invalid in ("", "0" * 64, pin.upper()):
                    with self.subTest(pin=invalid), self.assertRaises(measure.MeasurementError):
                        measure.receipt_sources(receipt, invalid, root)
            with patch.object(measure, "sources", return_value={"Cargo.lock": "changed"}):
                with self.assertRaises(measure.MeasurementError):
                    measure.receipt_sources(receipt, pin, root)
            (root / "debug-build.json").write_text("{}", encoding="utf-8")
            with patch.object(measure, "sources", return_value=inputs):
                with self.assertRaises(measure.MeasurementError):
                    measure.receipt_sources(receipt, pin, root)

    def test_archive_linker_and_nested_fingerprint_mutation_are_refused(self) -> None:
        for name in ("debug-runtime.a", "cc", "debug-fingerprint.json", "debug-libraries.json"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                inputs: dict[str, measure.Json] = {"Cargo.lock": "frozen"}
                receipt = self.fixture(root, inputs)
                expected = measure.digest(receipt)
                (root / name).write_bytes(b"changed after independent measurement")
                with patch.object(measure, "sources", return_value=inputs):
                    with self.assertRaises(measure.MeasurementError):
                        measure.receipt_sources(receipt, expected, root)

    def test_existing_output_is_never_rebuilt_or_reauthenticated(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(measure.MeasurementError):
                measure.prepare(root, root, "unused-cc", "unused-nm", "unused-readelf")

    def test_compiler_messages_must_select_one_actual_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "first"
            second = root / "second"
            first.write_bytes(b"first")
            second.write_bytes(b"second")
            def message(path: Path) -> str:
                return json.dumps({"reason": "compiler-artifact",
                                   "target": {"name": "native_conformance"}, "executable": str(path)})
            self.assertEqual(measure.compiled_test(message(first)), first)
            for invalid in ("", message(root / "absent"), message(first) + "\n" + message(second)):
                with self.subTest(value=invalid), self.assertRaises(measure.MeasurementError):
                    measure.compiled_test(invalid)

    def test_pin_is_compiled_then_removed_for_complete_source_gate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs: dict[str, measure.Json] = {"Cargo.lock": "frozen"}
            receipt = self.fixture(root, inputs)
            expected = measure.digest(receipt)
            binary = root / "compiled-gate"
            binary.write_bytes(b"compiled test")
            compiler = root / "rustc"
            compiler.write_bytes(b"measured compiler proxy")
            compile_environment: list[dict[str, str]] = []
            runtime_environment: list[dict[str, str]] = []
            def compile_test(argv: list[str], *, cwd: Path, env: dict[str, str], stdin: int,
                             stdout: BinaryIO, stderr: BinaryIO, timeout: int) -> subprocess.CompletedProcess[bytes]:
                self.assertIn("--no-run", argv)
                self.assertIn("--target", argv)
                self.assertEqual(argv[argv.index("--color") + 1], "never")
                self.assertEqual(env["CARGO_TERM_COLOR"], "never")
                self.assertNotIn("--cfg", argv)
                self.assertEqual(cwd, root)
                self.assertEqual(stdin, subprocess.DEVNULL)
                compile_environment.append(dict(env))
                stderr.write(f"Running `{str(compiler)!r} --crate-name jett_driver`\n".encode())
                stdout.write((json.dumps({"reason": "compiler-artifact",
                    "target": {"name": "native_conformance"}, "executable": str(binary)}) + "\n").encode())
                return subprocess.CompletedProcess(argv, 0)
            def execute_test(argv: list[str], directory: Path, environment: dict[str, str],
                             log: Path, timeout: int = 3600) -> str:
                if argv == [str(compiler), "-vV"]:
                    self.assertNotIn(measure.PIN_ENV, environment)
                    text = f"host: {measure.TARGET}\n"
                    log.write_text(text, encoding="utf-8")
                    return text
                self.assertEqual(argv[0], str(binary))
                self.assertEqual(argv[1], measure.TEST)
                self.assertIn("--ignored", argv)
                self.assertIn("--exact", argv)
                self.assertEqual(directory, root)
                runtime_environment.append(dict(environment))
                text = ("Resource native acceptance: 73 cases; 146 Source-deleted executions; "
                        "profiles=debug,release\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n")
                log.write_text(text, encoding="utf-8")
                return text
            def executable_for_test(name: str) -> Path:
                return root / name
            with patch.object(measure, "sources", return_value=inputs), \
                 patch.object(measure, "executable", side_effect=executable_for_test), \
                 patch.object(measure.subprocess, "run", side_effect=compile_test), \
                 patch.object(measure, "logged", side_effect=execute_test), \
                 patch.dict(os.environ, {}, clear=True):
                measure.run_gate(root, receipt, expected, root / "fresh-gate-target")
            self.assertEqual(compile_environment[0][measure.PIN_ENV], expected)
            self.assertEqual(compile_environment[0]["RUSTC"], str(compiler))
            self.assertNotIn(measure.RECEIPT_ENV, compile_environment[0])
            self.assertNotIn(measure.PIN_ENV, runtime_environment[0])
            self.assertEqual(runtime_environment[0][measure.RECEIPT_ENV], str(receipt))
            accepted = measure.read_object(root / "gate-acceptance.json")
            self.assertEqual(accepted["source_deleted_executions"], 146)
            self.assertEqual(accepted["runtime_expected_pin_environment"], False)

    def test_old_future_or_mixed_receipt_wire_cannot_authenticate_current_sources(self) -> None:
        for wires in ((2, 2), (4, 4), (3, 2), (1, 3)):
            with self.subTest(wires=wires), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                inputs: dict[str, measure.Json] = {"Cargo.lock": "frozen"}
                receipt = self.fixture(root, inputs)
                record = measure.read_object(receipt)
                rows = record["archives"]
                self.assertIsInstance(rows, list)
                if not isinstance(rows, list):
                    self.fail("fixture archive rows must be a list")
                for row, wire in zip(rows, wires, strict=True):
                    self.assertIsInstance(row, dict)
                    if not isinstance(row, dict):
                        self.fail("fixture archive row must be an object")
                    row["layout_wire"] = wire
                receipt.write_text(json.dumps(record), encoding="utf-8")
                with patch.object(measure, "sources", return_value=inputs), \
                     self.assertRaises(measure.MeasurementError):
                    measure.receipt_sources(receipt, measure.digest(receipt), root)


class CarrierContractTests(unittest.TestCase):
    def fixture(self, root: Path) -> tuple[Path, Path]:
        helper = root / "tools/measure_resource_native_archives.py"
        helper.parent.mkdir(parents=True)
        helper.write_bytes(Path(measure.__file__).read_bytes())
        resource = root / "crates/jett_runtime/src/native_abi/resource"
        (resource / "leaves").mkdir(parents=True)
        for relative in ("leaves.rs", "test_archive_api.rs"):
            original = measure.ROOT / "crates/jett_runtime/src/native_abi/resource" / relative
            (resource / relative).write_bytes(original.read_bytes())
        carriers = resource / "leaves/carriers.rs"
        carriers.write_text("\n".join(f'pub unsafe extern "C" fn {name}() {{}}'
            for name in sorted(measure.CARRIER_EXPORTS)), encoding="utf-8")
        registration = root / "crates/jett_runtime/src/resource_custody/registration.rs"
        registration.parent.mkdir(parents=True)
        registration.write_text("pub(crate) const NATIVE_RESOURCE_LAYOUT_WIRE_VERSION: u32 = 3;\n",
                                encoding="utf-8")
        return registration, carriers

    def test_exact_current_wire_and_carrier_source_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            measure.checked_layout_wire(root)
            leaves, private = measure.export_names(root)
            self.assertEqual((len(leaves), len(private)), (50, 8))
            self.assertTrue(measure.CARRIER_EXPORTS <= leaves)

    def test_old_future_duplicate_or_missing_runtime_wire_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            registration, _ = self.fixture(root)
            original = registration.read_text(encoding="utf-8")
            for invalid in (original.replace("= 3;", "= 2;"), original.replace("= 3;", "= 4;"),
                            original + original, ""):
                registration.write_text(invalid, encoding="utf-8")
                with self.subTest(invalid=invalid), self.assertRaises(measure.MeasurementError):
                    measure.checked_layout_wire(root)

    def test_missing_duplicate_and_foreign_carrier_names_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, carriers = self.fixture(root)
            original = carriers.read_text(encoding="utf-8")
            first, *rest = original.splitlines()
            for invalid in ("\n".join(rest), original + "\n" + first,
                            original.replace("carrier_adapt(", "carrier_foreign(")):
                carriers.write_text(invalid, encoding="utf-8")
                with self.subTest(invalid=invalid), self.assertRaises(measure.MeasurementError):
                    measure.export_names(root)
            carriers.write_text(original, encoding="utf-8")
            legacy = carriers.parent.parent / "leaves.rs"
            original_legacy = legacy.read_text(encoding="utf-8")
            first_name = sorted(measure.export_names(root)[0] - measure.CARRIER_EXPORTS)[0]
            legacy.write_text(original_legacy.replace(first_name + "(",
                "jett_rt_v1_resource_carrier_adapt("), encoding="utf-8")
            with self.assertRaises(measure.MeasurementError):
                measure.export_names(root)

    def test_stale_wire_or_symbols_refused_before_tools_and_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            registration, carriers = self.fixture(root)
            output = root / "fresh-measurement"
            original = registration.read_text(encoding="utf-8")
            registration.write_text(original.replace("= 3;", "= 2;"), encoding="utf-8")
            with patch.object(measure, "executable") as executable, \
                 self.assertRaises(measure.MeasurementError):
                measure.prepare(root, output, "unused-cc", "unused-nm", "unused-readelf")
            executable.assert_not_called()
            self.assertFalse(output.exists())
            registration.write_text(original, encoding="utf-8")
            carriers.write_text("", encoding="utf-8")
            with patch.object(measure, "executable") as executable, \
                 self.assertRaises(measure.MeasurementError):
                measure.prepare(root, output, "unused-cc", "unused-nm", "unused-readelf")
            executable.assert_not_called()
            self.assertFalse(output.exists())

    def test_foreign_executing_helper_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            measure.checked_measurement_source(root)
            (root / "tools/measure_resource_native_archives.py").write_bytes(b"foreign helper")
            with self.assertRaises(measure.MeasurementError):
                measure.checked_measurement_source(root)

    def test_guards_are_bound_to_the_source_snapshot_before_output_or_build(self) -> None:
        for changed in ("wire", "exports"):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                registration, carriers = self.fixture(root)
                output = root / "fresh-measurement"
                def snapshot_mutation(repo: Path, complete: bool = False) -> dict[str, measure.Json]:
                    self.assertEqual(repo, root)
                    self.assertFalse(complete)
                    if changed == "wire":
                        registration.write_text("pub(crate) const NATIVE_RESOURCE_LAYOUT_WIRE_VERSION: u32 = 2;\n",
                                                encoding="utf-8")
                    else:
                        carriers.write_text("", encoding="utf-8")
                    return {}
                with patch.object(measure, "sources", side_effect=snapshot_mutation), \
                     patch.object(measure, "executable", return_value=root / "unused-tool"), \
                     patch.object(measure, "build_environment", return_value={}), \
                     patch.object(measure, "toolchain") as toolchain, \
                     self.assertRaises(measure.MeasurementError):
                    measure.prepare(root, output, "unused-cc", "unused-nm", "unused-readelf")
                toolchain.assert_not_called()
                self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
