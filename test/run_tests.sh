#!/bin/bash


script_dir=$(dirname "$0")

num_procs=4
find "${script_dir}" -name 'test_*.sh' -print0 | parallel -n1 -P${num_procs} --tag -0 bash {}
