#!/bin/bash

export TEST_TRACE=0

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

test_section "[environment] single var resolves to measurement metadata"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
os = "GITPERF_IT_OS"
EOF
export GITPERF_IT_OS=linux
git perf add -m bench 100
# Audit with the selector passes only if the key-value was stored automatically
assert_success git perf audit -m bench -s os=linux
unset GITPERF_IT_OS

test_section "[defaults] static value appears as metadata"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[defaults]
environment = "local"
EOF
git perf add -m bench 100
assert_success git perf audit -m bench -s environment=local

test_section "CLI --key-value overrides [environment]"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
foo = "GITPERF_IT_FOO"
EOF
export GITPERF_IT_FOO=from_env
git perf add -m bench 100 -k foo=from_cli
# Selector matching env value must fail (cli wins)
assert_failure git perf audit -m bench -s foo=from_env
# Selector matching cli value must succeed
assert_success git perf audit -m bench -s foo=from_cli
unset GITPERF_IT_FOO

test_section "[environment] overrides [defaults]"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
foo = "GITPERF_IT_ENV_OVER"

[defaults]
foo = "from_defaults"
EOF
export GITPERF_IT_ENV_OVER=from_env
git perf add -m bench 100
assert_success git perf audit -m bench -s foo=from_env
assert_failure git perf audit -m bench -s foo=from_defaults
unset GITPERF_IT_ENV_OVER

test_section "--skip-env bypasses [environment], falls back to [defaults]"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
foo = "GITPERF_IT_SKIP"

[defaults]
foo = "from_defaults"
EOF
export GITPERF_IT_SKIP=from_env
git perf add -m bench 100 --skip-env
assert_success git perf audit -m bench -s foo=from_defaults
assert_failure git perf audit -m bench -s foo=from_env
unset GITPERF_IT_SKIP

test_section "Multi-source array: first found wins"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
runner_id = ["GITPERF_IT_R1", "GITPERF_IT_R2"]
EOF
unset GITPERF_IT_R1
export GITPERF_IT_R2=runner2
git perf add -m bench 100
assert_success git perf audit -m bench -s runner_id=runner2
unset GITPERF_IT_R2

test_section "Multi-source array: first set var wins over second"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
runner_id = ["GITPERF_IT_FIRST", "GITPERF_IT_SECOND"]
EOF
export GITPERF_IT_FIRST=runner1
export GITPERF_IT_SECOND=runner2
git perf add -m bench 100
assert_success git perf audit -m bench -s runner_id=runner1
assert_failure git perf audit -m bench -s runner_id=runner2
unset GITPERF_IT_FIRST
unset GITPERF_IT_SECOND

test_section "Sensitive variable names are blocked"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
tok = "MY_GITPERF_TOKEN"
sec = "GITPERF_SECRET_KEY"
EOF
export MY_GITPERF_TOKEN=supersecret
export GITPERF_SECRET_KEY=hunter2
git perf add -m bench 100
# Audit with the blocked values must fail (values not stored)
assert_failure git perf audit -m bench -s tok=supersecret
assert_failure git perf audit -m bench -s sec=hunter2
unset MY_GITPERF_TOKEN
unset GITPERF_SECRET_KEY

test_section "import command respects [environment]"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
ci = "GITPERF_IT_CI"
EOF
export GITPERF_IT_CI=true
# Create a minimal JUnit XML file
cat > junit.xml << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<testsuites>
  <testsuite name="suite" tests="1" time="0.5">
    <testcase name="mytest" time="0.5"/>
  </testsuite>
</testsuites>
EOF
git perf import junit junit.xml
assert_success git perf audit -m "test::mytest" -s ci=true
unset GITPERF_IT_CI

test_section "measure command respects [environment]"
cd_temp_repo
cat > .gitperfconfig << 'EOF'
[environment]
env_tag = "GITPERF_IT_TAG"
EOF
export GITPERF_IT_TAG=tagged
git perf measure -m bench -- true
assert_success git perf audit -m bench -s env_tag=tagged
unset GITPERF_IT_TAG

test_section "No [environment] or [defaults] means no extra metadata"
cd_temp_repo
git perf add -m bench 100
# Auditing with any arbitrary selector must fail (nothing was auto-added)
assert_failure git perf audit -m bench -s phantom=value

test_stats
exit 0
