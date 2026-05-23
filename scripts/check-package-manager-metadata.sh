#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

version="0.1.2"
tag="v${version}"
base_url="https://github.com/syndicalt/llmff/releases/download/${tag}"

linux_archive="llmff-${version}-x86_64-unknown-linux-gnu.tar.gz"
macos_arm_archive="llmff-${version}-aarch64-apple-darwin.tar.gz"
macos_intel_archive="llmff-${version}-x86_64-apple-darwin.tar.gz"
windows_zip="llmff-${version}-x86_64-pc-windows-msvc.zip"
windows_msi="llmff-${version}-x86_64-pc-windows-msvc.msi"

linux_sha256="48e290e689af48300af7ca9e3a53e2813a35d1b20c2228feae9c67d7412a1067"
macos_arm_sha256="7b15b3d8510aaebe88c5d273c2b8f92e33e80d0b9a1e5a87492d1488116445ee"
macos_intel_sha256="89780bd77f30584b06dfaf8b8179070fb87bdc497b4b3c1ca4f427eeb7dfe7ca"
windows_zip_sha256="440d428ff34e7e119aaf8f785d980a7b9bd1e6809ee6118c4395873f5adc50ba"
windows_msi_sha256="d034981db3d6c134cbbe5c98e17cb855c428e025952f0ce7861d1c3ebd02e6e9"

require_file() {
  local path="$1"
  if [ ! -f "$path" ]; then
    printf 'error: missing package-manager metadata: %s\n' "$path" >&2
    exit 1
  fi
}

require_text() {
  local path="$1"
  local text="$2"
  require_file "$path"
  if ! grep -Fq -- "$text" "$path"; then
    printf 'error: %s must contain: %s\n' "$path" "$text" >&2
    exit 1
  fi
}

require_absent_path() {
  local path="$1"
  if [ -e "$path" ]; then
    printf 'error: apt repository metadata is parked and must not be shipped: %s\n' "$path" >&2
    exit 1
  fi
}

homebrew="packaging/homebrew/llmff.rb"
winget_version="packaging/winget/Syndicalt.Llmff.yaml"
winget_locale="packaging/winget/Syndicalt.Llmff.locale.en-US.yaml"
winget_installer="packaging/winget/Syndicalt.Llmff.installer.yaml"
scoop="packaging/scoop/llmff.json"
aur_pkgbuild="packaging/aur/PKGBUILD"
aur_srcinfo="packaging/aur/.SRCINFO"

require_text "$homebrew" "version \"${version}\""
require_text "$homebrew" "${base_url}/${macos_arm_archive}"
require_text "$homebrew" "${macos_arm_sha256}"
require_text "$homebrew" "${base_url}/${macos_intel_archive}"
require_text "$homebrew" "${macos_intel_sha256}"
require_text "$homebrew" "${base_url}/${linux_archive}"
require_text "$homebrew" "${linux_sha256}"
require_text "$homebrew" "assert_match \"llmff ${version}\", shell_output(\"#{bin}/llmff --version\")"

require_text "$winget_version" "PackageIdentifier: Syndicalt.Llmff"
require_text "$winget_version" "PackageVersion: ${version}"
require_text "$winget_locale" "PackageName: llmff"
require_text "$winget_installer" "InstallerType: wix"
require_text "$winget_installer" "InstallerUrl: ${base_url}/${windows_msi}"
require_text "$winget_installer" "InstallerSha256: ${windows_msi_sha256}"

require_text "$scoop" "\"version\": \"${version}\""
require_text "$scoop" "\"url\": \"${base_url}/${windows_zip}\""
require_text "$scoop" "\"hash\": \"${windows_zip_sha256}\""
require_text "$scoop" "\"bin\": \"llmff.exe\""

require_text "$aur_pkgbuild" "pkgname=llmff-bin"
require_text "$aur_pkgbuild" "pkgver=${version}"
require_text "$aur_pkgbuild" "${base_url}/${linux_archive}"
require_text "$aur_pkgbuild" "sha256sums=('${linux_sha256}')"
require_text "$aur_srcinfo" "pkgbase = llmff-bin"
require_text "$aur_srcinfo" "pkgver = ${version}"
require_text "$aur_srcinfo" "source = llmff-bin-${version}.tar.gz::${base_url}/${linux_archive}"
require_text "$aur_srcinfo" "sha256sums = ${linux_sha256}"

require_absent_path "packaging/apt/Release"
require_absent_path "packaging/apt/InRelease"
require_absent_path "packaging/apt/Packages"
require_absent_path "packaging/apt/Sources"
require_absent_path "packaging/apt/Release.gpg"

printf 'package-manager metadata validation succeeded for %s\n' "$tag"
