#!/usr/bin/env bash
# Fails when a relative Markdown link in the given files points nowhere.
status=0
for f in "$@"; do
  base=$(dirname "$f")
  while read -r p; do
    [ -e "$base/$p" ] || [ -e "$p" ] || { echo "MISSING in $f: $p"; status=1; }
  done < <(grep -oE '\]\([^)#]+' "$f" | sed 's/](//' | grep -v '^http')
done
[ $status -eq 0 ] && echo "links ok: $*"
exit $status
