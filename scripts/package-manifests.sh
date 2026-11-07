#!/usr/bin/env bash
# Fills the winget and Homebrew templates for a published release.
# Usage: package-manifests.sh <version>   (e.g. 0.2.0)
# Output: dist/winget/manifests/a/ayokiru/mieruka/<version>/ (for a winget-pkgs PR)
#         dist/homebrew/Formula/mieruka.rb                  (for the ayokiru/homebrew-tap repo)
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: package-manifests.sh <version>}"
base="https://github.com/ayokiru/mieruka/releases/download/v$version"
sums="$(curl -fsSL "$base/SHA256SUMS")"
sha() { awk -v f="$1" '$2 == f {print $1}' <<<"$sums"; }
win="$(sha mieruka-x86_64-pc-windows-msvc.zip)"
arm="$(sha mieruka-aarch64-apple-darwin.tar.gz)"
intel="$(sha mieruka-x86_64-apple-darwin.tar.gz)"
linux="$(sha mieruka-x86_64-unknown-linux-musl.tar.gz)"
for s in "$win" "$arm" "$intel" "$linux"; do
  [[ ${#s} -eq 64 ]] || { echo "package-manifests: missing checksum in v$version/SHA256SUMS" >&2; exit 1; }
done
date="$(curl -fsSL "https://api.github.com/repos/ayokiru/mieruka/releases/tags/v$version" \
  | python3 -c 'import sys,json; print(json.loads(sys.stdin.read(), strict=False)["published_at"][:10])')"

out="dist/winget/manifests/a/ayokiru/mieruka/$version"
mkdir -p "$out"
for f in packaging/winget/*.yaml; do
  sed -e "s/{{VERSION}}/$version/g" -e "s/{{SHA256}}/$(tr a-f A-F <<<"$win")/g" -e "s/{{DATE}}/$date/g" \
    "$f" > "$out/$(basename "$f")"
done

mkdir -p dist/homebrew/Formula
sed -e "s/{{VERSION}}/$version/g" -e "s/{{SHA256_MACOS_ARM}}/$arm/" \
  -e "s/{{SHA256_MACOS_INTEL}}/$intel/" -e "s/{{SHA256_LINUX}}/$linux/" \
  packaging/homebrew/mieruka.rb > dist/homebrew/Formula/mieruka.rb
echo "$out"
echo "dist/homebrew/Formula/mieruka.rb"
