#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/package-archive.sh --binary <path> --version <semver> --target <triple> --out-dir <dir>

Creates a release archive containing the llmff binary, README, license, and
release notes, then writes a SHA-256 checksum file next to the archive.
USAGE
}

binary=""
version=""
target=""
out_dir=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --binary)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      binary="$2"
      shift 2
      ;;
    --version)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      version="$2"
      shift 2
      ;;
    --target)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      target="$2"
      shift 2
      ;;
    --out-dir)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      out_dir="$2"
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

if [ -z "$binary" ] || [ -z "$version" ] || [ -z "$target" ] || [ -z "$out_dir" ]; then
  usage >&2
  exit 2
fi

if [ ! -f "$binary" ]; then
  printf 'error: binary not found: %s\n' "$binary" >&2
  exit 1
fi

if [ ! -x "$binary" ]; then
  printf 'error: binary is not executable: %s\n' "$binary" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd -P)"

native_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s\n' "$1"
  fi
}

package="llmff-${version}-${target}"
stage="$(mktemp -d)"
cleanup() {
  rm -rf "$stage"
}
trap cleanup EXIT

payload="$stage/$package"
mkdir -p "$payload"

binary_name="llmff"
archive_ext="tar.gz"
case "$target" in
  *windows*)
    binary_name="llmff.exe"
    archive_ext="zip"
    ;;
esac

cp "$binary" "$payload/$binary_name"
cp "$repo_root/README.md" "$payload/README.md"

if [ -f "$repo_root/LICENSE" ]; then
  cp "$repo_root/LICENSE" "$payload/LICENSE"
fi

release_notes="$repo_root/docs/release-notes/v${version}.md"
if [ -f "$release_notes" ]; then
  mkdir -p "$payload/docs/release-notes"
  cp "$release_notes" "$payload/docs/release-notes/v${version}.md"
fi

archive="$out_dir/${package}.${archive_ext}"
checksum="$archive.sha256"
rm -f "$archive" "$checksum"

if [ "$archive_ext" = "zip" ]; then
  if command -v zip >/dev/null 2>&1; then
    (
      cd "$stage"
      zip -q -r "$archive" "$package"
    )
  elif command -v powershell.exe >/dev/null 2>&1; then
    (
      cd "$stage"
      powershell.exe -NoProfile -Command \
        "Compress-Archive -LiteralPath '$(native_path "$stage/$package")' -DestinationPath '$(native_path "$archive")' -Force" >/dev/null
    )
  elif command -v 7z >/dev/null 2>&1; then
    (
      cd "$stage"
      7z a -tzip "$archive" "$package" >/dev/null
    )
  else
    printf 'error: zip, powershell.exe, or 7z is required to create Windows archives\n' >&2
    exit 1
  fi
else
  tar -C "$stage" -czf "$archive" "$package"
fi

if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    sha256sum "$(basename "$archive")" >"$(basename "$checksum")"
  )
elif command -v shasum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    shasum -a 256 "$(basename "$archive")" >"$(basename "$checksum")"
  )
else
  printf 'error: sha256sum or shasum is required\n' >&2
  exit 1
fi

printf '%s\n' "$archive"
printf '%s\n' "$checksum"
