"""Generate scripts/diarization-runtime-manifest.json from the reviewed lockfile.

The app installs the diarization add-on by downloading exactly the wheels the
lockfile pins, straight from PyPI. It must never resolve dependencies or trust
a URL or digest served at run time, so this script turns each locked
``name==version --hash=sha256:...`` line into the concrete file the app will
fetch (filename, URL, size) and the manifest is compiled into the binary.

Run after regenerating the lockfile, review the diff, and commit both:

    python -I scripts/generate_diarization_manifest.py

Only metadata is read from PyPI; nothing is downloaded or executed.
"""

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
LOCKFILE = ROOT / "diarization-runtime-requirements.txt"
MANIFEST = ROOT / "diarization-runtime-manifest.json"

# Wheel tags the bundled runtime can load: embeddable CPython 3.11, x64.
COMPATIBLE_TAGS = ("cp311-cp311-win_amd64", "cp311-abi3-win_amd64", "abi3-win_amd64", "py3-none-any", "py2.py3-none-any")

# Locked as source archives only. Neither is required at run time by any
# package in the tree (docopt is only a `cli` extra of pyannote.metrics and
# pyannote.pipeline; nothing requires antlr4-python3-runtime), so the add-on
# leaves them out rather than building and hosting wheels for them.
SKIPPED_SOURCE_ONLY = {
    "antlr4-python3-runtime": "not required by any locked package",
    "docopt": "only the optional `cli` extra of pyannote.metrics / pyannote.pipeline",
}


def locked_lines():
    for line in LOCKFILE.read_text(encoding="utf-8").splitlines():
        match = re.match(r"^([A-Za-z0-9_.\-]+)==(\S+)\s+--hash=sha256:([0-9a-f]{64})$", line.strip())
        if match:
            yield match.groups()


def pypi_files(name, version):
    url = f"https://pypi.org/pypi/{name}/{version}/json"
    request = urllib.request.Request(url, headers={"User-Agent": "fung-manifest-generator"})
    with urllib.request.urlopen(request, timeout=60) as response:
        return json.load(response)["urls"]


def main() -> int:
    wheels = []
    skipped = []
    for name, version, digest in locked_lines():
        if name in SKIPPED_SOURCE_ONLY:
            skipped.append({"name": name, "version": version, "sha256": digest, "reason": SKIPPED_SOURCE_ONLY[name]})
            continue
        match = [f for f in pypi_files(name, version) if f["digests"]["sha256"] == digest]
        if not match:
            print(f"{name}=={version}: no PyPI file has the locked digest", file=sys.stderr)
            return 1
        file = match[0]
        if not file["filename"].endswith(".whl"):
            print(f"{name}=={version}: locked file is not a wheel ({file['filename']})", file=sys.stderr)
            return 1
        if not any(tag in file["filename"] for tag in COMPATIBLE_TAGS):
            print(f"{name}=={version}: {file['filename']} does not target cp311 win_amd64", file=sys.stderr)
            return 1
        wheels.append({
            "name": name,
            "version": version,
            "filename": file["filename"],
            "url": file["url"],
            "sha256": digest,
            "size": file["size"],
        })

    manifest = {
        "lockfile": LOCKFILE.name,
        "pythonTag": "cp311",
        "platformTag": "win_amd64",
        "wheels": sorted(wheels, key=lambda w: w["name"].lower()),
        "skipped": skipped,
    }
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8", newline="\n")
    total = sum(w["size"] for w in wheels)
    print(f"Wrote {MANIFEST.name}: {len(wheels)} wheels, {total / 1048576:.1f} MiB, {len(skipped)} skipped")
    return 0


if __name__ == "__main__":
    sys.exit(main())
