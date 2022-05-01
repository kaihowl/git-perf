#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" = "Darwin" ]]; then
  # This fails when run as part of tox pre_command on GitHub Actions.
  # It only fails after the installation of `parallel` is done.
  # Therefore, ignoring the failure.
  brew install -v parallel || true
elif [[ "$(lsb_release -i)" == *"Ubuntu"* ]]; then
  sudo apt-get install parallel
fi
