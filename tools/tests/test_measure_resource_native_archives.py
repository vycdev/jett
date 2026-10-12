"""Evidence controls for private GNU/MSVC archives and compiled pin handoff."""
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


def copy_resource_export_fixture(root: Path) -> tuple[Path, Path]:
    resource = root / "crates/jett_runtime/src/native_abi/resource"
    (resource / "leaves").mkdir(parents=True)
    for relative in ("leaves.rs", "test_archive_api.rs", "program_entry.rs"):
        original = measure.ROOT / "crates/jett_runtime/src/native_abi/resource" / relative
        (resource / relative).write_bytes(original.read_bytes())
    owner = resource.with_suffix(".rs")
    owner.write_bytes((measure.ROOT / "crates/jett_runtime/src/native_abi/resource.rs").read_bytes())
    carriers = resource / "leaves/carriers.rs"
    carriers.write_text("\n".join(f'pub unsafe extern "C" fn {name}() {{}}'
        for name in sorted(measure.CARRIER_EXPORTS)), encoding="utf-8")
    return owner, resource / "program_entry.rs"


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
        self.assertEqual((len(self.leaves), len(self.private)), (55, 8))
        self.assertTrue(measure.CARRIER_EXPORTS <= self.leaves)
        self.assertTrue(measure.STARTUP_EXPORTS <= self.leaves)
        self.assertEqual(len(self.leaves - measure.STARTUP_EXPORTS), 50)
        measure.check_symbols(self.symbols, self.leaves, self.private)

    def test_each_startup_symbol_must_have_one_text_definition(self) -> None:
        for name in sorted(measure.STARTUP_EXPORTS):
            line = f"{name} T 0 10"
            for invalid in (self.symbols.replace(line, ""), self.symbols + "\n" + line,
                            self.symbols.replace(line, f"{name} D 0 10")):
                with self.subTest(name=name, symbols=invalid), self.assertRaises(measure.MeasurementError):
                    measure.check_symbols(invalid, self.leaves, self.private)

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
        copy_resource_export_fixture(root)
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
        copy_resource_export_fixture(root)
        resource = root / "crates/jett_runtime/src/native_abi/resource"
        carriers = resource / "leaves/carriers.rs"
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
            self.assertEqual((len(leaves), len(private)), (55, 8))
            self.assertTrue(measure.CARRIER_EXPORTS <= leaves)
            self.assertEqual(leaves & measure.STARTUP_EXPORTS, measure.STARTUP_EXPORTS)

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
            first_name = sorted(measure.export_names(root)[0] - measure.CARRIER_EXPORTS - measure.STARTUP_EXPORTS)[0]
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
        for changed in ("wire", "exports", "startup_module", "startup_signature", "startup_exports"):
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
                    elif changed == "exports":
                        carriers.write_text("", encoding="utf-8")
                    elif changed == "startup_module":
                        owner = carriers.parent.parent.with_suffix(".rs")
                        owner.write_text(owner.read_text(encoding="utf-8").replace(
                            "mod program_entry;", "#[cfg(test)]\nmod program_entry;"), encoding="utf-8")
                    else:
                        child = carriers.parent.parent / "program_entry.rs"
                        text = child.read_text(encoding="utf-8")
                        if changed == "startup_signature":
                            text = text.replace("out_attempt: *mut u64,", "out_attempt: *mut u32,")
                        else:
                            text = text.replace("jett_rt_v1_resource_program_begin(",
                                                "jett_rt_v1_resource_program_foreign(")
                        child.write_text(text, encoding="utf-8")
                    return {}
                with patch.object(measure, "sources", side_effect=snapshot_mutation), \
                     patch.object(measure, "executable", return_value=root / "unused-tool"), \
                     patch.object(measure, "build_environment", return_value={}), \
                     patch.object(measure, "toolchain") as toolchain, \
                     self.assertRaises(measure.MeasurementError):
                    measure.prepare(root, output, "unused-cc", "unused-nm", "unused-readelf")
                toolchain.assert_not_called()
                self.assertFalse(output.exists())


class StartupContractTests(unittest.TestCase):
    def test_module_missing_duplicate_conditional_or_redirected_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            owner, _ = copy_resource_export_fixture(root)
            original = owner.read_text(encoding="utf-8")
            for replacement in ("", "mod program_entry;\nmod program_entry;",
                                "#[cfg(test)]\nmod program_entry;",
                                '#[path = "other.rs"]\nmod program_entry;',
                                "pub mod program_entry;"):
                owner.write_text(original.replace("mod program_entry;", replacement), encoding="utf-8")
                with self.subTest(module=replacement), self.assertRaises(measure.MeasurementError):
                    measure.export_names(root)

    def test_missing_child_and_inner_conditional_module_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, child = copy_resource_export_fixture(root)
            original = child.read_text(encoding="utf-8")
            child.write_text("#![cfg(test)]\n" + original, encoding="utf-8")
            with self.assertRaises(measure.MeasurementError):
                measure.export_names(root)
            child.unlink()
            with self.assertRaises(measure.MeasurementError):
                measure.export_names(root)

    def test_missing_duplicate_foreign_conditional_and_mangled_exports_are_refused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, child = copy_resource_export_fixture(root)
            original = child.read_text(encoding="utf-8")
            for name in sorted(measure.STARTUP_EXPORTS):
                prefix = f'pub unsafe extern "C" fn {name}('
                start = original.index(prefix)
                end = original.index("{", start)
                header = original[start:end]
                for invalid in (
                    original.replace(prefix, prefix.removeprefix("pub "), 1),
                    original + '\n#[unsafe(no_mangle)]\n' + header + '{}\n',
                    original.replace(name + "(", "jett_rt_v1_resource_program_foreign(", 1),
                    original.replace(prefix, "#[cfg(test)]\n" + prefix, 1),
                    original[:start].removesuffix("#[unsafe(no_mangle)]\n") + original[start:],
                    original.replace(prefix, prefix.replace('extern "C"', 'extern "Rust"'), 1),
                ):
                    child.write_text(invalid, encoding="utf-8")
                    with self.subTest(name=name, source=invalid), self.assertRaises(measure.MeasurementError):
                        measure.export_names(root)

    def test_every_startup_argument_and_return_type_is_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, child = copy_resource_export_fixture(root)
            original = child.read_text(encoding="utf-8")
            for name in sorted(measure.STARTUP_EXPORTS):
                start = original.index(f'pub unsafe extern "C" fn {name}(')
                end = original.index("{", start)
                header = original[start:end]
                lines = header.splitlines(keepends=True)
                for index, line in enumerate(lines):
                    if ": " not in line and "-> JettRuntimeStatusV1" not in line:
                        continue
                    changed = list(lines)
                    if ": " in line:
                        parameter = line.split(": ", 1)[0]
                        changed[index] = parameter + ": usize,\n"
                    else:
                        changed[index] = line.replace("JettRuntimeStatusV1", "u32")
                    child.write_text(original[:start] + "".join(changed) + original[end:], encoding="utf-8")
                    with self.subTest(name=name, parameter=index), self.assertRaises(measure.MeasurementError):
                        measure.export_names(root)

    def test_startup_cannot_be_relocated_into_old_leaf_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, child = copy_resource_export_fixture(root)
            leaves = child.parent / "leaves.rs"
            original = leaves.read_text(encoding="utf-8")
            names = sorted(measure.export_names(root)[0] - measure.CARRIER_EXPORTS - measure.STARTUP_EXPORTS)
            leaves.write_text(original.replace(names[0] + "(",
                "jett_rt_v1_resource_program_begin(", 1), encoding="utf-8")
            with self.assertRaises(measure.MeasurementError):
                measure.export_names(root)

    def test_invalid_startup_is_refused_before_tool_discovery_and_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, carriers = CarrierContractTests().fixture(root)
            startup = carriers.parent.parent / "program_entry.rs"
            startup.write_text(startup.read_text(encoding="utf-8").replace(
                "out_attempt: *mut u64,", "out_attempt: *mut u32,"), encoding="utf-8")
            output = root / "fresh-measurement"
            with patch.object(measure, "executable") as executable, \
                 self.assertRaises(measure.MeasurementError):
                measure.prepare(root, output, "unused-cc", "unused-nm", "unused-readelf")
            executable.assert_not_called()
            self.assertFalse(output.exists())

    def test_receipt_authentication_checks_startup_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs: dict[str, measure.Json] = {"Cargo.lock": "frozen"}
            receipt = ReceiptAndGateTests().fixture(root, inputs)
            child = root / "crates/jett_runtime/src/native_abi/resource/program_entry.rs"
            child.write_text(child.read_text(encoding="utf-8").replace(
                "out_attempt: *mut u64,", "out_attempt: *mut u32,"), encoding="utf-8")
            with patch.object(measure, "sources", return_value=inputs), \
                 self.assertRaises(measure.MeasurementError):
                measure.receipt_sources(receipt, measure.digest(receipt), root)


