#!/usr/bin/env bash
# Installs a pre-commit hook that runs clean-meta.sh on staged files.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
cat > .git/hooks/pre-commit <<'HOOK'
#!/usr/bin/env bash
exec scripts/clean-meta.sh --staged
HOOK
chmod +x .git/hooks/pre-commit
echo "pre-commit hook installed"
