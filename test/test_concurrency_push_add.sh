#!/bin/bash
set -euo pipefail

# Constants
NUM_PUSH_ITERATIONS=98
NUM_ADD_ITERATIONS=499
CONCURRENT_ADDERS=2

script_dir=$(unset CDPATH; cd "$(dirname "$0")" > /dev/null; pwd -P)
# shellcheck source=test/common.sh
source "$script_dir/common.sh"

# Disable set -x from the common.sh inclusion
set +x

cd "$(mktemp -d)"
root=$(pwd)

mkdir upstream
pushd upstream
git init --bare
popd

git clone "$root/upstream" work
pushd work
git commit --allow-empty -m 'test commit'
git push

# Function to handle cleanup when script is interrupted
# shellcheck disable=SC2317
cleanup() {
    echo ""
    echo "Received interrupt signal. Terminating all jobs..."
    # Kill all jobs in the current process group
    kill "$(jobs -p)" 2>/dev/null || true
    echo "Cleanup complete. Exiting."
    exit 1
}

# Set up trap for CTRL+C and other termination signals
trap cleanup SIGINT SIGTERM

# Function to run git-perf push command (without measurement parameter)
run_push_test() {
    local log_prefix=$1
    shift  # Remove first parameter (prefix) from the argument list
    local iterations=$NUM_PUSH_ITERATIONS

    local cmd_display="git-perf $*"
    echo "Starting $iterations iterations of '$cmd_display'..."

    for (( i=1; i<=iterations; i++ )); do
        # Run the push command without additional parameters
        git-perf "$@" 2>&1 | perl -pe "s/^/[$log_prefix] /" || (echo "Failed to push" && exit 1)

        # Every 10 iterations, print a status update
        if (( i % 10 == 0 )); then
            echo "[$log_prefix] Completed $i/$iterations iterations"
        fi
    done

    echo "[$log_prefix] All $iterations iterations completed"
    return 0
}

# Function to run git-perf add command (with measurement parameter)
run_add_test() {
    local log_prefix=$1
    shift  # Remove first parameter (prefix) from the argument list
    local iterations=$NUM_ADD_ITERATIONS

    local cmd_display="git-perf $* --measurement test-$log_prefix <random>"
    echo "Starting $iterations iterations of '$cmd_display'..."

    for (( i=1; i<=iterations; i++ )); do
        # Generate a random integer
        local random_value=$((RANDOM + RANDOM * i + $(date +%s) % 10000))

        # Run the add command with measurement parameter
        # Adding separate measurements for each adder as the random values could overlap
        git-perf "$@" --measurement "test-$log_prefix" $random_value 2>&1 | perl -pe "s/^/[$log_prefix] /" || (echo "Failed to add" && exit 1)

        # Every 100 iterations, print a status update
        if (( i % 100 == 0 )); then
            echo "[$log_prefix] Completed $i/$iterations iterations"
        fi
    done

    echo "[$log_prefix] All $iterations iterations completed"
    return 0
}

# Run the tests in parallel
echo "Starting test harness..."
echo "Press CTRL+C at any time to abort the test"

# Start the push process (without measurement parameter)
run_push_test "PUSH" push &
PUSH_PID=$!

ADD_PIDS=()
# Start the add process (with measurement parameter)
for i in $(seq 1 $CONCURRENT_ADDERS); do
run_add_test "ADD_$i" add &
ADD_PIDS+=($!)
done

# Wait for both processes to complete
echo "Waiting for all tests to complete..."
wait $PUSH_PID
PUSH_STATUS=$?

wait "${ADD_PIDS[@]}"
ADD_STATUS=$?

# Reset trap for normal completion
trap - SIGINT SIGTERM

# Check if both processes completed successfully
if [ $PUSH_STATUS -ne 0 ] || [ $ADD_STATUS -ne 0 ]; then
    echo "ERROR: One or more tests failed!"
    [ $PUSH_STATUS -ne 0 ] && echo "  'push' test failed with exit code $PUSH_STATUS"
    [ $ADD_STATUS -ne 0 ] && echo "  'add' test failed with exit code $ADD_STATUS"
    exit 1
fi

# Verify the results
echo "Verifying results..."
LINE_COUNT=$(git-perf report -o - | wc -l)
EXPECTED_COUNT=$((NUM_ADD_ITERATIONS * CONCURRENT_ADDERS))

if [[ $LINE_COUNT -eq $EXPECTED_COUNT ]]; then
    echo "SUCCESS: Verification passed. Found exactly $EXPECTED_COUNT lines in the report."
    exit 0
else
    echo "ERROR: Verification failed. Expected $EXPECTED_COUNT lines but found $LINE_COUNT."
    exit 1
fi
