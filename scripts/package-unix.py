#!/usr/bin/env python3
"""Package already-built Linux or universal macOS release binaries."""

import argparse
import json
from pathlib import Path
import plistlib
import shutil
import subprocess
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("platform", choices=("linux", "macos"))
    parser.add_argument("--legal-directory", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"], cwd=root
    ))
    version = next(package["version"] for package in metadata["packages"] if package["name"] == "cogcalc")
    target = Path(metadata["target_directory"])
    dist = root / "dist"
    dist.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="cogcalc-package-") as temporary:
        package = Path(temporary) / "CogCalc"
        package.mkdir()
        for name in ("LICENSE", "README.md", "THIRD-PARTY-NOTICES.txt", "SOURCE.txt"):
            shutil.copy2(args.legal_directory / name, package / name)
        shutil.copytree(root / "docs", package / "docs")
        if args.platform == "linux":
            binary = package / "CogCalc"
            shutil.copy2(target / "release" / "cogcalc", binary)
            binary.chmod(0o755)
            output = dist / "CogCalc-linux-x64.tar.gz"
            with tarfile.open(output, "w:gz") as archive:
                archive.add(package, arcname="CogCalc")
        else:
            app = package / "CogCalc.app"
            executable = app / "Contents" / "MacOS" / "CogCalc"
            executable.parent.mkdir(parents=True)
            resources = app / "Contents" / "Resources"
            resources.mkdir()
            for name in ("LICENSE", "THIRD-PARTY-NOTICES.txt", "SOURCE.txt"):
                shutil.copy2(package / name, resources / name)
            subprocess.run([
                "lipo", "-create", str(target / "aarch64-apple-darwin" / "release" / "cogcalc"),
                str(target / "x86_64-apple-darwin" / "release" / "cogcalc"), "-output", str(executable),
            ], check=True)
            executable.chmod(0o755)
            with (app / "Contents" / "Info.plist").open("wb") as info:
                plistlib.dump({
                    "CFBundleName": "CogCalc",
                    "CFBundleDisplayName": "CogCalc",
                    "CFBundleIdentifier": "io.github.wishdom.cogcalc",
                    "CFBundleExecutable": "CogCalc",
                    "CFBundlePackageType": "APPL",
                    "CFBundleInfoDictionaryVersion": "6.0",
                    "CFBundleShortVersionString": version,
                    "CFBundleVersion": version,
                    "LSMinimumSystemVersion": "11.0",
                    "NSHighResolutionCapable": True,
                }, info)
            subprocess.run(["plutil", "-lint", str(app / "Contents" / "Info.plist")], check=True)
            subprocess.run(["lipo", "-verify_arch", "arm64", "x86_64", str(executable)], check=True)
            # An ad hoc signature supports local execution but is not Developer ID signing or notarization.
            subprocess.run(["codesign", "--force", "--sign", "-", "--timestamp=none", str(app)], check=True)
            subprocess.run(["codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)], check=True)
            output = dist / "CogCalc-macos-universal.zip"
            subprocess.run(["ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(package), str(output)], check=True)
    print(f"Package: {output}")


if __name__ == "__main__":
    main()
