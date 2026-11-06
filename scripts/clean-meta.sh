#!/usr/bin/env bash
# Strips metadata from files before they are committed or released.
# Usage: clean-meta.sh --staged | <files...>
set -euo pipefail

staged=0
files=()
if [[ "${1:-}" == "--staged" ]]; then
  staged=1
  while IFS= read -r -d '' f; do files+=("$f"); done \
    < <(git diff --cached --name-only --diff-filter=ACMR -z)
else
  files=("$@")
fi
[[ ${#files[@]} -eq 0 ]] && exit 0

command -v exiftool >/dev/null || { echo "clean-meta: exiftool is required" >&2; exit 1; }

email="$(git config user.email || true)"
leaks=()
for f in "${files[@]}"; do
  [[ -f "$f" ]] || continue
  if [[ "$(uname)" == "Darwin" ]]; then xattr -c "$f" 2>/dev/null || true; fi
  lower="$(printf '%s' "$f" | tr '[:upper:]' '[:lower:]')"
  case "$lower" in
    *.jpg|*.jpeg|*.png|*.gif|*.webp|*.tif|*.tiff|*.heic|*.avif|*.mp4|*.mov|*.pdf)
      exiftool -q -q -all= -overwrite_original "$f"
      [[ $staged -eq 1 ]] && git add -- "$f"
      ;;
  esac
  # Text files must not contain local paths or the git email.
  if grep -Iq . "$f" 2>/dev/null; then
    if grep -Fq "$HOME" "$f" || { [[ -n "$email" ]] && grep -Fqi "$email" "$f"; }; then
      leaks+=("$f")
    fi
  fi
done

if [[ ${#leaks[@]} -gt 0 ]]; then
  echo "clean-meta: personal data found in:" >&2
  printf '  %s\n' "${leaks[@]}" >&2
  exit 1
fi
