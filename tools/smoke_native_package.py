#!/usr/bin/env python3
"""Check a relocated package without Cargo or the Jett source tree."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def command(args: list[str | Path], cwd: Path, env: dict[str, str], success: bool = True) -> subprocess.CompletedProcess:
    result = subprocess.run([str(arg) for arg in args], cwd=cwd, env=env, capture_output=True, text=True, timeout=120)
    if (result.returncode == 0) != success:
        raise AssertionError(f"{args}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    args = parser.parse_args()
    package = args.package.resolve()
    compiler = package / "bin" / ("jett.exe" if os.name == "nt" else "jett")
    env = os.environ.copy()
    env.pop("JETT_STDLIB_DIR", None)
    env.pop("JETT_NATIVE_RUNTIME_BUNDLE", None)
    # This added namespace proves installed-stdlib discovery won over any
    # development checkout that happens to remain on the packaging machine.
    probe = package / "lib/jett/stdlib/package_smoke_probe.jett"
    if probe.exists():
        raise RuntimeError(f"unexpected existing smoke probe: {probe}")
    probe.write_text('namespace package_smoke_probe\nexport function message() returns string:\n    return "installed stdlib"\n', encoding="utf-8")
    try:
        with tempfile.TemporaryDirectory(prefix="jett-package-smoke-") as temporary:
            work = Path(temporary)
            source = work / "hello.jett"
            source_text = ('function main(stdout: Stdout) returns nothing:\n'
                           '    int64 unused = 42\n'
                           '    string marker = package_smoke_probe.message()\n'
                           '    trace marker\n'
                           '    Stdout.write(view stdout, "{marker}\\n")\n')
            for release in (False, True):
                source.write_text(source_text, encoding="utf-8")
                binary = work / ("release.exe" if release else "debug.exe")
                build = [compiler, "build", source, "-o", binary]
                if release:
                    build.append("--release")
                compiled = command(build, work, env)
                assert "warning[E0202]" in compiled.stderr, compiled
                source.unlink()
                result = command([binary], work, env)
                assert result.stdout == "installed stdlib\n", result
                expected_debug = "" if release else "trace marker: string = installed stdlib\n"
                assert result.stderr == expected_debug, result
            # Failed builds preserve an existing output and expose diagnostics.
            source.write_text("function main() returns nothing:\n    println(1)\n", encoding="utf-8")
            existing = work / "preserved.exe"
            existing.write_bytes(b"existing artifact")
            failed = command([compiler, "build", source, "--release", "-o", existing], work, env, False)
            assert "E0362" in failed.stderr, failed
            assert existing.read_bytes() == b"existing artifact"
            source.write_text(source_text, encoding="utf-8")
            manifests = sorted((package / "lib/jett/runtime").glob("*/debug/launcher.json"))
            assert len(manifests) == 1, manifests
            manifest = manifests[0]
            metadata = json.loads(manifest.read_text(encoding="utf-8"))
            # Explicit mismatched runtime profiles must fail before publication.
            failed = command([compiler, "build", source, "--release", "--runtime-bundle", manifest, "-o", existing], work, env, False)
            assert "profile mismatch" in failed.stderr, failed
            assert existing.read_bytes() == b"existing artifact"
            # Agent mode identifies the artifact, and default paths separate profiles.
            result = command([compiler, "build", source, "--agent"], work, env)
            assert "status: ok" in result.stdout and "artifact:" in result.stdout, result
            assert "E0202" in result.stdout and "unused" in result.stdout, result
            name = "hello.exe" if os.name == "nt" else "hello"
            assert (work / "target" / metadata["target"] / "debug" / name).is_file()
            command([compiler, "build", source, "--check"], work, env)
            failed = command([compiler, "build", "absent.jett", "--target", "wasm32-unknown-unknown"], work, env, False)
            assert "unsupported native target" in failed.stderr, failed
    finally:
        probe.unlink()
    print("Native package smoke checks passed (debug, release, relocation, diagnostics, runtime profile, and atomic output).")


if __name__ == "__main__":
    main()
