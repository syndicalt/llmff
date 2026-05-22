#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/package-deb.sh --binary <path> --version <semver> --arch <deb-arch> --out-dir <dir>

Creates a Debian package for llmff and writes a SHA-256 checksum file next to it.

Supported arch values are Debian architecture names such as amd64, arm64, and armhf.
USAGE
}

binary=""
version=""
arch=""
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
    --arch)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      arch="$2"
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

if [ -z "$binary" ] || [ -z "$version" ] || [ -z "$arch" ] || [ -z "$out_dir" ]; then
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

case "$arch" in
  amd64 | arm64 | armhf) ;;
  *)
    printf 'error: unsupported Debian architecture: %s\n' "$arch" >&2
    exit 2
    ;;
esac

if ! command -v dpkg-deb >/dev/null 2>&1; then
  printf 'error: dpkg-deb is required to build .deb packages\n' >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd -P)"

stage="$(mktemp -d)"
cleanup() {
  rm -rf "$stage"
}
trap cleanup EXIT

package_root="$stage/llmff"
mkdir -p \
  "$package_root/DEBIAN" \
  "$package_root/usr/bin" \
  "$package_root/usr/share/doc/llmff"

install -m 0755 "$binary" "$package_root/usr/bin/llmff"
install -m 0644 "$repo_root/README.md" "$package_root/usr/share/doc/llmff/README.md"

release_notes="$repo_root/docs/release-notes/v${version}.md"
if [ -f "$release_notes" ]; then
  install -m 0644 "$release_notes" "$package_root/usr/share/doc/llmff/release-notes-v${version}.md"
fi

if [ -f "$repo_root/LICENSE" ]; then
  install -m 0644 "$repo_root/LICENSE" "$package_root/usr/share/doc/llmff/copyright"
else
  cat >"$package_root/usr/share/doc/llmff/copyright" <<'COPYRIGHT'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: llmff
Source: https://github.com/syndicalt/llmff
License: MIT
COPYRIGHT
fi

installed_size="$(du -sk "$package_root/usr" | awk '{print $1}')"
cat >"$package_root/DEBIAN/control" <<CONTROL
Package: llmff
Version: ${version}
Section: utils
Priority: optional
Architecture: ${arch}
Maintainer: llmff maintainers <maintainers@llmff.dev>
Installed-Size: ${installed_size}
Depends: libc6
Homepage: https://github.com/syndicalt/llmff
Description: FFmpeg-shaped command-line runner for LLM inference pipelines
 llmff composes typed LLM inference pipelines from manifests and inline
 graph expressions. It supports deterministic local stages, backend adapters,
 dry-run inspection, JSONL traces, and CLI-first execution.
CONTROL

deb="$out_dir/llmff_${version}_${arch}.deb"
checksum="$deb.sha256"
rm -f "$deb" "$checksum"

dpkg_deb_args=(--root-owner-group --build "$package_root" "$deb")
if command -v fakeroot >/dev/null 2>&1; then
  fakeroot dpkg-deb "${dpkg_deb_args[@]}"
else
  dpkg-deb "${dpkg_deb_args[@]}"
fi

if command -v sha256sum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    sha256sum "$(basename "$deb")" >"$(basename "$checksum")"
  )
elif command -v shasum >/dev/null 2>&1; then
  (
    cd "$out_dir"
    shasum -a 256 "$(basename "$deb")" >"$(basename "$checksum")"
  )
else
  printf 'error: sha256sum or shasum is required\n' >&2
  exit 1
fi

printf '%s\n' "$deb"
printf '%s\n' "$checksum"
