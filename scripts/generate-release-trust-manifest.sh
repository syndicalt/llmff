#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/generate-release-trust-manifest.sh --assets-dir <dir> --version <version> --out <path>

Generates a machine-readable release trust manifest for staged GitHub Release
assets. The manifest records checksum-only release posture, unsigned installer
status, and SHA-256 digests for each staged asset. It does not claim signed
provenance, notarization, or a complete SBOM.
USAGE
}

assets_dir=""
version=""
out=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --assets-dir)
      assets_dir="${2:-}"
      shift 2
      ;;
    --version)
      version="${2:-}"
      shift 2
      ;;
    --out)
      out="${2:-}"
      shift 2
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
done

if [ -z "$assets_dir" ] || [ -z "$version" ] || [ -z "$out" ]; then
  usage >&2
  exit 2
fi

python3 - "$assets_dir" "$version" "$out" <<'PY'
import hashlib
import json
import os
import pathlib
import sys
from datetime import datetime, timezone


assets_dir = pathlib.Path(sys.argv[1])
version = sys.argv[2]
out = pathlib.Path(sys.argv[3])

if not assets_dir.is_dir():
    raise SystemExit(f"assets directory does not exist: {assets_dir}")

out_name = out.name
files = [
    path
    for path in sorted(assets_dir.iterdir(), key=lambda path: path.name)
    if path.is_file() and path.name not in {".SRCINFO", out_name}
]

if not files:
    raise SystemExit(f"no release assets found in {assets_dir}")


def sha256(path: pathlib.Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def sidecar_digest(path: pathlib.Path):
    sidecar = path.with_name(f"{path.name}.sha256")
    if not sidecar.is_file():
        return None
    first = sidecar.read_text(encoding="utf-8").split()[0]
    return first


assets = []
for path in files:
    digest = sha256(path)
    recorded = sidecar_digest(path)
    if recorded is not None and recorded != digest:
        raise SystemExit(
            f"checksum sidecar mismatch for {path.name}: sidecar={recorded} actual={digest}"
        )
    assets.append(
        {
            "name": path.name,
            "size_bytes": path.stat().st_size,
            "sha256": digest,
            "checksum_sidecar": f"{path.name}.sha256"
            if recorded is not None
            else None,
        }
    )

source_epoch = os.environ.get("SOURCE_DATE_EPOCH")
if source_epoch:
    generated_at = datetime.fromtimestamp(int(source_epoch), tz=timezone.utc)
else:
    generated_at = datetime.now(tz=timezone.utc)

manifest = {
    "format_version": 1,
    "name": "llmff release trust manifest",
    "version": version,
    "release_tag": f"v{version}",
    "generated_at": generated_at.replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "repository": os.environ.get("GITHUB_REPOSITORY", "syndicalt/llmff"),
    "git_ref": os.environ.get("GITHUB_REF_NAME"),
    "git_sha": os.environ.get("GITHUB_SHA"),
    "trust_posture": {
        "release_assets": "github-release",
        "verification": "sha256-checksum-only",
        "sbom": "not-generated",
        "provenance": "not-attested",
        "windows_authenticode": "not-signed",
        "macos_developer_id": "not-signed",
        "macos_notarization": "not-notarized",
    },
    "assets": assets,
}

out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY
