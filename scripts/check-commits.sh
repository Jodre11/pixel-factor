#!/bin/sh
# Enforce the commit conventions in CONTRIBUTING.md.
#
#   scripts/check-commits.sh <base-ref>   checks every commit in <base-ref>..HEAD
#
# Rules:
#   - subject line present and at most 72 characters
#   - no tool or AI attribution lines in the message
set -eu

base=${1:?usage: check-commits.sh <base-ref>}
status=0

# Merge commits are skipped: on a pull request CI checks out a synthetic
# merge of the branch into its base, whose subject is not the author's.
for sha in $(git rev-list --no-merges "$base"..HEAD); do
    subject=$(git log -1 --format=%s "$sha")
    short=$(git rev-parse --short "$sha")

    if [ -z "$subject" ]; then
        echo "$short: empty subject line"
        status=1
    elif [ "${#subject}" -gt 72 ]; then
        echo "$short: subject is ${#subject} characters, the limit is 72"
        status=1
    fi

    if git log -1 --format=%B "$sha" | grep -qiE '^(co-authored-by:.*(claude|copilot|cursor|noreply@anthropic|\[bot\])|claude-session:|.*generated with \[?claude)'; then
        echo "$short: contains a tool or AI attribution line"
        status=1
    fi
done

if [ "$status" -eq 0 ]; then
    echo "commit-check: ok"
fi
exit "$status"
