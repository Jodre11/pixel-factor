#!/bin/sh
# Fail if the docs promise something the repo does not have.
#
# Checks every tracked or new markdown file for:
#   - `make <target>` in backticks, where <target> is not in the Makefile
#   - relative markdown links, [text](path), whose target file does not exist
set -eu

cd "$(git rev-parse --show-toplevel)"

status=0
files=$({ git ls-files '*.md'; git ls-files --others --exclude-standard '*.md'; } | sort -u)

for f in $files; do
    [ -f "$f" ] || continue

    for target in $(grep -o '`make [a-z-]*' "$f" | sed 's/^`make //' | sort -u); do
        [ -n "$target" ] || continue
        if ! grep -q "^$target:" Makefile; then
            echo "$f: mentions \`make $target\`, which is not a Makefile target"
            status=1
        fi
    done

    dir=$(dirname "$f")
    for link in $(grep -o '](\([^)# ]*\)' "$f" | sed 's/^](//' | sort -u); do
        case "$link" in
            "" | http://* | https://* | mailto:*) continue ;;
        esac
        if [ ! -e "$dir/$link" ]; then
            echo "$f: links to $link, which does not exist"
            status=1
        fi
    done
done

if [ "$status" -eq 0 ]; then
    echo "docs-check: ok"
fi
exit "$status"
