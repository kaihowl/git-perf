#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" = "Darwin" ]]; then
  brew install parallel || true
elif [[ "$(lsb_release -i)" == *"Ubuntu"* ]]; then
  sudo apt-get install parallel
fi
