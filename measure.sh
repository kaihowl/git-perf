#!/bin/bash

set -e
set -x

if [[ $# -eq 0 ]]; then
  echo Two few arguments
  exit 1
fi

measurements=$*

git notes --ref refs/notes/perf append -m "$measurements"