class MsvcObservationTests(unittest.TestCase):
    flags = ["-C", "target-feature=+crt-static", "-C", "debuginfo=0"]
    libraries = ["kernel32.lib", "advapi32.lib", "ntdll.lib", "userenv.lib",
                 "ws2_32.lib", "dbghelp.lib", "bcrypt.lib", "advapi32.lib",
                 "libvcruntime.lib", "libucrt.lib", "libcmt.lib"]

    raw_libraries = ["bcrypt.lib", "advapi32.lib", "gdi32.lib", "kernel32.lib", "msimg32.lib",
                     "opengl32.lib", "user32.lib", "winspool.lib", "kernel32.lib", "ntdll.lib",
                     "userenv.lib", "ws2_32.lib", "dbghelp.lib", "/defaultlib:libcmt"]
    @staticmethod
    def runtime_command(compiler: Path) -> str:
        return (f'Running `"{compiler}" --crate-name jett_runtime --crate-type rlib '
                '--crate-type staticlib --target x86_64-pc-windows-msvc '
                '--cfg test --cfg jett_resource_native_test_archive '
                '-C target-feature=+crt-static -C debuginfo=0 --print=native-static-libs`')

    @staticmethod
    def coff_observation() -> str:
        # llvm-readobj prints one complete target record for every archive member.
        return ('File: runtime.lib(runtime.o)\nFormat: COFF-x86-64\nArch: x86_64\n'
                'AddressSize: 64bit\nImageFileHeader {\n  Machine: IMAGE_FILE_MACHINE_AMD64 (0x8664)\n}\n'
                'Directive(s): /DEFAULTLIB:libcmt /DEFAULTLIB:libvcruntime /DEFAULTLIB:libucrt\n'
                'File: runtime.lib(compiler_builtins.o)\nFormat: COFF-x86-64\nArch: x86_64\n'
                'AddressSize: 64bit\nImageFileHeader {\n  Machine: IMAGE_FILE_MACHINE_AMD64 (0x8664)\n}\n')

    @staticmethod
    def rust_archive_without_directives() -> str:
        ordinary = MsvcObservationTests.coff_observation().replace(
            "Directive(s): /DEFAULTLIB:libcmt /DEFAULTLIB:libvcruntime /DEFAULTLIB:libucrt\n", "")
        imported = ("File: kernel32.dll\nFormat: COFF-import-file-x86-64\nType: code\n"
                    "Name type: name\nExport name: CloseHandle\n"
                    "Symbol: __imp_CloseHandle\nSymbol: CloseHandle\n")
        return ordinary + imported
    def test_windows_argv_preserves_paths_and_quoted_cfg_values(self) -> None:
        command = (r'"C:\Program Files\Rust\rustc.exe" --cfg "feature=\"native-tests\"" '
                   r'-L "C:\target with spaces\deps" --cfg test')
        self.assertEqual(measure.windows_arguments(command), [
            r"C:\Program Files\Rust\rustc.exe", "--cfg", 'feature="native-tests"',
            "-L", r"C:\target with spaces\deps", "--cfg", "test"])
        with self.assertRaises(measure.MeasurementError):
            measure.windows_arguments(r'"C:\Program Files\Rust\rustc.exe')

    def test_msvc_library_observation_keeps_order_and_duplicates(self) -> None:
        text = ("note: native-static-libs: kernel32.lib advapi32.lib ntdll.lib userenv.lib\n"
                "  ws2_32.lib dbghelp.lib bcrypt.lib advapi32.lib\n"
                "  libvcruntime.lib libucrt.lib libcmt.lib\nFinished build\n")
        self.assertEqual(measure.msvc_native_libraries(text), self.libraries)
        status = text.replace("Finished build", "    Finished `dev` profile [optimized] target(s) in 8.3s")
        self.assertEqual(measure.msvc_native_libraries(status), self.libraries)
        self.assertEqual(self.libraries.count("advapi32.lib"), 2)

    def test_msvc_library_observation_refuses_foreign_crt_or_linker_options(self) -> None:
        good = "note: native-static-libs: " + " ".join(self.libraries) + "\n"
        invalid = ["", good + good, good.replace("libcmt.lib", "msvcrt.lib"), good.replace("libcmt.lib", ""),
                   good.replace("libucrt.lib", "ucrt.lib"), good + "  /NODEFAULTLIB:libcmt\n",
                   good + "  jett_runtime.lib\n", good + "  jett_native_launcher.lib\n",
                   good.replace("libcmt.lib", "libcmt.a"), good + "  @response.rsp\n"]
        for text in invalid:
            with self.subTest(text=text), self.assertRaises(measure.MeasurementError):
                measure.msvc_native_libraries(text)

    def test_selected_windows_compiler_owns_both_private_cfgs(self) -> None:
        with tempfile.TemporaryDirectory(prefix="compiler identity ") as temporary:
            compiler = Path(temporary) / "measured rustc.exe"
            command = self.runtime_command(compiler)
            dependency = (f'Running `"{compiler}" --crate-name dependency --crate-type rlib '
                          '--target x86_64-pc-windows-msvc -C target-feature=+crt-static '
                          '-C debuginfo=0 --cfg "feature=\\\"jett_resource_native_test_archive\\\""`')
            measure.msvc_selected_cfgs(dependency + "\n" + command, compiler)
            invalid = [
                command + "\n" + command,
                command.replace("--crate-name jett_runtime", "--crate-name dependency"),
                command.replace("--cfg test ", ""),
                command.replace("--cfg test ", "--cfg test --cfg test "),
                command.replace("--crate-type staticlib ", ""),
                command.replace("--print=native-static-libs", "--test --print=native-static-libs"),
                command.replace("target-feature=+crt-static", "target-feature=-crt-static"),
                command.replace("debuginfo=0", "debuginfo=2"),
                command.replace(str(compiler), str(compiler.parent / "foreign rustc.exe")),
                command.replace('Running `"', 'Running `wrapper.exe "', 1),
                command + "\n" + dependency.replace('--cfg "feature=\\\"jett_resource_native_test_archive\\\""',
                                                    '--cfg jett_resource_native_test_archive'),
            ]
            for text in invalid:
                with self.subTest(text=text), self.assertRaises(measure.MeasurementError):
                    measure.msvc_selected_cfgs(text, compiler)

    def test_static_crt_fingerprint_requires_observed_flags_not_an_asserted_mode(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(measure.MeasurementError):
                measure.msvc_fingerprint_records(root)
            directory = root / "jett_runtime-hash"
            directory.mkdir()
            fingerprint = directory / "lib-jett_runtime.json"
            fingerprint.write_text(json.dumps({"rustflags": self.flags}), encoding="utf-8")
            records = measure.msvc_fingerprint_records(root)
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0], {**measure.artifact(fingerprint),
                "raw_json_utf8": fingerprint.read_text(encoding="utf-8")})
            for flags in ([], self.flags[:-2], ["--cfg", "test", *self.flags],
                          ["-C", "target-feature=-crt-static", "-C", "debuginfo=0"],
                          ["-C", "target-feature=+crt-static", "-C", "debuginfo=2"]):
                fingerprint.write_text(json.dumps({"rustflags": flags}), encoding="utf-8")
                with self.subTest(flags=flags), self.assertRaises(measure.MeasurementError):
                    measure.msvc_fingerprint_records(root)

    def test_every_coff_member_has_the_measured_target_and_static_crt_directives(self) -> None:
        good = self.coff_observation()
        measure.checked_coff_archive(good)
        measure.checked_coff_archive(good + 'Directive(s): /FAILIFMISMATCH:"RuntimeLibrary=MT_StaticRelease"\n')
        second = good.index("File: runtime.lib(compiler_builtins.o)")
        invalid = [
            "", good[:second] + good[second:].replace("COFF-x86-64", "COFF-ARM64"),
            good[:second] + good[second:].replace("x86_64", "aarch64"),
            good[:second] + good[second:].replace("64bit", "32bit"),
            good[:second] + good[second:].replace("IMAGE_FILE_MACHINE_AMD64 (0x8664)",
                                                    "IMAGE_FILE_MACHINE_I386 (0x14C)"),
            good[:second] + good[second:].replace("  Machine: IMAGE_FILE_MACHINE_AMD64 (0x8664)\n", ""),
            good + "Directive(s): /DEFAULTLIB:msvcrt\n",
            good + 'Directive(s): /DEFAULTLIB:"MSVCRTD.LIB"\n',
            good + "Directive(s): /DEFAULTLIB:ucrt\n",
            good + "Directive(s): /NODEFAULTLIB:libcmt\n",
            good + 'Directive(s): /FAILIFMISMATCH:"RuntimeLibrary=MD_DynamicRelease"\n',
            good + 'Directive(s): /FAILIFMISMATCH:"RuntimeLibrary=MTd_StaticDebug"\n',
        ]
        for text in invalid:
            with self.subTest(text=text), self.assertRaises(measure.MeasurementError):
                measure.checked_coff_archive(text)

    def test_msvc_environment_keeps_sdk_lib_verbatim_and_blocks_global_cfg_injection(self) -> None:
        sdk = r"C:\VS Libraries\x64;C:\SDK\ucrt\x64;C:\SDK\um\x64;"
        with patch.dict(os.environ, {"LIB": sdk, "CARGO_TERM_COLOR": "always"}, clear=True):
            environment = measure.msvc_build_environment()
            self.assertEqual(environment["LIB"], sdk)
            self.assertEqual(environment["CARGO_INCREMENTAL"], "0")
            self.assertEqual(environment["CARGO_TERM_COLOR"], "never")
            self.assertEqual(environment["CARGO_PROFILE_DEV_DEBUG"], "0")
            self.assertEqual(environment["CARGO_PROFILE_RELEASE_DEBUG"], "0")
        for name, value in (("RUSTFLAGS", "--cfg test"),
                            ("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1ftest"),
                            ("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS", "-Ctarget-feature=-crt-static"),
                            ("RUSTC_WRAPPER", "wrapper.exe"), ("RUSTC", "foreign-rustc.exe")):
            with self.subTest(name=name), patch.dict(os.environ, {"LIB": sdk, name: value}, clear=True):
                with self.assertRaises(measure.MeasurementError):
                    measure.msvc_build_environment()

    def test_coff_system_import_members_need_their_own_target_and_dll_evidence(self) -> None:
        # LLVM import members omit the ordinary Arch/AddressSize/FileHeader rows.
        imported = ("File: kernel32.dll\nFormat: COFF-import-file-x86-64\nType: code\n"
                    "Name type: name\nExport name: CloseHandle\n"
                    "Symbol: __imp_CloseHandle\nSymbol: CloseHandle\n")
        ordinary = self.coff_observation()
        measure.checked_coff_archive(ordinary + imported)
        measured_dlls = ("advapi32.dll", "api-ms-win-core-handle-l1-1-0.dll",
            "api-ms-win-core-synch-l1-2-0.dll", "bcp47mrm.dll", "bcryptprimitives.dll",
            "elscore.dll", "gdi32.dll", "icu.dll", "icuin.dll", "icuuc.dll", "kernel32.dll",
            "normaliz.dll", "ntdll.dll", "oleaut32.dll", "user32.dll", "usp10.dll")
        measure.checked_coff_archive(ordinary + "".join(imported.replace("kernel32.dll", name)
            for name in measured_dlls))
        invalid = (
            imported.replace("COFF-import-file-x86-64", "COFF-import-file-i386"),
            imported.replace("COFF-import-file-x86-64", "COFF-import-file-arm64"),
            imported.replace("COFF-import-file-x86-64", "unknown-import-format"),
            imported.replace("Type: code", "Type: data"),
            imported.replace("Name type: name", "Name type: ordinal"),
            imported.replace("kernel32.dll", "ucrtbase.dll"),
            imported.replace("kernel32.dll", "vcruntime140.dll"),
            imported.replace("kernel32.dll", "msvcrt.dll"),
            imported.replace("kernel32.dll", "foreign-runtime.dll"),
        )
        for value in invalid:
            with self.subTest(imported=value), self.assertRaises(measure.MeasurementError):
                measure.checked_coff_archive(ordinary + value)

    def test_mixed_rust_archive_with_no_directive_records_has_valid_coff_evidence(self) -> None:
        text = self.rust_archive_without_directives()
        self.assertNotIn("Directive(s):", text)
        measure.checked_coff_archive(text)

    def test_observed_directives_need_no_embedded_crt_library_but_must_not_conflict(self) -> None:
        text = self.rust_archive_without_directives()
        accepted = ("/DEFAULTLIB:kernel32", '/FAILIFMISMATCH:"_ITERATOR_DEBUG_LEVEL=0"',
                    '/FAILIFMISMATCH:"RuntimeLibrary=MT_StaticRelease"', "/DEFAULTLIB:libvcruntime")
        rejected = ("/DEFAULTLIB:msvcrt", "/DEFAULTLIB:ucrt", "/DEFAULTLIB:libcmtd",
                    "/NODEFAULTLIB:libcmt", '/FAILIFMISMATCH:"RuntimeLibrary=MD_DynamicRelease"')
        for directive in accepted:
            with self.subTest(directive=directive):
                observed = text.replace("File: kernel32.dll", f"Directive(s): {directive}\nFile: kernel32.dll")
                self.assertNotIn("/DEFAULTLIB:libcmt", observed)
                measure.checked_coff_archive(observed)
        for directive in rejected:
            with self.subTest(directive=directive), self.assertRaises(measure.MeasurementError):
                measure.checked_coff_archive(text.replace("File: kernel32.dll",
                    f"Directive(s): {directive}\nFile: kernel32.dll"))

    def test_actual_native_library_vector_preserves_duplicate_and_raw_crt_flag(self) -> None:
        text = ("note: native-static-libs: bcrypt.lib advapi32.lib gdi32.lib kernel32.lib "
                "msimg32.lib opengl32.lib user32.lib winspool.lib kernel32.lib ntdll.lib "
                "userenv.lib ws2_32.lib dbghelp.lib /defaultlib:libcmt\n")
        self.assertEqual(measure.msvc_native_libraries(text), self.raw_libraries)
        self.assertEqual(self.raw_libraries.count("kernel32.lib"), 2)
        self.assertEqual(self.raw_libraries[-1], "/defaultlib:libcmt")

    def test_wrapped_safe_defaultlib_flag_is_validated_without_rewriting_it(self) -> None:
        for flag in ("/defaultlib:libcmt", "/DEFAULTLIB:libcmt.lib", "/DefaultLib:LiBcMt.LiB"):
            with self.subTest(flag=flag):
                text = ("note: native-static-libs: " + " ".join(self.raw_libraries[:-1])
                        + "\n  " + flag + "\n    Finished `dev` profile [optimized] target(s) in 8.3s\n")
                self.assertEqual(measure.msvc_native_libraries(text), [*self.raw_libraries[:-1], flag])

    def test_defaultlib_tokens_cannot_hide_dynamic_crt_paths_or_unknown_switches(self) -> None:
        invalid = ("/defaultlib:msvcrt", "/DEFAULTLIB:UCRT.LIB", "/defaultlib:libcmtd",
                   "/defaultlib:vcruntime", "/defaultlib:libcmt.a", "/defaultlib:libcmt.lib.exe",
                   r"/defaultlib:C:\SDK\libcmt.lib", "/defaultlib:../libcmt.lib", r"/LIBPATH:C:\SDK",
                   "@native.rsp", "/WHOLEARCHIVE:libcmt.lib", "-defaultlib:libcmt", "/defaultlib:")
        for flag in invalid:
            for separator in (" ", "\n  "):
                with self.subTest(flag=flag, wrapped=separator != " "), self.assertRaises(measure.MeasurementError):
                    measure.msvc_native_libraries("note: native-static-libs: "
                        + " ".join(self.raw_libraries[:-1]) + separator + flag + "\n")
        with self.assertRaises(measure.MeasurementError):
            measure.msvc_native_libraries("note: native-static-libs: " + " ".join(self.raw_libraries[:-1])
                + "\n  /defaultlib:libcmt @native.rsp\n")

