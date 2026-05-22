#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage:
  scripts/package-arch.sh --version <semver> --archive-url <url> --archive-sha256 <sha256> --out-dir <dir>

Creates an AUR-ready PKGBUILD and .SRCINFO for the prebuilt Linux x86_64 llmff archive.
USAGE
}

version=""
archive_url=""
archive_sha256=""
out_dir=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      version="$2"
      shift 2
      ;;
    --archive-url)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      archive_url="$2"
      shift 2
      ;;
    --archive-sha256)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      archive_sha256="$2"
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

if [ -z "$version" ] || [ -z "$archive_url" ] || [ -z "$archive_sha256" ] || [ -z "$out_dir" ]; then
  usage >&2
  exit 2
fi

case "$version" in
  *[!0-9.]* | "" | *..* | .* | *.)
    printf 'error: version must be a dotted numeric semver without a leading v: %s\n' "$version" >&2
    exit 2
    ;;
esac

case "$archive_url" in
  https://*) ;;
  *)
    printf 'error: archive URL must use https: %s\n' "$archive_url" >&2
    exit 2
    ;;
esac

case "$archive_sha256" in
  *[!0123456789abcdefABCDEF]* | "")
    printf 'error: archive sha256 must be hexadecimal\n' >&2
    exit 2
    ;;
esac

if [ "${#archive_sha256}" -ne 64 ]; then
  printf 'error: archive sha256 must be 64 hex characters\n' >&2
  exit 2
fi

mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd -P)"

pkgbuild="$out_dir/PKGBUILD"
srcinfo="$out_dir/.SRCINFO"
package_dir="llmff-${version}-x86_64-unknown-linux-gnu"

cat >"$pkgbuild" <<PKGBUILD
# Maintainer: llmff maintainers <maintainers@llmff.dev>

pkgname=llmff-bin
pkgver=${version}
pkgrel=1
pkgdesc='FFmpeg-shaped command-line runner for LLM inference pipelines'
arch=('x86_64')
url='https://github.com/syndicalt/llmff'
license=('MIT')
depends=('glibc')
provides=('llmff')
conflicts=('llmff')
source=("\${pkgname}-\${pkgver}.tar.gz::${archive_url}")
sha256sums=('${archive_sha256}')

package() {
  install -Dm755 "${package_dir}/llmff" "\${pkgdir}/usr/bin/llmff"
  install -Dm644 "${package_dir}/README.md" "\${pkgdir}/usr/share/doc/llmff/README.md"

  if [[ -f "${package_dir}/LICENSE" ]]; then
    install -Dm644 "${package_dir}/LICENSE" "\${pkgdir}/usr/share/licenses/llmff/LICENSE"
  fi

  if [[ -f "${package_dir}/docs/release-notes/v${version}.md" ]]; then
    install -Dm644 "${package_dir}/docs/release-notes/v${version}.md" "\${pkgdir}/usr/share/doc/llmff/release-notes-v${version}.md"
  fi
}
PKGBUILD

cat >"$srcinfo" <<SRCINFO
pkgbase = llmff-bin
	pkgdesc = FFmpeg-shaped command-line runner for LLM inference pipelines
	pkgver = ${version}
	pkgrel = 1
	url = https://github.com/syndicalt/llmff
	arch = x86_64
	license = MIT
	depends = glibc
	provides = llmff
	conflicts = llmff
	source = llmff-bin-${version}.tar.gz::${archive_url}
	sha256sums = ${archive_sha256}

pkgname = llmff-bin
SRCINFO

printf '%s\n' "$pkgbuild"
printf '%s\n' "$srcinfo"
