#!/bin/bash

set -e
set -x


git --no-pager log --no-color --pretty='-- %H%n%N' --notes=refs/notes/perf | python3 report.py
