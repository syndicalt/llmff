#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/package-macos-pkg.sh --binary <path> --version <semver> --target <triple> --out-dir <dir> [--emit-payload-only]

Creates an unsigned macOS Installer .pkg for llmff and writes a SHA-256 checksum
file next to it. Use --emit-payload-only to validate the staged package payload
without requiring macOS pkgbuild on non-Darwin development machines.
USAGE
}

binary=""
version=""
target=""
out_dir=""
emit_payload_only=false

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
    --emit-payload-only)
      emit_payload_only=true
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
    printf 'error: macOS package version must contain only dot-separated numeric components: %s\n' "$version" >&2
    exit 1
    ;;
esac

case "$target" in
  aarch64-apple-darwin | x86_64-apple-darwin)
    ;;
  *)
    printf 'error: unsupported macOS package target: %s\n' "$target" >&2
    exit 1
    ;;
esac

if [ ! -f "$binary" ]; then
  printf 'error: binary not found: %s\n' "$binary" >&2
  exit 1
fi

if [ ! -x "$binary" ]; then
  printf 'error: binary is not executable: %s\n' "$binary" >&2
  exit 1
fi

mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd -P)"
binary="$(cd "$(dirname "$binary")" && pwd -P)/$(basename "$binary")"

package="llmff-${version}-${target}"
identifier="dev.syndicalt.llmff"
stage="$out_dir/${package}.pkgroot"
pkg="$out_dir/${package}.pkg"
checksum="$pkg.sha256"

rm -rf "$stage"
mkdir -p "$stage/usr/local/bin"
install -m 0755 "$binary" "$stage/usr/local/bin/llmff"

if [ "$emit_payload_only" = true ]; then
  printf '%s\n' "$stage"
  exit 0
fi

case "$(uname -s)" in
  Darwin)
    ;;
  *)
    printf 'error: macOS .pkg builds require a Darwin host; use --emit-payload-only for local payload validation\n' >&2
    exit 1
    ;;
esac

if ! command -v pkgbuild >/dev/null 2>&1; then
  printf 'error: pkgbuild is required to build macOS .pkg installers\n' >&2
  exit 1
fi

rm -f "$pkg" "$checksum"
pkgbuild \
  --root "$stage" \
  --identifier "$identifier" \
  --version "$version" \
  --install-location / \
  "$pkg"

if command -v shasum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    shasum -a 256 "$(basename "$pkg")" >"$(basename "$checksum")"
  )
elif command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    sha256sum "$(basename "$pkg")" >"$(basename "$checksum")"
  )
else
  printf 'error: shasum or sha256sum is required\n' >&2
  exit 1
fi

printf '%s\n' "$pkg"
printf '%s\n' "$checksum"
