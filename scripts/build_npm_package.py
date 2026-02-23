#!/usr/bin/env python3
"""Stage and optionally pack npm packages for xurl."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
NPM_ROOT = REPO_ROOT / "npm"
MAIN_PACKAGE_NAME = "@xuanwo/xurl"

PLATFORM_PACKAGES: dict[str, dict[str, str]] = {
    "xurl-linux-x64": {
        "npm_name": "@xuanwo/xurl-linux-x64",
        "npm_tag": "linux-x64",
        "target_triple": "x86_64-unknown-linux-gnu",
        "os": "linux",
        "cpu": "x64",
    },
    "xurl-linux-arm64": {
        "npm_name": "@xuanwo/xurl-linux-arm64",
        "npm_tag": "linux-arm64",
        "target_triple": "aarch64-unknown-linux-gnu",
        "os": "linux",
        "cpu": "arm64",
    },
    "xurl-darwin-x64": {
        "npm_name": "@xuanwo/xurl-darwin-x64",
        "npm_tag": "darwin-x64",
        "target_triple": "x86_64-apple-darwin",
        "os": "darwin",
        "cpu": "x64",
    },
    "xurl-darwin-arm64": {
        "npm_name": "@xuanwo/xurl-darwin-arm64",
        "npm_tag": "darwin-arm64",
        "target_triple": "aarch64-apple-darwin",
        "os": "darwin",
        "cpu": "arm64",
    },
    "xurl-win32-x64": {
        "npm_name": "@xuanwo/xurl-win32-x64",
        "npm_tag": "win32-x64",
        "target_triple": "x86_64-pc-windows-msvc",
        "os": "win32",
        "cpu": "x64",
    },
    "xurl-win32-arm64": {
        "npm_name": "@xuanwo/xurl-win32-arm64",
        "npm_tag": "win32-arm64",
        "target_triple": "aarch64-pc-windows-msvc",
        "os": "win32",
        "cpu": "arm64",
    },
}

PACKAGE_EXPANSIONS: dict[str, list[str]] = {
    "xurl": ["xurl", *PLATFORM_PACKAGES],
}

PACKAGE_NATIVE_COMPONENTS: dict[str, list[str]] = {
    "xurl": [],
    "xurl-linux-x64": ["xurl"],
    "xurl-linux-arm64": ["xurl"],
    "xurl-darwin-x64": ["xurl"],
    "xurl-darwin-arm64": ["xurl"],
    "xurl-win32-x64": ["xurl"],
    "xurl-win32-arm64": ["xurl"],
}

COMPONENT_DEST_DIR: dict[str, str] = {
    "xurl": "xurl",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build or stage xurl npm packages.")
    parser.add_argument(
        "--package",
        choices=tuple(PACKAGE_NATIVE_COMPONENTS),
        default="xurl",
        help="Package name to stage.",
    )
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version number to write to package.json.",
    )
    parser.add_argument(
        "--staging-dir",
        type=Path,
        help="Directory to stage package contents. Must be empty when provided.",
    )
    parser.add_argument(
        "--pack-output",
        type=Path,
        help="Path where npm pack output tarball should be written.",
    )
    parser.add_argument(
        "--vendor-src",
        type=Path,
        help="Vendor source directory containing target-triple trees.",
    )
    return parser.parse_args()


def prepare_staging_dir(staging_dir: Path | None) -> tuple[Path, bool]:
    if staging_dir is not None:
        staging_dir = staging_dir.resolve()
        staging_dir.mkdir(parents=True, exist_ok=True)
        if any(staging_dir.iterdir()):
            raise RuntimeError(f"Staging directory {staging_dir} is not empty.")
        return staging_dir, False

    temp_dir = Path(tempfile.mkdtemp(prefix="xurl-npm-stage-"))
    return temp_dir, True


def compute_platform_package_version(version: str, platform_tag: str) -> str:
    # npm does not allow reusing the same package name/version for different payloads.
    return f"{version}-{platform_tag}"


def stage_sources(staging_dir: Path, version: str, package: str) -> None:
    package_json: dict
    if package == "xurl":
        bin_dir = staging_dir / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(NPM_ROOT / "bin" / "xurl.js", bin_dir / "xurl.js")
        shutil.copy2(REPO_ROOT / "README.md", staging_dir / "README.md")

        with open(NPM_ROOT / "package.json", "r", encoding="utf-8") as fh:
            package_json = json.load(fh)

        package_json["version"] = version
        package_json["files"] = ["bin"]
        package_json["optionalDependencies"] = {
            PLATFORM_PACKAGES[platform_package]["npm_name"]: (
                f"npm:{MAIN_PACKAGE_NAME}@"
                f"{compute_platform_package_version(version, PLATFORM_PACKAGES[platform_package]['npm_tag'])}"
            )
            for platform_package in PACKAGE_EXPANSIONS["xurl"]
            if platform_package != "xurl"
        }
    elif package in PLATFORM_PACKAGES:
        platform_package = PLATFORM_PACKAGES[package]
        platform_version = compute_platform_package_version(version, platform_package["npm_tag"])

        with open(NPM_ROOT / "package.json", "r", encoding="utf-8") as fh:
            main_package = json.load(fh)

        package_json = {
            "name": MAIN_PACKAGE_NAME,
            "version": platform_version,
            "license": main_package.get("license", "Apache-2.0"),
            "os": [platform_package["os"]],
            "cpu": [platform_package["cpu"]],
            "files": ["vendor"],
            "repository": main_package.get("repository"),
        }

        engines = main_package.get("engines")
        if isinstance(engines, dict):
            package_json["engines"] = engines

        shutil.copy2(REPO_ROOT / "README.md", staging_dir / "README.md")
    else:
        raise RuntimeError(f"Unknown package '{package}'.")

    with open(staging_dir / "package.json", "w", encoding="utf-8") as out:
        json.dump(package_json, out, indent=2)
        out.write("\n")


def copy_native_binaries(
    vendor_src: Path,
    staging_dir: Path,
    components: list[str],
    target_filter: set[str] | None = None,
) -> None:
    vendor_src = vendor_src.resolve()
    if not vendor_src.exists():
        raise RuntimeError(f"Vendor source directory not found: {vendor_src}")

    components_set = {component for component in components if component in COMPONENT_DEST_DIR}
    if not components_set:
        return

    vendor_dest = staging_dir / "vendor"
    if vendor_dest.exists():
        shutil.rmtree(vendor_dest)
    vendor_dest.mkdir(parents=True, exist_ok=True)

    copied_targets: set[str] = set()
    for target_dir in vendor_src.iterdir():
        if not target_dir.is_dir():
            continue
        if target_filter is not None and target_dir.name not in target_filter:
            continue

        dest_target_dir = vendor_dest / target_dir.name
        dest_target_dir.mkdir(parents=True, exist_ok=True)
        copied_targets.add(target_dir.name)

        for component in components_set:
            dest_dir_name = COMPONENT_DEST_DIR[component]
            src_component_dir = target_dir / dest_dir_name
            if not src_component_dir.exists():
                raise RuntimeError(
                    f"Missing native component '{component}' in vendor source: {src_component_dir}"
                )

            dest_component_dir = dest_target_dir / dest_dir_name
            if dest_component_dir.exists():
                shutil.rmtree(dest_component_dir)
            shutil.copytree(src_component_dir, dest_component_dir)

    if target_filter is not None:
        missing_targets = sorted(target_filter - copied_targets)
        if missing_targets:
            missing_text = ", ".join(missing_targets)
            raise RuntimeError(f"Missing target directories in vendor source: {missing_text}")


def run_npm_pack(staging_dir: Path, output_path: Path) -> Path:
    output_path = output_path.resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="xurl-npm-pack-") as pack_dir_str:
        pack_dir = Path(pack_dir_str)
        stdout = subprocess.check_output(
            ["npm", "pack", "--json", "--pack-destination", str(pack_dir)],
            cwd=staging_dir,
            text=True,
        )
        pack_output = json.loads(stdout)
        if not pack_output:
            raise RuntimeError("npm pack did not produce an output tarball.")

        tarball_name = pack_output[0].get("filename") or pack_output[0].get("name")
        if not tarball_name:
            raise RuntimeError("Unable to determine npm pack output filename.")

        tarball_path = pack_dir / tarball_name
        if not tarball_path.exists():
            raise RuntimeError(f"Expected npm pack output not found: {tarball_path}")

        shutil.move(str(tarball_path), output_path)
    return output_path


def main() -> int:
    args = parse_args()
    staging_dir, created_temp = prepare_staging_dir(args.staging_dir)

    package = args.package
    version = args.release_version
    target_filter: set[str] | None = None

    try:
        stage_sources(staging_dir, version, package)

        native_components = PACKAGE_NATIVE_COMPONENTS.get(package, [])
        if native_components:
            if args.vendor_src is None:
                components_text = ", ".join(native_components)
                raise RuntimeError(
                    "Native components "
                    f"({components_text}) required for package '{package}'. Provide --vendor-src."
                )

            target_filter = (
                {PLATFORM_PACKAGES[package]["target_triple"]}
                if package in PLATFORM_PACKAGES
                else None
            )
            copy_native_binaries(
                args.vendor_src,
                staging_dir,
                native_components,
                target_filter=target_filter,
            )

        if args.pack_output is not None:
            output_path = run_npm_pack(staging_dir, args.pack_output)
            print(f"npm pack output written to {output_path}")
        else:
            print(f"Staged package in {staging_dir}")
    finally:
        if created_temp:
            # Keep temp staging for inspection when run manually.
            pass

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
