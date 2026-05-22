#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/package-windows-msi.sh --binary <path> --version <semver> --target <triple> --out-dir <dir> [--emit-wxs-only]

Creates a Windows MSI installer for llmff with WiX and writes a SHA-256
checksum file next to it. Use --emit-wxs-only to validate the WiX source
without requiring WiX on non-Windows development machines.
USAGE
}

binary=""
version=""
target=""
out_dir=""
emit_wxs_only=false

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
    --emit-wxs-only)
      emit_wxs_only=true
      shift
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

case "$version" in
  *[!0-9.]* | "" | *..* | .* | *.)
    printf 'error: MSI version must contain only dot-separated numeric components: %s\n' "$version" >&2
    exit 1
    ;;
esac

case "$target" in
  x86_64-pc-windows-msvc)
    wix_arch="x64"
    ;;
  *)
    printf 'error: unsupported Windows MSI target: %s\n' "$target" >&2
    exit 1
    ;;
esac

if [ ! -f "$binary" ]; then
  printf 'error: binary not found: %s\n' "$binary" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
template="$repo_root/packaging/windows/llmff.wxs"

if [ ! -f "$template" ]; then
  printf 'error: WiX source template not found: %s\n' "$template" >&2
  exit 1
fi

native_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    printf '%s\n' "$1"
  fi
}

mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd -P)"
binary="$(cd "$(dirname "$binary")" && pwd -P)/$(basename "$binary")"

package="llmff-${version}-${target}"
wxs="$out_dir/${package}.wxs"
msi="$out_dir/${package}.msi"
checksum="$msi.sha256"

cp "$template" "$wxs"

if [ "$emit_wxs_only" = true ]; then
  printf '%s\n' "$wxs"
  exit 0
fi

if ! command -v wix >/dev/null 2>&1; then
  printf 'error: wix is required to build Windows MSI installers\n' >&2
  exit 1
fi

rm -f "$msi" "$checksum"
wix build \
  -arch "$wix_arch" \
  -d "Version=$version" \
  -d "Binary=$(native_path "$binary")" \
  -out "$(native_path "$msi")" \
  "$(native_path "$template")"

if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    sha256sum "$(basename "$msi")" >"$(basename "$checksum")"
  )
elif command -v shasum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    shasum -a 256 "$(basename "$msi")" >"$(basename "$checksum")"
  )
else
  printf 'error: sha256sum or shasum is required\n' >&2
  exit 1
fi

printf '%s\n' "$msi"
printf '%s\n' "$checksum"
