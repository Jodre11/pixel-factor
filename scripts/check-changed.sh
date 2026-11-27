#!/bin/sh
# Run only the checks that the current change needs.
#
#   scripts/check-changed.sh            changes since the merge base with origin/main,
#                                       plus anything uncommitted
#   scripts/check-changed.sh <ref>      changes since <ref>, plus anything uncommitted
#   scripts/check-changed.sh --staged   only what is staged (used by the pre-commit hook)
#
# Rules:
#   - workspace-wide files changed  -> the full `make check`
#   - files inside a crate changed  -> format, lint and test that crate only
#   - markdown changed              -> the docs check
#   - anything else                 -> nothing to prove
set -eu

cd "$(git rev-parse --show-toplevel)"

if [ "${1:-}" = "--staged" ]; then
    files=$(git diff --cached --name-only --diff-filter=ACMR)
else
    base=${1:-}
    if [ -z "$base" ]; then
        base=$(git merge-base HEAD origin/main 2>/dev/null || git rev-parse HEAD)
    fi
    files=$({
        git diff --name-only --diff-filter=ACMR "$base"
        git ls-files --others --exclude-standard
    } | sort -u)
fi

if [ -z "$files" ]; then
    echo "check-changed: nothing changed"
    exit 0
fi

full=0
docs=0
crates=""
for f in $files; do
    case "$f" in
        Cargo.toml | Cargo.lock | rust-toolchain.toml | Makefile | scripts/* | .github/workflows/* | .githooks/*)
            full=1
            ;;
        *.md)
            docs=1
            ;;
    esac
    top=${f%%/*}
    if [ "$top" != "$f" ] && [ -f "$top/Cargo.toml" ]; then
        case " $crates " in
            *" $top "*) ;;
            *) crates="$crates $top" ;;
        esac
    fi
done

if [ "$full" -eq 1 ]; then
    echo "check-changed: workspace-wide files changed, running the full check"
    exec make check
fi

for dir in $crates; do
    name=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$dir/Cargo.toml" | head -n 1)
    echo "check-changed: checking crate $name ($dir/)"
    cargo fmt -p "$name" --check
    cargo clippy -p "$name" --all-targets --locked -- -D warnings
    cargo test -p "$name" --locked --offline
done

if [ "$docs" -eq 1 ]; then
    echo "check-changed: markdown changed, checking docs"
    scripts/check-docs.sh
fi

if [ -z "$crates" ] && [ "$docs" -eq 0 ]; then
    echo "check-changed: no code or docs changed, nothing to prove"
fi
