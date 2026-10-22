#!/usr/bin/env bash
set -euo pipefail
python3 "$(dirname "${BASH_SOURCE[0]}")/hardening.py"
python3 "$(dirname "${BASH_SOURCE[0]}")/installer.py"
