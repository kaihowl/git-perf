#!/bin/bash

set -e
set -x

script_dir=$(dirname "$0")

git fetch origin "refs/notes/perf:refs/notes/perf"
git --no-pager log --no-color --pretty='-- %H%n%N' --notes=refs/notes/perf | python3 "${script_dir}/report.py"
