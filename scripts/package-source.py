#!/usr/bin/env python3
"""Package the tagged source, locked dependencies, and dependency notices."""

import argparse
import gzip
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import tomllib


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repository", default="wishdom/CogCalc")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    if args.tag != "v" + manifest["package"]["version"]:
        parser.error("tag must match the Cargo package version")
    dist = root / "dist"
    legal = dist / "legal"
    legal.mkdir(parents=True, exist_ok=True)
    commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    epoch = int(subprocess.check_output(["git", "show", "-s", "--format=%ct", "HEAD"], cwd=root))
    source_url = f"https://github.com/{args.repository}/releases/download/{args.tag}/CogCalc-source.tar.gz"
    (legal / "SOURCE.txt").write_text(
        f"CogCalc {args.tag}\nSource commit: {commit}\nLicense: GPL-3.0-only\n"
        f"Matching source, build scripts, and vendored dependencies: {source_url}\n"
        "See LICENSE and THIRD-PARTY-NOTICES.txt for license terms.\n",
        encoding="utf-8",
    )
    for name in ("LICENSE", "README.md"):
        shutil.copy2(root / name, legal / name)

    with tempfile.TemporaryDirectory(prefix="cogcalc-source-") as temporary:
        staging = Path(temporary)
        source = staging / "CogCalc-source"
        source.mkdir()
        archive = staging / "repository.tar"
        run("git", "archive", "--format=tar", "--output", str(archive), "HEAD", cwd=root)
        with tarfile.open(archive) as repository:
            repository.extractall(source, filter="data")
        (source / ".cargo").mkdir(exist_ok=True)
        with (source / ".cargo" / "config.toml").open("w", encoding="utf-8") as config:
            run("cargo", "vendor", "--locked", "vendor", cwd=source, stdout=config)
        run("cargo", "metadata", "--locked", "--offline", "--format-version", "1",
            cwd=source, stdout=subprocess.DEVNULL)

        notices = [f"Third-party license notices for CogCalc {args.tag}\n",
                   "Includes locked build, test, and platform-specific dependency sources.\n",
                   "Dependency source and original license files are in CogCalc-source.tar.gz.\n"]
        for crate in sorted((source / "vendor").iterdir()):
            package = tomllib.loads((crate / "Cargo.toml").read_text(encoding="utf-8"))["package"]
            notices.append(f"\n{'=' * 72}\n{package['name']} {package['version']}\n"
                           f"Declared license: {package.get('license', 'see license files')}\n")
            candidates = {
                path for path in crate.rglob("*") if path.is_file()
                and (any(word in path.name.lower() for word in ("license", "licence", "copying", "copyright", "notice"))
                     or path.name.lower() in {"ofl.txt", "ufl.txt", "hack-regular.txt"})
                and path.suffix.lower() not in {".rs", ".py", ".sh", ".toml", ".json"}
            }
            if package.get("license-file"):
                candidates.add(crate / package["license-file"])
            for path in sorted(candidates):
                data = path.read_bytes()
                if b"\0" in data:
                    continue
                notices.append(f"\n--- {path.relative_to(crate).as_posix()} ---\n"
                               + data.decode("utf-8", errors="replace") + "\n")
        (legal / "THIRD-PARTY-NOTICES.txt").write_text("".join(notices), encoding="utf-8")
        shutil.copy2(legal / "THIRD-PARTY-NOTICES.txt", source)
        shutil.copy2(legal / "SOURCE.txt", source)
        (source / "BUILDING.txt").write_text(
            "Build with Rust 1.95.0 and the native platform compiler/SDK.\n"
            "The included .cargo/config.toml uses the bundled vendor directory.\n"
            "Run: cargo build --release --locked --offline\n"
            "Linux also needs the development packages listed in .github/workflows/release.yml.\n"
            "See README.md and scripts/ for packaging instructions.\n"
            "The vendor directory preserves each dependency's original license files.\n",
            encoding="utf-8",
        )
        # Stable ordering and metadata make this source archive repeatable.
        def normalize(info):
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            info.mtime = epoch
            return info

        output = dist / "CogCalc-source.tar.gz"
        with output.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=epoch) as compressed:
                with tarfile.open(fileobj=compressed, mode="w") as bundle:
                    bundle.add(source, arcname="CogCalc-source", filter=normalize)
    print(f"Source archive: {output}")


if __name__ == "__main__":
    main()
