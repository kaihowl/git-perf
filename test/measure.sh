#!/bin/bash
set -e

if [[ $# -eq 0 ]]; then
  echo Two few arguments
  exit 1
fi

measurements=$*

git notes --ref refs/notes/perf-v2 append -m "${measurements[@]}"
