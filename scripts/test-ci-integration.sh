#!/bin/bash
# WayLog CLI CI Integration Tests
# Tests that don't require local environment dependencies

set -e

# Test statistics
PASSED=0
FAILED=0
TOTAL=0

# Test case function
test_case() {
    local name="$1"
    local command="$2"
    local expected_exit="${3:-0}"
    
    ((TOTAL++))
    echo ""
    echo "Testing: $name"
    echo "  Command: $command"
    
    # Capture both stdout and stderr for debugging
    local temp_output=$(mktemp)
    local temp_error=$(mktemp)
    
    # Temporarily disable set -e to capture exit code even if command fails
    set +e
    eval "$command" > "$temp_output" 2> "$temp_error"
    EXIT_CODE=$?
    set -e
    
    echo "  Exit code: $EXIT_CODE (expected: $expected_exit)"
    
    # Always output stderr if present
    if [ -s "$temp_error" ]; then
        echo "  Stderr:"
        sed 's/^/    /' < "$temp_error"
    else
        echo "  Stderr: (empty)"
    fi
    
    # Always output stdout if present
    if [ -s "$temp_output" ]; then
        echo "  Stdout:"
        sed 's/^/    /' < "$temp_output"
    else
        echo "  Stdout: (empty)"
    fi
    
    if [ $EXIT_CODE -eq $expected_exit ]; then
        echo "  Result: ✓ PASS"
        ((PASSED++))
    else
        echo "  Result: ✗ FAIL"
        ((FAILED++))
    fi
    
    rm -f "$temp_output" "$temp_error"
    return 0
}

# Test JSON output validity
test_json_output() {
    local name="$1"
    local command="$2"
    
    ((TOTAL++))
    echo ""
    echo "Testing: $name"
    echo "  Command: $command"
    
    local temp_output=$(mktemp)
    local temp_error=$(mktemp)
    
    # Temporarily disable set -e to capture exit code even if command fails
    set +e
    eval "$command" > "$temp_output" 2> "$temp_error"
    EXIT_CODE=$?
    
    # Try to validate JSON
    if grep -E '^\{' < "$temp_output" | head -1 | python3 -c "import sys, json; json.load(sys.stdin)" > /dev/null 2>&1; then
        RESULT=0
        VALIDATION="Valid JSON"
    else
        RESULT=1
        VALIDATION="Invalid JSON"
    fi
    set -e
    
    echo "  Exit code: $EXIT_CODE"
    echo "  JSON validation: $VALIDATION"
    
    # Always output stderr if present
    if [ -s "$temp_error" ]; then
        echo "  Stderr:"
        sed 's/^/    /' < "$temp_error"
    else
        echo "  Stderr: (empty)"
    fi
    
    # Always output stdout if present
    if [ -s "$temp_output" ]; then
        echo "  Stdout:"
        sed 's/^/    /' < "$temp_output"
    else
        echo "  Stdout: (empty)"
    fi
    
    if [ $RESULT -eq 0 ]; then
        echo "  Result: ✓ PASS"
        ((PASSED++))
    else
        echo "  Result: ✗ FAIL"
        ((FAILED++))
    fi
    
    rm -f "$temp_output" "$temp_error"
    return 0
}

echo "WayLog CLI CI Integration Tests"
echo "================================"

# 1. Basic Commands
echo ""
echo "=== Basic Commands ==="
test_case "Help command" "cargo run -- --help" 0
test_case "Version command" "cargo run -- --version" 0

# 2. Error Handling
echo ""
echo "=== Error Handling ==="
test_case "Missing agent error (exit 64)" "cargo run -- run" 64
test_case "Unknown agent error (exit 64)" "cargo run -- run nonexistent_agent_xyz" 64
test_case "Invalid provider error (exit 64)" "cargo run -- pull --provider invalid_provider" 64
test_case "Error message not duplicated" "[ \$(cargo run -- run 2>&1 | grep -c 'Missing required argument') -eq 1 ]" 0

# 3. Output Format
echo ""
echo "=== Output Format ==="
test_json_output "JSON output format (error case)" "cargo run -- --output json run 2>&1"
test_case "Quiet mode (error still shown)" "[ \$(cargo run -- --quiet run 2>&1 | wc -l) -ge 1 ]" 0
test_case "JSON + Quiet combination" "cargo run -- --output json --quiet run 2>&1 | wc -l" 0

# 4. Terminal Detection
echo ""
echo "=== Terminal Detection ==="
test_case "Terminal output" "cargo run -- --help 2>&1 | head -1" 0
test_case "Non-terminal output (piped)" "cargo run -- --help 2>&1 | cat | head -1" 0

# 5. Build Tests
echo ""
echo "=== Build Tests ==="
test_case "Release build" "cargo build --release" 0
test_case "Binary exists" "[ -f target/release/waylog ]" 0

# Summary
echo ""
echo "================================"
echo "Test Summary"
echo "================================"
echo "Total:  $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
echo ""

if [ $FAILED -eq 0 ]; then
    echo "✓ All tests passed!"
    exit 0
else
    echo "✗ Some tests failed!"
    exit 1
fi

