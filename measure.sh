#!/bin/bash

set -e
set -x

# TODO(kaihowl) used for tests, will break when adding arguments
if [[ $# -gt 0 ]]; then
  measurements=$*
else
  measurements="mymeasurement $(date +%s) $RANDOM github_action=$GITHUB_ACTION github_run_id=$GITHUB_RUN_ID github_run_number=$GITHUB_RUN_NUMBER runner_arch=$RUNNER_ARCH runner_os=$RUNNER_OS runner_name=$RUNNER_NAME"
fi

git notes --ref refs/notes/perf append -m "$measurements"
while [[ counter -lt 3 ]] && ! git push origin refs/notes/perf; do
  git fetch origin "refs/notes/perf"
  git notes --ref refs/notes/perf merge -s cat_sort_uniq FETCH_HEAD
  counter=$((counter+1))
done
