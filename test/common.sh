#!/bin/bash

set -e
PS4='${LINENO}: '
set -x

shopt -s nocasematch

script_dir=$(pwd)/$(dirname "$0")

export PYTHONOPTIMIZE=TRUE

export GIT_COMMITTER_NAME="github-actions[bot]"
export GIT_COMMITTER_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"
export GIT_AUTHOR_NAME="github-actions[bot]"
export GIT_AUTHOR_EMAIL="41898282+github-actions[bot]@users.noreply.github.com"

PATH=${script_dir}/..:$PATH
export PATH

function cd_empty_repo() {
  local tmpgit
  tmpgit="$(mktemp -d)"
  pushd "${tmpgit}"
  git init
  # Only setting the values with envvars fails for libgit2 git_signature_default
  git config user.name "$GIT_COMMITTER_NAME"
  git config user.email "$GIT_COMMITTER_EMAIL"
}

function create_commit() {
  echo "a" >> a
  git add a
  git commit -m 'my commit'
}

function cd_temp_repo() {
  cd_empty_repo
  create_commit
  create_commit
  create_commit
  create_commit
}
