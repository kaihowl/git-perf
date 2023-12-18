#!/bin/bash

set -e

# Check if no argument is provided
if [ $# -eq 0 ]; then
    echo "Usage: $0 <directory>"
    exit 1
fi

directory=$1

# Check if the directory exists
if [ ! -d "$directory" ]; then
    echo "Directory '$directory' not found."
    exit 1
fi

# Check if the directory is a Git repository
if [ ! -d "$directory/.git" ]; then
    echo "Directory '$directory' is not a Git repository."
    exit 1
fi

echo "Directory '$directory' is a Git repository."

tmpdir=$(mktemp -d)

pushd "$tmpdir"

trap 'rm -rf $tmpdir' EXIT

git clone "${directory}" test-repo

cd test-repo

git fetch origin refs/notes/perf:refs/notes/perf

git checkout refs/notes/perf

process_file() {
  perl -pe 's/^/0 /' -i "$1"
  git add "$1"
}

git ls-tree -r --name-only HEAD | while read -r file; do
    if [ ! -d "$file" ]; then
        process_file "$file"
    fi
done

git commit -m 'migrate to v2'

git push origin HEAD:refs/notes/perf-v2
