#!/usr/bin/env bash
set -euo pipefail

limit="${SOURCE_FILE_LINE_LIMIT:-1000}"
status=0

while IFS= read -r file; do
  lines="$(wc -l < "$file" | tr -d ' ')"
  if (( lines > limit )); then
    printf '%7d  %s\n' "$lines" "$file"
    status=1
  fi
done < <(
  rg --files \
    -g '!target/**' \
    -g '!node_modules/**' \
    -g '!dist/**' \
    -g '!coverage/**' \
    -g '*.rs' \
    -g '*.ts' \
    -g '*.tsx' \
    -g '*.js' \
    -g '*.jsx' \
    -g '*.css' \
    | sort
)

if (( status != 0 )); then
  echo "source files above ${limit} lines must be split by responsibility" >&2
  exit "$status"
fi

echo "all hand-written source files are at or below ${limit} lines"
