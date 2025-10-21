#!/bin/bash

set -e
PS4='${BASH_SOURCE}:${LINENO}: '
set -x

export RUST_BACKTRACE=1

shopt -s nocasematch

# Hermetic git environment - ignore system and global git config
# This prevents issues with commit signing and other global git settings
export GIT_CONFIG_NOSYSTEM=true
export GIT_CONFIG_GLOBAL=/dev/null

# Set git author and committer info for tests
export GIT_COMMITTER_NAME="github-actions[bot]"
export GIT_COMMITTER_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"
export GIT_AUTHOR_NAME="github-actions[bot]"
export GIT_AUTHOR_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"

function cd_empty_repo() {
  local tmpgit
  tmpgit="$(mktemp -d)"
  pushd "${tmpgit}"
  git init --initial-branch=master
}

function create_commit() {
  # Since some of the commits are added in the same instant with the same content, they result in the same hash.
  # Instead, use random files such that there is a very small chance in collision.
  local file
  file=$RANDOM
  # As the RANDOM function can collide, ensure that with each call of create_commit, the file content changes
  # by appending to the (often but not always) new file.
  # Without this, the git commit might end up as 'empty'.
  echo content >> "$file"
  git add "$file"
  git commit -m 'my commit'
}

function cd_temp_repo() {
  cd_empty_repo
  create_commit
  create_commit
  create_commit
  create_commit
}

function assert_output_contains() {
  local output="$1"
  local expected="$2"
  local error_message="${3:-Missing expected string in output}"

  if [[ $output != *"$expected"* ]]; then
    echo "$error_message:"
    echo "$output"
    exit 1
  fi
}

function assert_output_not_contains() {
  local output="$1"
  local unexpected="$2"
  local error_message="${3:-Unexpected string found in output}"

  if [[ $output == *"$unexpected"* ]]; then
    echo "$error_message:"
    echo "$output"
    exit 1
  fi
}
