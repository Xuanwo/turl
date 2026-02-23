#!/usr/bin/env python3
"""Stage xurl npm tarballs for release."""

from __future__ import annotations

import argparse
import importlib.util
import shutil
import subprocess
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
BUILD_SCRIPT = REPO_ROOT / "scripts" / "build_npm_package.py"

_SPEC = importlib.util.spec_from_file_location("xurl_build_npm_package", BUILD_SCRIPT)
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {BUILD_SCRIPT}")
_BUILD_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_BUILD_MODULE)

PACKAGE_NATIVE_COMPONENTS = getattr(_BUILD_MODULE, "PACKAGE_NATIVE_COMPONENTS", {})
PACKAGE_EXPANSIONS = getattr(_BUILD_MODULE, "PACKAGE_EXPANSIONS", {})
PLATFORM_PACKAGES = getattr(_BUILD_MODULE, "PLATFORM_PACKAGES", {})


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version to stage, for example 0.0.8.",
    )
    parser.add_argument(
        "--package",
        dest="packages",
        action="append",
        default=None,
        help="Package name to stage. May be provided multiple times. Defaults to xurl.",
    )
    parser.add_argument(
        "--vendor-src",
        type=Path,
        required=True,
        help="Vendor source directory that contains target-triple trees.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=REPO_ROOT / "dist" / "npm",
        help="Directory where npm tarballs should be written.",
    )
    parser.add_argument(
        "--keep-staging-dirs",
        action="store_true",
        help="Keep temporary staging directories for debugging.",
    )
    return parser.parse_args()


def expand_packages(packages: list[str]) -> list[str]:
    expanded: list[str] = []
    for package in packages:
        for expanded_package in PACKAGE_EXPANSIONS.get(package, [package]):
            if expanded_package in expanded:
                continue
            expanded.append(expanded_package)
    return expanded


def collect_native_components(packages: list[str]) -> set[str]:
    components: set[str] = set()
    for package in packages:
        components.update(PACKAGE_NATIVE_COMPONENTS.get(package, []))
    return components


def tarball_name_for_package(package: str, version: str) -> str:
    if package in PLATFORM_PACKAGES:
        platform = package.removeprefix("xurl-")
        return f"xurl-npm-{platform}-{version}.tgz"
    return f"xurl-npm-{version}.tgz"


def run_command(cmd: list[str]) -> None:
    print("+", " ".join(cmd))
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)


def main() -> int:
    args = parse_args()
    packages = args.packages or ["xurl"]
    expanded_packages = expand_packages(packages)
    native_components = collect_native_components(expanded_packages)

    if native_components and not args.vendor_src.exists():
        raise RuntimeError(f"Vendor source directory not found: {args.vendor_src}")

    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    staged_messages: list[str] = []
    for package in expanded_packages:
        staging_dir = Path(tempfile.mkdtemp(prefix=f"npm-stage-{package}-"))
        pack_output = output_dir / tarball_name_for_package(package, args.release_version)

        cmd = [
            str(BUILD_SCRIPT),
            "--package",
            package,
            "--release-version",
            args.release_version,
            "--staging-dir",
            str(staging_dir),
            "--pack-output",
            str(pack_output),
        ]

        if PACKAGE_NATIVE_COMPONENTS.get(package):
            cmd.extend(["--vendor-src", str(args.vendor_src.resolve())])

        try:
            run_command(cmd)
        finally:
            if not args.keep_staging_dirs:
                shutil.rmtree(staging_dir, ignore_errors=True)

        staged_messages.append(f"Staged {package} at {pack_output}")

    for message in staged_messages:
        print(message)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
