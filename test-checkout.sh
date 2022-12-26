#!/bin/bash

set -euxo pipefail


cd "$(mktemp -d)"
git init .
git remote add origin https://github.com/kaihowl/dotfiles

# git config --local --name-only --get-regexp core\.sshCommand
# git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'core\.sshCommand' && git config --local --unset-all 'core.sshCommand' || :"
# git config --local --name-only --get-regexp http\.https\:\/\/github\.com\/\.extraheader
# git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'http\.https\:\/\/github\.com\/\.extraheader' && git config --local --unset-all 'http.https://github.com/.extraheader' || :"

AUTH=$(echo -n ":$GITHUB_PAT" | openssl base64 | tr -d '\n')
git config --local http.https://github.com/.extraheader "AUTHORIZATION: basic $AUTH"

git -c protocol.version=2 fetch --no-tags --prune --progress --no-recurse-submodules --depth=40 origin +a40339946a12d4922d16a002ee8e2d3ae2702c5f:refs/remotes/pull/513/merge
