#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/check-release-assets.sh <tag> [--repo <owner/repo>] [--download-dir <path>] [--skip-smoke]

Downloads the expected GitHub Release assets for a tag, verifies their
checksums, and smoke-tests artifacts that are runnable on the current host.
USAGE
}

repo="${GITHUB_REPOSITORY:-syndicalt/llmff}"
download_dir=""
skip_smoke=0
tag=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      repo="$2"
      shift 2
      ;;
    --download-dir)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      download_dir="$2"
      shift 2
      ;;
    --skip-smoke)
      skip_smoke=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*)
      usage >&2
      exit 2
      ;;
    *)
      if [ -n "$tag" ]; then
        usage >&2
        exit 2
      fi
      tag="$1"
      shift
      ;;
  esac
done

if [ -z "$tag" ]; then
  usage >&2
  exit 2
fi

case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *)
    printf 'error: tag must be a semver tag like v0.1.2: %s\n' "$tag" >&2
    exit 2
    ;;
esac

if ! command -v gh >/dev/null 2>&1; then
  printf 'error: gh is required to inspect and download GitHub Release assets\n' >&2
  exit 1
fi

version="${tag#v}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

if [ -z "$download_dir" ]; then
  download_dir="$(mktemp -d)"
  cleanup_download_dir=1
else
  cleanup_download_dir=0
fi

cleanup() {
  if [ "$cleanup_download_dir" -eq 1 ]; then
    rm -rf "$download_dir"
  fi
}
trap cleanup EXIT

mkdir -p "$download_dir"

expected_assets=(
  "llmff-${version}-x86_64-unknown-linux-gnu.tar.gz"
  "llmff-${version}-x86_64-unknown-linux-gnu.tar.gz.sha256"
  "llmff-${version}-aarch64-apple-darwin.tar.gz"
  "llmff-${version}-aarch64-apple-darwin.tar.gz.sha256"
  "llmff-${version}-x86_64-apple-darwin.tar.gz"
  "llmff-${version}-x86_64-apple-darwin.tar.gz.sha256"
  "llmff-${version}-x86_64-pc-windows-msvc.zip"
  "llmff-${version}-x86_64-pc-windows-msvc.zip.sha256"
  "llmff-${version}-x86_64-pc-windows-msvc.msi"
  "llmff-${version}-x86_64-pc-windows-msvc.msi.sha256"
  "llmff-${version}-aarch64-apple-darwin.pkg"
  "llmff-${version}-aarch64-apple-darwin.pkg.sha256"
  "llmff-${version}-x86_64-apple-darwin.pkg"
  "llmff-${version}-x86_64-apple-darwin.pkg.sha256"
  "llmff_${version}_amd64.deb"
  "llmff_${version}_amd64.deb.sha256"
  "PKGBUILD"
  "llmff-${version}-arch.SRCINFO"
)

asset_list="$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')"

for asset in "${expected_assets[@]}"; do
  if ! grep -Fxq "$asset" <<<"$asset_list"; then
    printf 'error: release %s is missing expected asset: %s\n' "$tag" "$asset" >&2
    exit 1
  fi
done

for asset in "${expected_assets[@]}"; do
  gh release download "$tag" --repo "$repo" --dir "$download_dir" --clobber --pattern "$asset" >/dev/null
done

checksum_tool=()
if command -v sha256sum >/dev/null 2>&1; then
  checksum_tool=(sha256sum -c)
elif command -v shasum >/dev/null 2>&1; then
  checksum_tool=(shasum -a 256 -c)
else
  printf 'error: sha256sum or shasum is required to verify release checksums\n' >&2
  exit 1
fi

while IFS= read -r checksum; do
  [ -n "$checksum" ] || continue
  (cd "$download_dir" && "${checksum_tool[@]}" "$(basename "$checksum")")
done < <(find "$download_dir" -maxdepth 1 -type f -name '*.sha256' | sort)

if [ "$skip_smoke" -eq 1 ]; then
  printf 'release asset verification succeeded for %s\n' "$tag"
  exit 0
fi

kernel="$(uname -s)"
machine="$(uname -m)"

case "${kernel}:${machine}" in
  Linux:x86_64)
    scripts/smoke-archive.sh --archive "$download_dir/llmff-${version}-x86_64-unknown-linux-gnu.tar.gz"
    if command -v dpkg-deb >/dev/null 2>&1; then
      scripts/smoke-deb.sh --deb "$download_dir/llmff_${version}_amd64.deb"
    fi
    ;;
  Darwin:arm64)
    scripts/smoke-archive.sh --archive "$download_dir/llmff-${version}-aarch64-apple-darwin.tar.gz"
    scripts/smoke-macos-pkg.sh --pkg "$download_dir/llmff-${version}-aarch64-apple-darwin.pkg"
    ;;
  Darwin:x86_64)
    scripts/smoke-archive.sh --archive "$download_dir/llmff-${version}-x86_64-apple-darwin.tar.gz"
    scripts/smoke-macos-pkg.sh --pkg "$download_dir/llmff-${version}-x86_64-apple-darwin.pkg"
    ;;
  MINGW*:* | MSYS*:* | CYGWIN*:*)
    scripts/smoke-archive.sh --archive "$download_dir/llmff-${version}-x86_64-pc-windows-msvc.zip"
    ;;
  *)
    printf 'warning: no host smoke path for %s/%s; checksum verification completed\n' "$kernel" "$machine" >&2
    ;;
esac

printf 'release asset verification succeeded for %s\n' "$tag"
