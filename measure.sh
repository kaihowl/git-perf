#!/bin/bash

set -e
set -x

git notes --ref refs/notes/perf append -m "mymeasurement $(date +%s) $RANDOM os=$1"
while [[ counter -lt 3 ]] && ! git push origin refs/notes/perf; do
  git fetch origin "refs/notes/perf"
  git notes --ref refs/notes/perf merge -s cat_sort_uniq FETCH_HEAD
  counter=$((counter+1))
done
