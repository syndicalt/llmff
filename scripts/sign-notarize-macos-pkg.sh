#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/sign-notarize-macos-pkg.sh \
    --pkg <path> \
    --identity <developer-id-installer> \
    --certificate-base64 <p12-base64> \
    --certificate-password <password> \
    --apple-id <apple-id> \
    --team-id <team-id> \
    --apple-password <app-specific-password>

Signs a macOS Installer .pkg with productsign, verifies the package signature,
submits the signed package to Apple notarization, staples the notarization
ticket, validates the staple, and replaces the original package atomically.
USAGE
}

pkg=""
identity=""
certificate_base64=""
certificate_password=""
apple_id=""
team_id=""
apple_password=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --pkg)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      pkg="$2"
      shift 2
      ;;
    --identity)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      identity="$2"
      shift 2
      ;;
    --certificate-base64)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      certificate_base64="$2"
      shift 2
      ;;
    --certificate-password)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      certificate_password="$2"
      shift 2
      ;;
    --apple-id)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      apple_id="$2"
      shift 2
      ;;
    --team-id)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      team_id="$2"
      shift 2
      ;;
    --apple-password)
      if [ "$#" -lt 2 ] || [ -z "$2" ]; then
        usage >&2
        exit 2
      fi
      apple_password="$2"
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

if [ -z "$pkg" ] ||
  [ -z "$identity" ] ||
  [ -z "$certificate_base64" ] ||
  [ -z "$certificate_password" ] ||
  [ -z "$apple_id" ] ||
  [ -z "$team_id" ] ||
  [ -z "$apple_password" ]; then
  usage >&2
  exit 2
fi

if [ ! -f "$pkg" ]; then
  printf 'error: macOS package not found: %s\n' "$pkg" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin) ;;
  *)
    printf 'error: macOS package signing requires a Darwin host\n' >&2
    exit 1
    ;;
esac

for tool in security productsign pkgutil xcrun base64 openssl; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'error: required macOS signing tool not found: %s\n' "$tool" >&2
    exit 1
  fi
done

pkg="$(cd "$(dirname "$pkg")" && pwd -P)/$(basename "$pkg")"
pkg_dir="$(dirname "$pkg")"
pkg_base="$(basename "$pkg")"
signed_pkg="${pkg_dir}/.${pkg_base}.signed"
remove_tmp_dir=false
tmp_dir="${RUNNER_TEMP:-}"
if [ -z "$tmp_dir" ]; then
  tmp_dir="$(mktemp -d)"
  remove_tmp_dir=true
fi
certificate_path="${tmp_dir}/llmff-macos-installer.p12"
keychain_password="$(openssl rand -hex 24)"
keychain_path="${tmp_dir}/llmff-signing.keychain-db"

cleanup() {
  rm -f "$certificate_path" "$signed_pkg"
  if [ -f "$keychain_path" ]; then
    security delete-keychain "$keychain_path" >/dev/null 2>&1 || true
  fi
  if [ "$remove_tmp_dir" = true ]; then
    rm -rf "$tmp_dir"
  fi
}
trap cleanup EXIT

printf '%s' "$certificate_base64" | base64 --decode >"$certificate_path"

security create-keychain -p "$keychain_password" "$keychain_path"
security set-keychain-settings -lut 21600 "$keychain_path"
security unlock-keychain -p "$keychain_password" "$keychain_path"
security import "$certificate_path" \
  -k "$keychain_path" \
  -P "$certificate_password" \
  -T /usr/bin/productsign
security set-key-partition-list -S apple-tool:,apple: -s -k "$keychain_password" "$keychain_path"

productsign \
  --keychain "$keychain_path" \
  --sign "$identity" \
  "$pkg" \
  "$signed_pkg"

pkgutil --check-signature "$signed_pkg"

xcrun notarytool submit "$signed_pkg" \
  --apple-id "$apple_id" \
  --team-id "$team_id" \
  --password "$apple_password" \
  --wait

xcrun stapler staple "$signed_pkg"
xcrun stapler validate "$signed_pkg"

mv "$signed_pkg" "$pkg"
pkgutil --check-signature "$pkg"
printf '%s\n' "$pkg"
