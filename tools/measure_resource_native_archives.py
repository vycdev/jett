#!/usr/bin/env python3
"""Measure fresh GNU private Resource archives, then run their pinned Source gate.

These archives are compiler-test artifacts, never production launcher packages.
Preparation and gate execution are separate commands and separate processes.
"""
from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
from typing import TypeAlias


ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
CFGS = ["test", "jett_resource_native_test_archive"]
PIN_ENV = "JETT_RESOURCE_NATIVE_TEST_ARCHIVE_EXPECTED_SHA256_V1"
RECEIPT_ENV = "JETT_RESOURCE_NATIVE_TEST_ARCHIVE_RECEIPT_V1"
TEST = ("resource_execution::"
        "native_resource_original_source_lifecycle_matches_reference_and_retires_before_teardown")
Json: TypeAlias = str | int | bool | None | list["Json"] | dict[str, "Json"]


class MeasurementError(RuntimeError):
    """A build or independent observation cannot support an acceptance receipt."""


@dataclass(frozen=True)
class Tools:
    cargo: Path
    rustc: Path
    nm: Path
    readelf: Path
    cc: Path


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MeasurementError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def artifact(path: Path) -> dict[str, Json]:
    require(path.is_file(), f"missing measured file: {path}")
    return {"path": str(path.resolve()), "sha256": digest(path)}


def write_json(path: Path, value: Json) -> None:
    with path.open("x", encoding="utf-8", newline="\n") as output:
        output.write(json.dumps(value, indent=2, sort_keys=True) + "\n")


def normalize_json(value: object) -> Json:
    if value is None or isinstance(value, (str, bool, int)):
        return value
    if isinstance(value, list):
        return [normalize_json(item) for item in value]
    if isinstance(value, dict):
        output: dict[str, Json] = {}
        for key, item in value.items():
            require(isinstance(key, str), "JSON object keys must be strings")
            if isinstance(key, str):
                output[key] = normalize_json(item)
        return output
    raise MeasurementError("unexpected JSON value in measured record")


def read_object(path: Path) -> dict[str, Json]:
    raw: object = json.loads(path.read_text(encoding="utf-8"))
    value = normalize_json(raw)
    require(isinstance(value, dict), f"expected JSON object: {path}")
    if isinstance(value, dict):
        return value
    raise MeasurementError(f"expected JSON object: {path}")


def executable(value: str) -> Path:
    found = shutil.which(value)
    require(found is not None, f"required executable is unavailable: {value}")
    if found is None:
        raise MeasurementError(f"required executable is unavailable: {value}")
    # Rustup proxy dispatch depends on argv[0]; preserve cargo/rustc symlinks.
    path = Path(found).absolute()
    require(path.is_file(), f"executable must be a regular file: {path}")
    return path


def logged(argv: list[str], directory: Path, environment: dict[str, str],
           log: Path, timeout: int = 3600) -> str:
    with log.open("xb") as output:
        result = subprocess.run(argv, cwd=directory, env=environment, stdin=subprocess.DEVNULL,
                                stdout=output, stderr=subprocess.STDOUT, timeout=timeout)
    text = log.read_text(encoding="utf-8", errors="replace")
    require(result.returncode == 0, f"command exited {result.returncode}; see {log}")
    return text


def sources(repo: Path, complete: bool = False) -> dict[str, Json]:
    paths = {repo / "Cargo.toml", repo / "Cargo.lock",
             repo / "crates/jett_runtime/Cargo.toml",
             repo / "tools/measure_resource_native_archives.py"}
    paths.update((repo / "crates/jett_runtime").rglob("*.rs"))
    paths.update((repo / ".cargo").rglob("*.toml"))
    if complete:
        paths.add(repo / ".github/workflows/native.yml")
        for suffix in ("*.rs", "*.jett", "Cargo.toml"):
            paths.update((repo / "crates").rglob(suffix))
        paths.update((repo / "stdlib").rglob("*.jett"))
    return {str(path.relative_to(repo)): digest(path) for path in sorted(paths)}


