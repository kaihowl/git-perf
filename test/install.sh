#!/bin/bash

set -e
set -x

if [[ "$(uname -s)" = "Darwin" ]]; then
  brew install parallel
elif [[ "$(lsb_release -i)" == *"Ubuntu"* ]]; then
  apt-get install parallel
fi