class MsvcReceiptTests(unittest.TestCase):
    @staticmethod
    def rewrite(path: Path, value: measure.Json) -> None:
        path.write_text(json.dumps(value), encoding="utf-8")

    @staticmethod
    def rows(receipt: Path) -> list[dict[str, measure.Json]]:
        value = measure.read_object(receipt).get("archives")
        if not isinstance(value, list) or any(not isinstance(row, dict) for row in value):
            raise AssertionError("fixture must retain archive objects")
        return [row for row in value if isinstance(row, dict)]

    def fixture(self, root: Path) -> tuple[Path, dict[str, str]]:
        # These are synthetic measurements, never copies of built Cargo artifacts.
        helper = root / "tools/measure_resource_native_archives.py"
        helper.parent.mkdir(parents=True)
        helper.write_bytes(Path(measure.__file__).read_bytes())
        runtime = root / "crates/jett_runtime"
        resource = runtime / "src/native_abi/resource"
        copy_resource_export_fixture(root)
        leaves, private = measure.export_names(measure.ROOT)
        for path, names in (
            (resource / "leaves.rs", leaves - measure.CARRIER_EXPORTS - measure.STARTUP_EXPORTS),
            (resource / "leaves/carriers.rs", measure.CARRIER_EXPORTS),
            (resource / "test_archive_api.rs", private),
        ):
            path.write_text("\n".join(f'pub unsafe extern "C" fn {name}() {{}}' for name in sorted(names)),
                            encoding="utf-8")
        registration = runtime / "src/resource_custody/registration.rs"
        registration.parent.mkdir(parents=True)
        registration.write_text("pub(crate) const NATIVE_RESOURCE_LAYOUT_WIRE_VERSION: u32 = 3;\n",
                                encoding="utf-8")
        for path in (root / "Cargo.lock", root / "Cargo.toml", runtime / "Cargo.toml"):
            path.write_bytes(b"synthetic retained source fixture")
        compiler = root / "tool chain/rustc.exe"
        compiler.parent.mkdir()
        compiler.write_bytes(b"measured compiler fixture")
        linker = root / "tool chain/link.exe"
        linker.write_bytes(b"measured linker fixture")
        environment = {"LIB": str(root / "SDK Libraries/x64") + ";" + str(root / "VS Libraries/x64") + ";",
                       "VCINSTALLDIR": str(root / "VS Libraries"), "VSCMD_ARG_TGT_ARCH": "x64",
                       "PATH": str(linker.parent)}
        inputs = measure.sources(root)
        archives: list[measure.Json] = []
        for profile in ("debug", "release"):
            archive = root / f"{profile}-runtime.lib"
            archive.write_bytes(("synthetic " + profile + " archive fixture").encode())
            fingerprint = root / f"{profile}-fingerprint.json"
            self.rewrite(fingerprint, {"rustflags": MsvcObservationTests.flags})
            build_log = root / f"{profile}-build.log"
            build_log.write_text(MsvcObservationTests.runtime_command(compiler).replace("--print=native-static-libs",
                f'-C "linker={linker}" --print=native-static-libs') + "\n"
                + "note: native-static-libs: " + " ".join(MsvcObservationTests.libraries) + "\n",
                encoding="utf-8")
            symbols = root / f"{profile}-symbols.log"
            symbols.write_text("runtime.lib(runtime.o):\n" + "\n".join(
                f"{name} T 0 10" for name in sorted(leaves | private | {"main"})), encoding="utf-8")
            coff = root / f"{profile}-coff.log"
            coff.write_text(MsvcObservationTests.coff_observation(), encoding="utf-8")
            build = root / f"{profile}-build.json"
            measure.write_json(build, {"profile": profile, "target": "x86_64-pc-windows-msvc",
                "runtime_sources": inputs, "archive": measure.artifact(archive),
                "cfgs_selected_package_only": ["test", "jett_resource_native_test_archive"],
                "measurement_script": measure.artifact(Path(measure.__file__)),
                "cargo_build_log": measure.artifact(build_log), "build_environment": {
                    "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS": "-C target-feature=+crt-static -C debuginfo=0",
                    "CARGO_INCREMENTAL": "0", "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_RELEASE_DEBUG": "0"},
                "toolchain": {
                    "rustc": {"invocation_path": str(compiler), "executable": measure.artifact(compiler)}}})
            libraries = root / f"{profile}-libraries.json"
            measure.write_json(libraries, {"arguments": MsvcObservationTests.libraries,
                "actual_print_log": measure.artifact(build_log), "order_and_duplication_preserved": True})
            crt = root / f"{profile}-crt.json"
            measure.write_json(crt, {"mode": "static", "linker_path": str(linker),
                "linker_executable": measure.artifact(linker), "sdk_LIB": environment["LIB"],
                "developer_shell": {"VCINSTALLDIR": environment["VCINSTALLDIR"], "VSCMD_ARG_TGT_ARCH": "x64"},
                "actual_target_scoped_rustflags": MsvcObservationTests.flags,
                "cargo_fingerprints": [{**measure.artifact(fingerprint),
                    "raw_json_utf8": fingerprint.read_text(encoding="utf-8")}],
                "complete_coff_archive_inventory": measure.artifact(coff), "coff_directive_record_count": 1,
                "complete_symbol_inventory": measure.artifact(symbols),
                "native_static_libs": measure.artifact(libraries)})
            archives.append({"profile": profile, "target": "x86_64-pc-windows-msvc", "runtime_abi": 1,
                "layout_wire": 3, "private_cfgs": ["test", "jett_resource_native_test_archive"],
                "crt_mode": "static", "archive_path": str(archive), "archive_sha256": measure.digest(archive),
                "linker_path": str(linker), "native_library_args": MsvcObservationTests.libraries,
                "build_receipt": measure.artifact(build), "native_static_libs_receipt": measure.artifact(libraries),
                "crt_receipt": measure.artifact(crt)})
        receipt = root / "receipt.json"
        measure.write_json(receipt, {"version": 1, "archives": archives})
        return receipt, environment

    def refresh_artifacts(self, value: measure.Json) -> None:
        if isinstance(value, list):
            for item in value:
                self.refresh_artifacts(item)
        elif isinstance(value, dict):
            for item in value.values():
                self.refresh_artifacts(item)
            path = value.get("path")
            if isinstance(path, str) and isinstance(value.get("sha256"), str):
                file = Path(path)
                if file.suffix == ".json":
                    record = measure.read_object(file)
                    self.refresh_artifacts(record)
                    self.rewrite(file, record)
                value["sha256"] = measure.digest(file)
                if "raw_json_utf8" in value:
                    value["raw_json_utf8"] = file.read_text(encoding="utf-8")

    def reauthenticate(self, receipt: Path) -> str:
        record = measure.read_object(receipt)
        self.refresh_artifacts(record)
        self.rewrite(receipt, record)
        return measure.digest(receipt)

    def verify(self, receipt: Path, root: Path, environment: dict[str, str], expected: str | None = None) -> None:
        with patch.dict(os.environ, environment, clear=True), \
             patch.object(measure.shutil, "which", return_value=str(root / "tool chain/link.exe")):
            measure.receipt_sources(receipt, expected or measure.digest(receipt), root,
                                    target="x86_64-pc-windows-msvc")

    def test_complete_msvc_receipt_retains_current_exports_and_sdk_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt, environment = self.fixture(root)
            self.verify(receipt, root, environment)
            self.assertEqual([row["profile"] for row in self.rows(receipt)], ["debug", "release"])
            self.assertEqual(self.rows(receipt)[0]["native_library_args"], MsvcObservationTests.libraries)
            leaves, private = measure.export_names(root)
            self.assertEqual((len(leaves - measure.CARRIER_EXPORTS - measure.STARTUP_EXPORTS),
                              len(measure.CARRIER_EXPORTS), len(leaves & measure.STARTUP_EXPORTS), len(private)),
                             (37, 13, 5, 8))

    def test_receipt_pin_and_actual_runtime_or_helper_source_changes_are_refused(self) -> None:
        for changed in ("Cargo.lock", "tools/measure_resource_native_archives.py",
                        "crates/jett_runtime/src/native_abi/resource/leaves.rs"):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.fixture(root)
                expected = measure.digest(receipt)
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment, "0" * 64)
                (root / changed).write_bytes(b"changed current source after measurement")
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment, expected)

    def test_sdk_lib_and_developer_shell_cannot_change_after_measurement(self) -> None:
        for name, replacement in (("LIB", "same directories;different exact vector"),
                                  ("VSCMD_ARG_TGT_ARCH", "x86"), ("VCINSTALLDIR", "foreign installation")):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.fixture(root)
                environment[name] = environment[name].replace(";", ";;", 1) if name == "LIB" else replacement
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment)

    def test_rehashed_forged_observations_do_not_prove_static_crt_or_compiler_identity(self) -> None:
        corruptions = {
            "fingerprint": ("debug-fingerprint.json", '{"rustflags":[]}'),
            "coff-crt": ("debug-coff.log", MsvcObservationTests.coff_observation() + "Directive(s): /DEFAULTLIB:msvcrt\n"),
            "coff-member": ("debug-coff.log", MsvcObservationTests.coff_observation().replace("COFF-x86-64", "COFF-ARM64", 1)),
            "symbols": ("debug-symbols.log", "main T 0 10\nmain T 0 10\n"),
        }
        for name, (filename, text) in corruptions.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.fixture(root)
                (root / filename).write_text(text, encoding="utf-8")
                if filename == "debug-coff.log":
                    crt_path = root / "debug-crt.json"
                    crt = measure.read_object(crt_path)
                    crt["coff_directive_record_count"] = text.count("Directive(s):")
                    self.rewrite(crt_path, crt)
                expected = self.reauthenticate(receipt)
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment, expected)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt, environment = self.fixture(root)
            build_log = root / "debug-build.log"
            build_log.write_text(build_log.read_text(encoding="utf-8").replace('Running `"', 'Running `wrapper.exe "'),
                                 encoding="utf-8")
            expected = self.reauthenticate(receipt)
            with self.assertRaises(measure.MeasurementError):
                self.verify(receipt, root, environment, expected)

    def test_rehashed_receipt_cannot_change_order_duplicates_or_profile_contract(self) -> None:
        changes: tuple[tuple[str, measure.Json], ...] = (
            ("native_library_args", list(reversed(MsvcObservationTests.libraries))),
            ("native_library_args", list(dict.fromkeys(MsvcObservationTests.libraries))),
            ("profile", "release"), ("target", "x86_64-unknown-linux-gnu"), ("runtime_abi", 2),
            ("layout_wire", 2), ("private_cfgs", ["test"]), ("crt_mode", "dynamic"))
        for field, value in changes:
            with self.subTest(field=field, value=value), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.fixture(root)
                record = measure.read_object(receipt)
                archives = record.get("archives")
                if not isinstance(archives, list) or not isinstance(archives[0], dict):
                    self.fail("fixture must retain archive objects")
                archives[0][field] = value
                self.rewrite(receipt, record)
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment)

    def test_changed_nested_archive_linker_or_observation_cannot_keep_original_pin(self) -> None:
        for name in ("debug-runtime.lib", "tool chain/link.exe", "debug-build.log", "debug-fingerprint.json"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.fixture(root)
                expected = measure.digest(receipt)
                (root / name).write_bytes(b"changed independent artifact")
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment, expected)

    def test_inherited_path_must_select_the_same_linker_without_an_override(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, environment = self.fixture(root)
            linker = root / "tool chain/link.exe"
            with patch.object(measure.shutil, "which", return_value=str(linker)) as which:
                measure.checked_msvc_environment(environment, linker)
            which.assert_called_once_with("link.exe", path=environment["PATH"])
            with patch.object(measure.shutil, "which", return_value=str(root / "foreign-link.exe")), \
                 self.assertRaises(measure.MeasurementError):
                measure.checked_msvc_environment(environment, linker)
            environment["CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER"] = str(root / "foreign-link.exe")
            with patch.object(measure.shutil, "which", return_value=str(linker)), \
                 self.assertRaises(measure.MeasurementError):
                measure.checked_msvc_environment(environment, linker)

    def test_existing_output_and_stale_source_contract_stop_before_tools(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            with patch.object(measure, "executable") as executable, self.assertRaises(measure.MeasurementError):
                measure.prepare_msvc(root, root)
            executable.assert_not_called()
            registration = root / "crates/jett_runtime/src/resource_custody/registration.rs"
            registration.write_text("pub(crate) const NATIVE_RESOURCE_LAYOUT_WIRE_VERSION: u32 = 2;\n",
                                    encoding="utf-8")
            output = root / "fresh-output"
            with patch.object(measure, "executable") as executable, self.assertRaises(measure.MeasurementError):
                measure.prepare_msvc(root, output)
            executable.assert_not_called()
            self.assertFalse(output.exists())

    def test_sources_changed_by_build_stop_before_archive_observation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, environment = self.fixture(root)
            inputs = measure.sources(root)
            leaves, private = measure.export_names(root)
            directory = root / "measurement"
            directory.mkdir()
            tools = measure.MsvcTools(root / "tool chain/cargo.exe", root / "tool chain/rustc.exe",
                                      root / "tool chain/llvm-nm.exe", root / "tool chain/llvm-readobj.exe",
                                      root / "tool chain/link.exe")
            def changed_build(argv: list[str], repo: Path, env: dict[str, str],
                              log: Path, timeout: int = 3600) -> str:
                self.assertEqual(argv[1], "rustc")
                self.assertEqual(repo, root)
                self.assertEqual(env["LIB"], environment["LIB"])
                text = MsvcObservationTests.runtime_command(tools.rustc)
                log.write_text(text, encoding="utf-8")
                (root / "Cargo.lock").write_bytes(b"changed during the selected runtime build")
                return text
            with patch.object(measure, "logged", side_effect=changed_build) as logged, \
                 self.assertRaises(measure.MeasurementError):
                measure.measure_msvc_profile(tools, root, directory, "debug", environment,
                                             inputs, {}, leaves, private)
            self.assertEqual(logged.call_count, 1)
            self.assertFalse((directory / "debug-symbols.log").exists())
            self.assertFalse((directory / "receipt.json").exists())

    def test_rehashed_build_cannot_redirect_archive_or_assert_different_private_cfgs(self) -> None:
        for change in ("archive", "cfgs"):
            with self.subTest(change=change), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.fixture(root)
                build_path = root / "debug-build.json"
                build = measure.read_object(build_path)
                if change == "archive":
                    build["archive"] = measure.artifact(root / "release-runtime.lib")
                else:
                    build["cfgs_selected_package_only"] = ["test", "test", "jett_resource_native_test_archive"]
                self.rewrite(build_path, build)
                expected = self.reauthenticate(receipt)
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment, expected)

    def test_rehashed_library_metadata_cannot_override_the_observed_vector(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt, environment = self.fixture(root)
            changed = list(dict.fromkeys(MsvcObservationTests.libraries))
            record = measure.read_object(receipt)
            archives = record.get("archives")
            if not isinstance(archives, list) or not isinstance(archives[0], dict):
                self.fail("fixture must retain archive objects")
            archives[0]["native_library_args"] = changed
            self.rewrite(receipt, record)
            library_path = root / "debug-libraries.json"
            libraries = measure.read_object(library_path)
            libraries["arguments"] = changed
            self.rewrite(library_path, libraries)
            expected = self.reauthenticate(receipt)
            with self.assertRaises(measure.MeasurementError):
                self.verify(receipt, root, environment, expected)

    def no_directive_fixture(self, root: Path) -> tuple[Path, dict[str, str]]:
        receipt, environment = self.fixture(root)
        for profile in ("debug", "release"):
            (root / f"{profile}-coff.log").write_text(MsvcObservationTests.rust_archive_without_directives(),
                                                     encoding="utf-8")
            crt_path = root / f"{profile}-crt.json"
            crt = measure.read_object(crt_path)
            crt["coff_directive_record_count"] = 0
            self.rewrite(crt_path, crt)
        self.reauthenticate(receipt)
        return receipt, environment

    def test_receipt_can_record_zero_directives_with_independent_static_crt_proof(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt, environment = self.no_directive_fixture(root)
            self.verify(receipt, root, environment)
            for profile in ("debug", "release"):
                self.assertEqual(measure.read_object(root / f"{profile}-crt.json")["coff_directive_record_count"], 0)
                self.assertNotIn("Directive(s):", (root / f"{profile}-coff.log").read_text(encoding="utf-8"))
            self.assertIn("libcmt.lib", MsvcObservationTests.libraries)

    def test_rehashed_receipt_cannot_forge_missing_or_noninteger_directive_counts(self) -> None:
        for count in (1, -1, "0", False, None):
            with self.subTest(count=count), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt, environment = self.no_directive_fixture(root)
                crt_path = root / "debug-crt.json"
                crt = measure.read_object(crt_path)
                if count is None:
                    del crt["coff_directive_record_count"]
                else:
                    crt["coff_directive_record_count"] = count
                self.rewrite(crt_path, crt)
                expected = self.reauthenticate(receipt)
                with self.assertRaises(measure.MeasurementError):
                    self.verify(receipt, root, environment, expected)

    def raw_defaultlib_fixture(self, root: Path) -> tuple[Path, dict[str, str]]:
        receipt, environment = self.fixture(root)
        record = measure.read_object(receipt)
        archives = record.get("archives")
        if not isinstance(archives, list):
            self.fail("fixture must retain archive rows")
        for row in archives:
            if not isinstance(row, dict):
                self.fail("fixture must retain archive objects")
            row["native_library_args"] = MsvcObservationTests.raw_libraries
        self.rewrite(receipt, record)
        for profile in ("debug", "release"):
            build_log = root / f"{profile}-build.log"
            build_log.write_text(build_log.read_text(encoding="utf-8").replace(
                " ".join(MsvcObservationTests.libraries), " ".join(MsvcObservationTests.raw_libraries)),
                encoding="utf-8")
            library_path = root / f"{profile}-libraries.json"
            libraries = measure.read_object(library_path)
            libraries["arguments"] = MsvcObservationTests.raw_libraries
            self.rewrite(library_path, libraries)
        self.reauthenticate(receipt)
        return receipt, environment

    def test_receipt_accepts_raw_defaultlib_token_and_retains_the_original_vector(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt, environment = self.raw_defaultlib_fixture(root)
            self.verify(receipt, root, environment)
            for row in self.rows(receipt):
                self.assertEqual(row["native_library_args"], MsvcObservationTests.raw_libraries)

    def test_rehashed_receipt_cannot_replace_observed_crt_flag_with_an_equivalent_library(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            receipt, environment = self.raw_defaultlib_fixture(root)
            normalized = [*MsvcObservationTests.raw_libraries[:-1], "libcmt.lib"]
            record = measure.read_object(receipt)
            archives = record.get("archives")
            if not isinstance(archives, list) or not isinstance(archives[0], dict):
                self.fail("fixture must retain archive objects")
            archives[0]["native_library_args"] = normalized
            self.rewrite(receipt, record)
            libraries_path = root / "debug-libraries.json"
            libraries = measure.read_object(libraries_path)
            libraries["arguments"] = normalized
            self.rewrite(libraries_path, libraries)
            expected = self.reauthenticate(receipt)
            with self.assertRaises(measure.MeasurementError):
                self.verify(receipt, root, environment, expected)

class MsvcGateTests(unittest.TestCase):
    occupied_tests = (
        "resource_execution::occupied_struct_harness::native_resource_occupied_struct_scripts_retire_before_teardown",
        "resource_execution::occupied_struct_harness::native_resource_occupied_struct_scripts_reenter_with_same_provider_grant",
    )

    def run_fixture(self, root: Path, *, fresh: bool = False, mutate_gate: str | None = None,
                    execution: str = "running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n") -> tuple[
                        dict[str, measure.Json], list[dict[str, str]], list[dict[str, str]]]:
        receipt, environment = MsvcReceiptTests().fixture(root)
        workflow = root / ".github/workflows/native.yml"
        workflow.parent.mkdir(parents=True)
        workflow.write_bytes(b"synthetic complete Source fixture")
        expected = measure.digest(receipt)
        compiler = root / "tool chain/rustc.exe"
        linker = root / "tool chain/link.exe"
        binary = root / "compiled gate.exe"
        target = root / "existing gate target"
        target.mkdir()
        compile_environments: list[dict[str, str]] = []
        runtime_environments: list[dict[str, str]] = []
        def compile_gate(argv: list[str], *, cwd: Path, env: dict[str, str], stdin: int,
                         stdout: BinaryIO, stderr: BinaryIO, timeout: int) -> subprocess.CompletedProcess[bytes]:
            self.assertEqual(cwd, root)
            self.assertEqual(stdin, subprocess.DEVNULL)
            self.assertIn("--no-run", argv)
            self.assertIn("--offline", argv)
            self.assertEqual(argv[argv.index("--target") + 1], "x86_64-pc-windows-msvc")
            self.assertEqual(argv[argv.index("--target-dir") + 1], str(target))
            self.assertNotIn("--cfg", argv)
            compile_environments.append(dict(env))
            binary.write_bytes(b"synthetic freshly compiled gate")
            binary.with_suffix(".d").write_text(
                f"# env-dep:JETT_RESOURCE_NATIVE_TEST_ARCHIVE_EXPECTED_SHA256_V1={env[measure.PIN_ENV]}\n",
                encoding="utf-8")
            stderr.write((f'Running `"{compiler}" --crate-name native_conformance --test '
                '--target x86_64-pc-windows-msvc --emit=dep-info,link '
                f'-C target-feature=+crt-static -C debuginfo=0 -C "linker={linker}"`\n').encode())
            stdout.write((json.dumps({"reason": "compiler-artifact", "fresh": fresh,
                "target": {"name": "native_conformance"}, "executable": str(binary)}) + "\n").encode())
            return subprocess.CompletedProcess(argv, 0)
        def execute_gate(argv: list[str], repo: Path, env: dict[str, str],
                         log: Path, timeout: int = 3600) -> str:
            self.assertEqual(repo, root)
            if argv == [str(compiler), "-vV"]:
                self.assertNotIn(measure.PIN_ENV, env)
                text = "host: x86_64-pc-windows-msvc\n"
            else:
                self.assertEqual(argv, [str(binary), self.occupied_tests[len(runtime_environments)],
                    "--ignored", "--exact", "--test-threads=1", "--nocapture"])
                runtime_environments.append(dict(env))
                if mutate_gate == "binary":
                    binary.write_bytes(b"changed gate during execution")
                elif mutate_gate == "dep-info":
                    binary.with_suffix(".d").write_bytes(b"changed compiled pin evidence during execution")
                text = execution
            log.write_text(text, encoding="utf-8")
            return text
        def tool(name: str) -> Path:
            return root / "tool chain" / (name if name.endswith(".exe") else name + ".exe")
        with patch.dict(os.environ, environment, clear=True), \
             patch.object(measure.shutil, "which", return_value=str(linker)), \
             patch.object(measure, "executable", side_effect=tool), \
             patch.object(measure.subprocess, "run", side_effect=compile_gate), \
             patch.object(measure, "logged", side_effect=execute_gate):
            measure.run_gate(root, receipt, expected, target, target="x86_64-pc-windows-msvc",
                             suite="occupied-structs", reuse_target_dir=True)
        return measure.read_object(root / "gate-acceptance.json"), compile_environments, runtime_environments

    def test_occupied_gates_compile_the_pin_and_execute_each_exact_test_once(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            accepted, compiled, executed = self.run_fixture(root)
            self.assertEqual(len(compiled), 1)
            self.assertEqual(len(executed), 2)
            self.assertEqual(compiled[0][measure.PIN_ENV], accepted["compile_time_receipt_sha256"])
            self.assertNotIn(measure.RECEIPT_ENV, compiled[0])
            for environment in executed:
                self.assertNotIn(measure.PIN_ENV, environment)
                self.assertEqual(environment[measure.RECEIPT_ENV], str(root / "receipt.json"))
            self.assertEqual(accepted["suite"], "occupied-structs")
            observations = accepted["gate_executions"]
            if not isinstance(observations, list):
                self.fail("accepted executions must be an observed list")
            self.assertEqual([row.get("test") for row in observations if isinstance(row, dict)],
                             list(self.occupied_tests))
            self.assertNotIn("cases", accepted)
            self.assertNotIn("source_deleted_executions", accepted)
            self.assertEqual(accepted["runtime_expected_pin_environment"], False)

    def test_reused_target_cannot_accept_cached_compilation_or_empty_test_selection(self) -> None:
        for fresh, execution in ((True, "running 1 test\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n"),
                                 (False, "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored;\n"),
                                 (False, "running 2 tests\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n")):
            with self.subTest(fresh=fresh, execution=execution), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with self.assertRaises(measure.MeasurementError):
                    self.run_fixture(root, fresh=fresh, execution=execution)
                self.assertFalse((root / "gate-acceptance.json").exists())

    def test_compiler_dep_info_requires_one_exact_pin_and_fresh_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "native_conformance.exe"
            binary.write_bytes(b"synthetic test binary")
            expected = "a" * 64
            observation: dict[str, measure.Json] = {"reason": "compiler-artifact", "fresh": False,
                "target": {"name": "native_conformance"}, "executable": str(binary)}
            message = json.dumps(observation)
            dep_info = binary.with_suffix(".d")
            exact_pin = f"# env-dep:JETT_RESOURCE_NATIVE_TEST_ARCHIVE_EXPECTED_SHA256_V1={expected}\n"
            with self.assertRaises(measure.MeasurementError):
                measure.checked_msvc_gate_artifact(message, binary, expected)
            dep_info.write_text(exact_pin, encoding="utf-8")
            self.assertEqual(measure.checked_msvc_gate_artifact(message, binary, expected), dep_info)
            for invalid in ("", exact_pin + exact_pin, exact_pin.replace(expected, "b" * 64)):
                dep_info.write_text(invalid, encoding="utf-8")
                with self.subTest(dep_info=invalid), self.assertRaises(measure.MeasurementError):
                    measure.checked_msvc_gate_artifact(message, binary, expected)
            dep_info.write_text(exact_pin, encoding="utf-8")
            cached = {**observation, "fresh": True}
            missing_fresh = {key: value for key, value in observation.items() if key != "fresh"}
            foreign = {**observation, "executable": str(binary.parent / "foreign.exe")}
            for invalid in (json.dumps(cached), json.dumps(missing_fresh), json.dumps(foreign), message + "\n" + message):
                with self.subTest(message=invalid), self.assertRaises(measure.MeasurementError):
                    measure.checked_msvc_gate_artifact(invalid, binary, expected)

    def test_unknown_suite_and_implicit_target_reuse_stop_before_compilation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "existing"
            target.mkdir()
            with patch.object(measure, "receipt_sources") as receipt_sources, \
                 patch.object(measure.subprocess, "run") as compile_gate:
                for suite, reuse in (("foreign", True), ("occupied-structs", False)):
                    with self.subTest(suite=suite, reuse=reuse), self.assertRaises(measure.MeasurementError):
                        measure.run_gate(root, root / "absent-receipt.json", "a" * 64, target,
                            target="x86_64-pc-windows-msvc", suite=suite, reuse_target_dir=reuse)
                receipt_sources.assert_not_called()
                compile_gate.assert_not_called()

    def test_gate_binary_and_compiled_pin_evidence_cannot_change_during_execution(self) -> None:
        for changed in ("binary", "dep-info"):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                with self.assertRaises(measure.MeasurementError):
                    self.run_fixture(root, mutate_gate=changed)
                self.assertFalse((root / "gate-acceptance.json").exists())

class MsvcLinkerVersionTests(unittest.TestCase):
    help_text = ("Microsoft (R) Incremental Linker Version 14.44.35213.0\n"
                 "Copyright (C) Microsoft Corporation. All rights reserved.\n\n"
                 "usage: LINK [options] [files] [@commandfile]\n\n"
                 "   options:\n\n"
                 "      /ALIGN:#\n      /DEBUG\n")

    def observe(self, root: Path, text: str, returncode: int, *, generic: bool = False) -> tuple[str, int]:
        linker = root / "measured link.exe"
        linker.write_bytes(b"synthetic linker executable")
        log = root / "linker-version.log"
        environment = {"LIB": r"C:\SDK Libraries\x64;", "PATH": str(root)}
        def version_probe(argv: list[str], *, cwd: Path, env: dict[str, str], stdin: int,
                          stdout: BinaryIO, stderr: int, timeout: int) -> subprocess.CompletedProcess[bytes]:
            self.assertEqual(argv, [str(linker), "/?"])
            self.assertEqual(cwd, root)
            self.assertEqual(env, environment)
            self.assertEqual(stdin, subprocess.DEVNULL)
            self.assertEqual(stderr, subprocess.STDOUT)
            stdout.write(text.encode("utf-8"))
            return subprocess.CompletedProcess(argv, returncode)
        with patch.object(measure.subprocess, "run", side_effect=version_probe) as run:
            if generic:
                result = measure.logged([str(linker), "/?"], root, environment, log, timeout=60)
                observed = (result, returncode)
            else:
                observed = measure.msvc_linker_version(linker, root, environment, log)
        run.assert_called_once()
        self.assertEqual(log.read_text(encoding="utf-8"), text)
        return observed

    def test_microsoft_help_exit_1100_and_zero_retain_the_actual_version_observation(self) -> None:
        for returncode in (0, 1100):
            with self.subTest(returncode=returncode), tempfile.TemporaryDirectory() as temporary:
                self.assertEqual(self.observe(Path(temporary), self.help_text, returncode),
                                 (self.help_text, returncode))

    def test_error_exit_1100_cannot_authenticate_even_with_a_valid_help_prefix(self) -> None:
        for diagnostic in ("LINK : fatal error LNK1158: cannot run 'rc.exe'\n",
                           "LINK : error LNK2001: unresolved external symbol main\n"):
            for prefix in ("", self.help_text):
                with self.subTest(diagnostic=diagnostic, help_prefix=bool(prefix)), \
                     tempfile.TemporaryDirectory() as temporary, self.assertRaises(measure.MeasurementError):
                    self.observe(Path(temporary), prefix + diagnostic, 1100)

    def test_unsupported_status_or_incomplete_or_foreign_help_is_refused(self) -> None:
        invalid = ((self.help_text, 1), (self.help_text, 2),
                   ("Microsoft (R) Incremental Linker Version 14.44.35213.0\n", 1100),
                   (self.help_text.replace("usage: LINK [options] [files] [@commandfile]\n", ""), 1100),
                   (self.help_text.replace("   options:\n", ""), 1100),
                   (self.help_text.replace("Microsoft (R) Incremental Linker Version 14.44.35213.0",
                                           "LLVM Linker Version 21.0.0"), 1100))
        for text, returncode in invalid:
            with self.subTest(text=text, returncode=returncode), tempfile.TemporaryDirectory() as temporary, \
                 self.assertRaises(measure.MeasurementError):
                self.observe(Path(temporary), text, returncode)

    def test_generic_logged_still_rejects_valid_help_with_nonzero_exit_status(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, self.assertRaises(measure.MeasurementError):
            self.observe(Path(temporary), self.help_text, 1100, generic=True)

    def test_toolchain_receipt_records_the_actual_linker_help_status_and_hashed_log(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "observations"
            directory.mkdir()
            tools = measure.MsvcTools(root / "cargo.exe", root / "rustc.exe", root / "llvm-nm.exe",
                                      root / "llvm-readobj.exe", root / "link.exe")
            for file in (tools.cargo, tools.rustc, tools.nm, tools.readobj, tools.linker):
                file.write_bytes(b"synthetic measured tool")
            expected: dict[str, tuple[str, str, int]] = {
                str(tools.cargo): ("-vV", "cargo 1.92.0\n", 0),
                str(tools.rustc): ("-vV", "rustc 1.92.0\nhost: x86_64-pc-windows-msvc\n", 0),
                str(tools.nm): ("--version", "LLVM version 21.0.0\n", 0),
                str(tools.readobj): ("--version", "LLVM version 21.0.0\n", 0),
                str(tools.linker): ("/?", self.help_text, 1100),
            }
            environment = {"LIB": r"C:\SDK\x64;", "PATH": str(root)}
            def observe(argv: list[str], *, cwd: Path, env: dict[str, str], stdin: int,
                        stdout: BinaryIO, stderr: int, timeout: int) -> subprocess.CompletedProcess[bytes]:
                argument, text, returncode = expected[argv[0]]
                self.assertEqual(argv, [argv[0], argument])
                self.assertEqual(cwd, root)
                self.assertEqual(env, environment)
                self.assertEqual(stdin, subprocess.DEVNULL)
                self.assertEqual(stderr, subprocess.STDOUT)
                stdout.write(text.encode("utf-8"))
                return subprocess.CompletedProcess(argv, returncode)
            with patch.object(measure.subprocess, "run", side_effect=observe) as run:
                recorded = measure.msvc_toolchain(tools, root, directory, environment)
            self.assertEqual(run.call_count, 5)
            linker = recorded["linker"]
            if not isinstance(linker, dict):
                self.fail("toolchain linker observation must be an object")
            self.assertEqual(linker["returncode"], 1100)
            self.assertEqual(linker["version_observation"], measure.artifact(directory / "linker-version.log"))
            self.assertEqual(linker["executable"], measure.artifact(tools.linker))
            for name in ("rustc", "cargo", "nm", "readobj"):
                observed = recorded[name]
                if not isinstance(observed, dict):
                    self.fail("toolchain version observation must be an object")
                self.assertEqual(observed["returncode"], 0)

if __name__ == "__main__":
    unittest.main()