def build_environment() -> dict[str, str]:
    environment = os.environ.copy()
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS",
                 "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS"):
        require(not environment.get(name, "").strip(),
                f"GNU dynamic-CRT measurement requires empty {name}")
    for name in ("RUSTC", "CARGO_BUILD_RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                 "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"):
        require(not environment.get(name), f"unmeasured compiler wrapper: {name}")
    # The selected Cargo command alone supplies the two private cfgs.
    environment.pop(PIN_ENV, None)
    environment.pop(RECEIPT_ENV, None)
    # Cargo command identity and rustc's native-library note are parsed as text.
    # Override user/config color preferences instead of discarding escape bytes.
    environment["CARGO_TERM_COLOR"] = "never"
    return environment


def native_libraries(text: str) -> list[str]:
    lines = text.splitlines()
    indices = [index for index, line in enumerate(lines) if "note: native-static-libs:" in line]
    require(len(indices) == 1, "exactly one actual native-static-libs observation is required")
    index = indices[0]
    parts = [lines[index].split("note: native-static-libs:", 1)[1]]
    for line in lines[index + 1:]:
        if not re.fullmatch(r"\s+-\S.*", line):
            break
        parts.append(line.strip())
    arguments = shlex.split(" ".join(parts))
    require(bool(arguments), "native-static-libs vector must not be empty")
    for argument in arguments:
        lower = argument.lower()
        require("jett_runtime" not in lower and "jett_native_launcher" not in lower,
                "a second Rust runtime archive is forbidden")
        require(argument.startswith("-l") and len(argument) > 2,
                f"unexpected GNU native library argument: {argument}")
    require("-lc" in arguments and "-lgcc_s" in arguments,
            "GNU dynamic CRT libraries were not observed")
    return arguments


def selected_cfgs(text: str, compiler: Path | None = None) -> None:
    commands = [line.strip() for line in text.splitlines() if line.strip().startswith("Running ")]
    private = [line for line in commands if "jett_resource_native_test_archive" in line]
    require(len(private) == 1, "private cfg must occur in exactly one executed rustc command")
    command = private[0]
    require("--crate-name jett_runtime " in command, "private cfg escaped selected runtime package")
    require("--cfg test " in command and "--cfg jett_resource_native_test_archive" in command,
            "both selected-package cfgs must occur in the actual rustc command")
    require("--crate-type staticlib" in command, "private runtime staticlib was not built")
    if compiler is not None:
        checked_compiler_commands(text, compiler)


def checked_compiler_commands(text: str, compiler: Path) -> None:
    commands = [line.strip() for line in text.splitlines()
                if line.strip().startswith("Running ") and "--crate-name " in line]
    require(bool(commands), "actual compiler invocation is missing")
    for command in commands:
        arguments = shlex.split(command.removeprefix("Running ").strip("`"))
        require(bool(arguments) and Path(arguments[0]).absolute() == compiler.absolute(),
                "Cargo used an unmeasured compiler or compiler wrapper")


def export_names(repo: Path) -> tuple[set[str], set[str]]:
    directory = repo / "crates/jett_runtime/src/native_abi/resource"
    pattern = r'pub unsafe extern "C" fn (jett_rt_v1_resource_[a-z_]+)\('
    leaves = re.findall(pattern, (directory / "leaves.rs").read_text(encoding="utf-8"))
    private = re.findall(pattern, (directory / "test_archive_api.rs").read_text(encoding="utf-8"))
    require(len(leaves) == len(set(leaves)) == 37, "frozen Resource leaf contract must contain 37 names")
    require(len(private) == len(set(private)) == 8, "frozen private test contract must contain eight names")
    require(all(name.startswith("jett_rt_v1_resource_test_") for name in private),
            "unexpected private export name")
    return set(leaves), set(private)


def check_symbols(text: str, leaves: set[str], private: set[str]) -> None:
    definitions: Counter[str] = Counter()
    production: set[str] = set()
    test_exports: set[str] = set()
    for line in text.splitlines():
        fields = line.split()
        if len(fields) < 3 or len(fields[1]) != 1:
            continue
        name, kind = fields[:2]
        if name == "main" or name.startswith("jett_rt_v1_resource_"):
            require(kind == "T", f"expected one global text definition for {name}, got {kind}")
            definitions[name] += 1
            if name.startswith("jett_rt_v1_resource_test_"):
                test_exports.add(name)
            elif name != "main":
                production.add(name)
    require(production == leaves, "Resource leaf symbol inventory differs from frozen source")
    require(test_exports == private, "private test symbol inventory differs from frozen source")
    for name in leaves | private | {"main"}:
        require(definitions[name] == 1, f"expected exactly one definition of {name}")


def fingerprint_records(directory: Path) -> list[Json]:
    paths = sorted(directory.glob("jett_runtime-*/lib-jett_runtime.json"))
    require(bool(paths), "runtime Cargo fingerprint is missing")
    records: list[Json] = []
    for path in paths:
        value = read_object(path)
        require(value.get("rustflags") == [], "GNU target fingerprint has unexpected rustflags")
        records.append({**artifact(path), "raw_json_utf8": path.read_text(encoding="utf-8")})
    return records


def toolchain(tools: Tools, repo: Path, output: Path, environment: dict[str, str]) -> dict[str, Json]:
    observed: dict[str, Json] = {}
    for name, path, arguments in (
        ("rustc", tools.rustc, ["-vV"]), ("cargo", tools.cargo, ["-vV"]),
        ("nm", tools.nm, ["--version"]), ("readelf", tools.readelf, ["--version"]),
        ("cc", tools.cc, ["--version"]),
    ):
        log = output / f"{name}-version.log"
        text = logged([str(path), *arguments], repo, environment, log)
        observed[name] = {"invocation_path": str(path), "executable": artifact(path),
                          "version_observation": artifact(log)}
        if name == "rustc":
            require(f"host: {TARGET}" in text.splitlines(), "private archives must be built on their GNU host")
    target_log = output / "cc-target.log"
    target = logged([str(tools.cc), "-dumpmachine"], repo, environment, target_log).strip()
    require(re.fullmatch(r"x86_64(?:-[a-z0-9_]+)*-linux-gnu", target) is not None,
            f"unexpected C linker target: {target}")
    observed["cc_target"] = artifact(target_log)
    return observed


def dynamic_crt(tools: Tools, repo: Path, output: Path, profile: str,
                arguments: list[str], environment: dict[str, str]) -> dict[str, Json]:
    source = output / f"{profile}-crt-probe.c"
    source.write_text("int main(void) { return 0; }\n", encoding="utf-8")
    binary = output / f"{profile}-crt-probe"
    command = [str(tools.cc), "-no-pie", "-o", str(binary), str(source), *arguments]
    link_log = output / f"{profile}-crt-probe-link.log"
    logged(command, repo, environment, link_log, timeout=60)
    inspection = output / f"{profile}-crt-probe-elf.log"
    text = logged([str(tools.readelf), "--file-header", "--program-headers", "--dynamic", str(binary)],
                  repo, environment, inspection, timeout=60)
    require(re.search(r"Type:\s+EXEC\b", text) is not None, "GNU probe must be an ELF executable")
    require("INTERP" in text and "(NEEDED)" in text and "libc.so" in text,
            "dynamic GNU CRT interpreter/libc dependency was not observed")
    return {"source": artifact(source), "binary": artifact(binary), "link_command": list(command),
            "link_observation": artifact(link_log), "elf_observation": artifact(inspection),
            "scope": "Measured C driver and native-library vector; not Resource Source execution"}


def measure_profile(tools: Tools, repo: Path, output: Path, profile: str,
                    environment: dict[str, str], inputs: dict[str, Json],
                    versions: dict[str, Json], leaves: set[str], private: set[str]) -> dict[str, Json]:
    target_dir = output / f"{profile}-build"
    command = [str(tools.cargo), "rustc", "--locked", "--color", "never", "--verbose", "-p", "jett_runtime", "--lib",
               "--target", TARGET, "--target-dir", str(target_dir)]
    if profile == "release":
        command.append("--release")
    command.extend(["--", "--cfg", "test", "--cfg", "jett_resource_native_test_archive",
                    "--print=native-static-libs"])
    build_log = output / f"{profile}-build.log"
    text = logged(command, repo, environment, build_log)
    require(sources(repo) == inputs, "runtime inputs changed during private archive build")
    selected_cfgs(text, tools.rustc)
    arguments = native_libraries(text)
    directory = target_dir / TARGET / profile
    archive = directory / "libjett_runtime.a"
    require(archive.is_file() and archive.stat().st_size <= 256 * 1024 * 1024,
            "private archive is missing or exceeds the harness limit")
    archive_hash = digest(archive)
    symbols = output / f"{profile}-symbols.log"
    text = logged([str(tools.nm), "--defined-only", "--extern-only", "--format=posix", str(archive)],
                  repo, environment, symbols)
    check_symbols(text, leaves, private)
    elf = output / f"{profile}-archive-elf.log"
    text = logged([str(tools.readelf), "--file-header", "--symbols", str(archive)], repo, environment, elf)
    require("ELF64" in text and "X86-64" in text, "GNU archive target inspection failed")
    fingerprints = fingerprint_records(directory / ".fingerprint")
    probe = dynamic_crt(tools, repo, output, profile, arguments, environment)
    require(sources(repo) == inputs, "runtime inputs changed during archive measurement")
    require(digest(archive) == archive_hash, "runtime archive changed during independent measurement")
    build = output / f"{profile}-build.json"
    write_json(build, {"profile": profile, "target": TARGET, "toolchain": versions,
                       "runtime_sources": inputs, "workspace_lock": inputs["Cargo.lock"],
                       "cfgs_selected_package_only": list(CFGS), "actual_build_command": list(command),
                       "cargo_build_log": artifact(build_log), "archive": artifact(archive)})
    libraries = output / f"{profile}-native-static-libs.json"
    write_json(libraries, {"actual_print_log": artifact(build_log), "arguments": list(arguments),
                           "order_and_duplication_preserved": True})
    crt = output / f"{profile}-crt.json"
    write_json(crt, {"actual_target_scoped_rustflags": [], "cargo_fingerprints": fingerprints,
                     "native_static_libs": artifact(libraries), "complete_symbol_inventory": artifact(symbols),
                     "complete_elf_archive_inventory": artifact(elf), "dynamic_crt_probe": probe,
                     "linker_path": str(tools.cc), "linker_executable": artifact(tools.cc), "mode": "dynamic"})
    return {"profile": profile, "target": TARGET, "archive_path": str(archive),
            "archive_sha256": archive_hash, "runtime_abi": 1, "layout_wire": 2,
            "private_cfgs": list(CFGS), "crt_mode": "dynamic", "linker_path": str(tools.cc),
            "native_library_args": list(arguments), "build_receipt": artifact(build),
            "native_static_libs_receipt": artifact(libraries), "crt_receipt": artifact(crt)}


def prepare(repo: Path, output: Path, cc: str, nm: str, readelf: str) -> tuple[Path, str]:
    require(not output.exists(), "preserve immutable measurements: choose a fresh output directory")
    environment = build_environment()
    tools = Tools(executable("cargo"), executable("rustc"), executable(nm), executable(readelf), executable(cc))
    # Pin Cargo to the measured rustup proxy and disable config-provided wrappers.
    environment["RUSTC"] = str(tools.rustc)
    environment["RUSTC_WRAPPER"] = ""
    environment["RUSTC_WORKSPACE_WRAPPER"] = ""
    output.mkdir(parents=True)
    inputs = sources(repo)
    versions = toolchain(tools, repo, output, environment)
    leaves, private = export_names(repo)
    archives: list[Json] = []
    for profile in ("debug", "release"):
        archives.append(measure_profile(tools, repo, output, profile, environment, inputs,
                                        versions, leaves, private))
    receipt = output / "receipt.json"
    write_json(receipt, {"version": 1, "scope": "Measured compiler-test GNU archives; not Source execution",
                         "archives": archives})
    return receipt, digest(receipt)


def measured_artifact(record: Json) -> Path:
    require(isinstance(record, dict), "measured artifact must be an object")
    if not isinstance(record, dict):
        raise MeasurementError("measured artifact must be an object")
    path, expected = record.get("path"), record.get("sha256")
    require(isinstance(path, str) and isinstance(expected, str), "artifact path/hash are required")
    if not isinstance(path, str) or not isinstance(expected, str):
        raise MeasurementError("artifact path/hash are required")
    file = Path(path)
    require(file.is_absolute() and file.is_file() and file.stat().st_size <= 256 * 1024 * 1024
            and digest(file) == expected,
            f"measured artifact changed: {file}")
    return file


def verify_embedded_artifacts(value: Json) -> None:
    if isinstance(value, dict):
        if "path" in value and "sha256" in value:
            measured_artifact(value)
            raw = value.get("raw_json_utf8")
            if isinstance(raw, str):
                require(hashlib.sha256(raw.encode("utf-8")).hexdigest() == value["sha256"],
                        "retained Cargo fingerprint bytes differ from their observation")
        for item in value.values():
            verify_embedded_artifacts(item)
    elif isinstance(value, list):
        for item in value:
            verify_embedded_artifacts(item)


def receipt_sources(receipt: Path, expected: str, repo: Path) -> None:
    require(re.fullmatch(r"[0-9a-f]{64}", expected) is not None, "expected SHA-256 must be canonical")
    require(receipt.is_file() and receipt.stat().st_size <= 1024 * 1024,
            "receipt exceeds the test harness limit")
    require(digest(receipt) == expected, "receipt differs from the independently supplied compile-time pin")
    record = read_object(receipt)
    require(record.get("version") == 1, "unsupported archive receipt version")
    archives = record.get("archives")
    require(isinstance(archives, list) and len(archives) == 2, "two measured runtime profiles are required")
    if not isinstance(archives, list):
        raise MeasurementError("archive rows are required")
    profiles: set[str] = set()
    for row in archives:
        require(isinstance(row, dict), "archive row must be an object")
        if not isinstance(row, dict):
            raise MeasurementError("archive row must be an object")
        profile = row.get("profile")
        require(isinstance(profile, str) and profile in ("debug", "release"), "unknown runtime profile")
        if isinstance(profile, str):
            profiles.add(profile)
        require(row.get("target") == TARGET and row.get("runtime_abi") == 1
                and row.get("layout_wire") == 2 and row.get("private_cfgs") == CFGS
                and row.get("crt_mode") == "dynamic", "GNU archive contract differs from measured gate")
        measured_artifact({"path": row.get("archive_path"), "sha256": row.get("archive_sha256")})
        for name in ("build_receipt", "native_static_libs_receipt", "crt_receipt"):
            provenance = read_object(measured_artifact(row.get(name)))
            verify_embedded_artifacts(provenance)
        build = read_object(measured_artifact(row.get("build_receipt")))
        require(build.get("runtime_sources") == sources(repo), "measured runtime source differs from current tree")
        libraries = read_object(measured_artifact(row.get("native_static_libs_receipt")))
        require(row.get("native_library_args") == libraries.get("arguments"), "native library vector changed")
        crt = read_object(measured_artifact(row.get("crt_receipt")))
        require(crt.get("mode") == "dynamic" and crt.get("linker_path") == row.get("linker_path"),
                "GNU linker/CRT identity differs from receipt")
        linker = measured_artifact(crt.get("linker_executable"))
        path = row.get("linker_path")
        require(isinstance(path, str), "measured linker path is missing")
        if isinstance(path, str):
            require(Path(path).resolve(strict=True) == linker.resolve(strict=True), "measured linker changed")
    require(profiles == {"debug", "release"}, "measured profiles must be unique")


def compiled_test(messages: str) -> Path:
    executables: set[Path] = set()
    for line in messages.splitlines():
        value = normalize_json(json.loads(line))
        if not isinstance(value, dict) or value.get("reason") != "compiler-artifact":
            continue
        target = value.get("target")
        executable_path = value.get("executable")
        if isinstance(target, dict) and target.get("name") == "native_conformance" and isinstance(executable_path, str):
            executables.add(Path(executable_path))
    require(len(executables) == 1, "one compiled native_conformance executable must be selected")
    executable_path = next(iter(executables))
    require(executable_path.is_absolute() and executable_path.is_file(), "compiled gate executable is missing")
    return executable_path


def run_gate(repo: Path, receipt: Path, expected: str, target_dir: Path) -> None:
    require(not target_dir.exists(), "compile the receipt pin in a fresh gate target directory")
    receipt_sources(receipt, expected, repo)
    environment = build_environment()
    compiler = executable("rustc")
    environment["RUSTC"] = str(compiler)
    environment["RUSTC_WRAPPER"] = ""
    environment["RUSTC_WORKSPACE_WRAPPER"] = ""
    inputs = sources(repo, complete=True)
    output = receipt.parent
    compiler_log = output / "gate-rustc-version.log"
    version = logged([str(compiler), "-vV"], repo, environment, compiler_log)
    require(f"host: {TARGET}" in version.splitlines(), "Source gate must compile on its measured GNU host")
    environment[PIN_ENV] = expected
    command = [str(executable("cargo")), "test", "--locked", "--color", "never", "-p", "jett_driver", "--test", "native_conformance",
               "--target", TARGET, "--target-dir", str(target_dir), "--verbose", "--no-run", "--message-format=json"]
    messages = output / "gate-compile-messages.jsonl"
    errors = output / "gate-compile-stderr.log"
    with messages.open("xb") as stdout, errors.open("xb") as stderr:
        result = subprocess.run(command, cwd=repo, env=environment, stdin=subprocess.DEVNULL,
                                stdout=stdout, stderr=stderr, timeout=3600)
    require(result.returncode == 0, f"gate compilation failed; see {errors}")
    checked_compiler_commands(errors.read_text(encoding="utf-8"), compiler)
    binary = compiled_test(messages.read_text(encoding="utf-8"))
    require(sources(repo, complete=True) == inputs, "compiler/Source inputs changed during gate compilation")
    receipt_sources(receipt, expected, repo)
    environment.pop(PIN_ENV)
    environment[RECEIPT_ENV] = str(receipt)
    gate_log = output / "gate-execution.log"
    text = logged([str(binary), TEST, "--ignored", "--exact", "--test-threads=1", "--nocapture"],
                  repo, environment, gate_log, timeout=3600)
    require("1 passed; 0 failed; 0 ignored;" in text, "the complete ignored Resource gate did not execute")
    summary = re.findall(r"Resource native acceptance: (\d+) cases; (\d+) Source-deleted executions; profiles=debug,release", text)
    require(len(summary) == 1, "complete Source execution summary is missing")
    count, executions = (int(value) for value in summary[0])
    require(count > 0 and executions == 2 * count, "both archive profiles must execute the complete corpus")
    require(sources(repo, complete=True) == inputs, "compiler/Source inputs changed during gate execution")
    receipt_sources(receipt, expected, repo)
    write_json(output / "gate-acceptance.json", {"scope": "Complete shared Source corpus on private GNU archives",
               "target": TARGET, "cases": count, "source_deleted_executions": executions,
               "profiles": ["debug", "release"], "compile_time_receipt_sha256": expected,
               "receipt": artifact(receipt), "compiler_and_source_inputs": inputs,
               "compiler_executable": artifact(compiler), "compiler_version": artifact(compiler_log),
               "compile_command": list(command), "compile_messages": artifact(messages),
               "compile_stderr": artifact(errors), "compiled_gate": artifact(binary),
               "gate_execution": artifact(gate_log), "runtime_expected_pin_environment": False})


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    preparation = commands.add_parser("prepare", help="build and independently measure both private archives")
    preparation.add_argument("--output", type=Path, required=True)
    preparation.add_argument("--cc", default=os.environ.get("JETT_NATIVE_CC", "cc"))
    preparation.add_argument("--nm", default="nm")
    preparation.add_argument("--readelf", default="readelf")
    preparation.add_argument("--github-output", type=Path)
    gate = commands.add_parser("run", help="compile the independently supplied receipt pin and run Source gate")
    gate.add_argument("--receipt", type=Path, required=True)
    gate.add_argument("--expected-sha256", required=True)
    gate.add_argument("--target-dir", type=Path, required=True)
    arguments = parser.parse_args()
    if arguments.command == "prepare":
        receipt, expected = prepare(ROOT, arguments.output.resolve(), arguments.cc, arguments.nm, arguments.readelf)
        if arguments.github_output is not None:
            require("\n" not in str(receipt) and "\r" not in str(receipt), "receipt path cannot contain a newline")
            with arguments.github_output.open("a", encoding="utf-8", newline="\n") as output:
                output.write(f"receipt={receipt}\nsha256={expected}\n")
        print(json.dumps({"receipt": str(receipt), "sha256": expected, "native_resource_source_execution": False}))
    else:
        run_gate(ROOT, arguments.receipt.resolve(), arguments.expected_sha256, arguments.target_dir.resolve())


if __name__ == "__main__":
    main()
