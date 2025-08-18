#!/bin/bash

# Script to run the slow bash tests in a loop until they fail

echo "Starting test loop - will run until failure..."
echo "=============================================="

iteration=1
max_iterations=100  # Safety limit to prevent infinite loops

while [ $iteration -le $max_iterations ]; do
    echo ""
    echo "=== Iteration $iteration ==="
    echo "Running slow bash tests..."
    echo "=========================="
    
    # Run the test using cargo test
    if cargo test run_slow_bash_tests_with_binary --lib --test bash_tests; then
        echo "✅ Test passed on iteration $iteration"
        echo "Continuing to next iteration..."
        ((iteration++))
    else
        echo ""
        echo "❌ Test FAILED on iteration $iteration"
        echo "====================================="
        echo "The test failed after $iteration iterations."
        exit 1
    fi
done

echo "Reached maximum iterations ($max_iterations) without failure."
echo "Consider increasing max_iterations if you want to continue testing."
