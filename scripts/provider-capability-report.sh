#!/usr/bin/env bash
set -euo pipefail

binary="${LLMFF_BIN:-llmff}"

exec "$binary" backends report "$@"
