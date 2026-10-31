#!/usr/bin/env python3
"""Build a relocatable host-native compiler package with both runtime profiles.

Usage: python3 tools/package_native.py --output target/package [--compiler-profile debug]
Requires the host Rust toolchain and C linker/SDK. It never cross-compiles.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(*args: str, env: dict[str, str], capture: bool = False) -> str:
    result = subprocess.run(args, cwd=ROOT, env=env, check=True, text=True,
                            stdout=subprocess.PIPE if capture else None)
    return result.stdout if capture else ""


def copy_notices(package: Path, metadata: dict, env: dict[str, str]) -> None:
    notices = package / "licenses"
    notices.mkdir()
    entries = []
    for dependency in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        source = Path(dependency["manifest_path"]).parent
        files = set(source.glob("LICENSE*")) | set(source.glob("COPYING*")) | set(source.glob("NOTICE*"))
        if dependency.get("license_file"):
            files.add(source / dependency["license_file"])
        destination = notices / f'{dependency["name"]}-{dependency["version"]}'
        for file in sorted(files):
            if file.is_file():
                destination.mkdir(exist_ok=True)
                shutil.copy2(file, destination / file.name)
        entries.append({key: dependency.get(key) for key in ("name", "version", "license", "repository")})
    (notices / "packages.json").write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")
    rust_docs = Path(run("rustc", "--print", "sysroot", env=env, capture=True).strip()) / "share/doc/rust"
    rust_notices = notices / "rust"
    rust_notices.mkdir()
    for pattern in ("LICENSE*", "COPYRIGHT*"):
        for file in rust_docs.glob(pattern):
            if file.is_file():
                shutil.copy2(file, rust_notices / file.name)
    shutil.copy2(ROOT / "Cargo.lock", package / "Cargo.lock")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--compiler-profile", choices=("debug", "release"), default="release")
    args = parser.parse_args()
    output = args.output.resolve()
    if output.exists():
        parser.error(f"output already exists: {output}; choose a new package directory")
    env = os.environ.copy()
    host = next(line.removeprefix("host: ") for line in
                run("rustc", "-vV", env=env, capture=True).splitlines() if line.startswith("host: "))
    windows = host == "x86_64-pc-windows-msvc"
    if not windows and host != "x86_64-unknown-linux-gnu":
        parser.error(f"unsupported native packaging host: {host}")
    if windows:
        # Explicit --target applies these flags to target artifacts, leaving
        # host procedural macros and build scripts with their ordinary ABI.
        flags = env.get("CARGO_ENCODED_RUSTFLAGS")
        if flags is not None:
            env["CARGO_ENCODED_RUSTFLAGS"] = flags + "\x1f-Ctarget-feature=+crt-static"
        else:
            env["RUSTFLAGS"] = env.get("RUSTFLAGS", "") + " -Ctarget-feature=+crt-static"
    metadata = json.loads(run("cargo", "metadata", "--locked", "--format-version=1", env=env, capture=True))
    target = Path(metadata["target_directory"]) / host
    compiler_flags = ["--release"] if args.compiler_profile == "release" else []
    run("cargo", "build", "--locked", "-p", "jett_cli", "--target", host, *compiler_flags, env=env)
    for profile in ("debug", "release"):
        flags = ["--release"] if profile == "release" else []
        run("cargo", "build", "--locked", "-p", "jett_native_launcher", "--target", host, *flags, env=env)
    output.parent.mkdir(parents=True, exist_ok=True)
    # Publish only after all compiler/runtime files and notices are present.
    with tempfile.TemporaryDirectory(prefix="jett-package-", dir=output.parent) as staging:
        package = Path(staging) / "package"
        binary_dir = package / "bin"
        binary_dir.mkdir(parents=True)
        binary_name = "jett.exe" if windows else "jett"
        shutil.copy2(target / args.compiler_profile / binary_name, binary_dir / binary_name)
        shutil.copytree(ROOT / "stdlib", package / "lib/jett/stdlib")
        archive_name = "jett_native_launcher.lib" if windows else "libjett_native_launcher.a"
        for profile in ("debug", "release"):
            runtime = package / "lib/jett/runtime" / host / profile
            runtime.mkdir(parents=True)
            archive = runtime / archive_name
            shutil.copy2(target / profile / archive_name, archive)
            run("cargo", "run", "--locked", "--quiet", "-p", "jett_driver", "--example",
                "native_bundle_manifest", "--target", host, "--", str(archive), profile, env=env)
        (package / "tools").mkdir()
        shutil.copy2(ROOT / "tools/smoke_native_package.py", package / "tools/smoke_native_package.py")
        copy_notices(package, metadata, env)
        (package / "README.txt").write_text(
            f"Jett native compiler for {host}\n\n"
            "Run bin/jett build source.jett, optionally with --release or -o output.\n"
            "Build requires the host C linker and SDK; the compiler never invokes Cargo.\n"
            "Run bin/jett build --check source.jett for frontend validation only.\n"
            "Keep bin and lib together when moving this package.\n"
            "Runtime manifests declare the required target, profile, CRT, ABI, and system libraries.\n"
            "Licenses and dependency declarations are collected under licenses/.\n",
            encoding="utf-8")
        package.rename(output)
    print(f"Packaged native compiler: {output}")


if __name__ == "__main__":
    main()
