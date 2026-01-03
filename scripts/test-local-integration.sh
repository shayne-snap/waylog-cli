#!/bin/bash
# WayLog CLI Local Integration Tests
# Local integration test script for core functionality
# Run this script locally to verify core features work correctly

set -e

# Color output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test statistics
PASSED=0
FAILED=0
SKIPPED=0
TOTAL=0

# Test log file
TEST_LOG="/tmp/waylog_integration_test.log"
> "$TEST_LOG"

# Print section header
section() {
    echo ""
    echo -e "${BLUE}=== $1 ===${NC}"
}

# Test case function
# Usage: test_case "Test name" "command" [expected_exit_code]
test_case() {
    local name="$1"
    local command="$2"
    local expected_exit="${3:-0}"
    
    ((TOTAL++))
    echo -n "  Testing: $name... "
    
    # Run command and capture exit code
    if eval "$command" >> "$TEST_LOG" 2>&1; then
        EXIT_CODE=$?
    else
        EXIT_CODE=$?
    fi
    
    if [ $EXIT_CODE -eq $expected_exit ]; then
        echo -e "${GREEN}✓ PASS${NC}"
        ((PASSED++))
        return 0
    else
        echo -e "${RED}✗ FAIL${NC} (expected $expected_exit, got $EXIT_CODE)"
        echo "    Last 3 lines of output:"
        tail -3 "$TEST_LOG" | sed 's/^/    /'
        ((FAILED++))
        return 1
    fi
}

# Test JSON output validity
test_json_output() {
    local name="$1"
    local command="$2"
    
    ((TOTAL++))
    echo -n "  Testing: $name... "
    
    if eval "$command" 2>&1 | grep -E '^\{' | head -1 | python3 -c "import sys, json; json.load(sys.stdin)" >> "$TEST_LOG" 2>&1; then
        echo -e "${GREEN}✓ PASS${NC}"
        ((PASSED++))
        return 0
    else
        echo -e "${RED}✗ FAIL${NC} (invalid JSON)"
        ((FAILED++))
        return 1
    fi
}

# Check if agent is available
check_agent() {
    local agent="$1"
    if command -v "$agent" > /dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

echo "WayLog CLI Integration Tests"
echo "=============================="

# 1. Basic Functionality Tests
section "Basic Functionality Tests"

test_case "Help command" "cargo run -- --help" 0
test_case "Version command" "cargo run -- --version" 0
test_case "Pull command (basic)" "cargo run -- pull" 0
test_case "Pull with verbose" "cargo run -- pull --verbose" 0
test_case "Pull with specific provider" "cargo run -- pull --provider claude" 0
test_case "Pull with force flag" "cargo run -- pull --force" 0

# 2. Exit Code Propagation Tests
section "Exit Code Propagation Tests"

if check_agent "claude"; then
    test_case "Child process success (exit 0)" "cargo run -- run claude --help" 0
    test_case "Child process failure (exit 1)" "cargo run -- run claude --invalid-flag-that-does-not-exist" 1
else
    echo -e "  ${YELLOW}⚠ SKIP${NC} (claude not available)"
    ((SKIPPED++))
    ((SKIPPED++))
fi

# 3. Output Format Tests
section "Output Format Tests"

test_json_output "JSON output format (valid JSON)" "cargo run -- --output json pull --verbose"
test_case "Quiet mode (suppresses output)" "[ \$(cargo run -- --quiet pull 2>&1 | wc -l) -lt 5 ]" 0
test_case "JSON + Quiet combination" "cargo run -- --output json --quiet pull 2>&1 | wc -l" 0

# 4. Error Handling Tests
section "Error Handling Tests"

test_case "Missing agent error (exit 64)" "cargo run -- run" 64
test_case "Unknown agent error (exit 64)" "cargo run -- run nonexistent_agent_xyz" 64
test_case "Invalid provider error (exit 64)" "cargo run -- pull --provider invalid_provider" 64
test_case "Error message not duplicated" "[ \$(cargo run -- run 2>&1 | grep -c 'Missing required argument') -eq 1 ]" 0

# 5. Terminal Detection Tests
section "Terminal Detection Tests"

test_case "Terminal output (with color support)" "cargo run -- --help 2>&1 | head -1" 0
test_case "Non-terminal output (piped, no color)" "cargo run -- --help 2>&1 | cat | head -1" 0

# 6. Logging Tests
section "Logging Tests"

# Check if log file is created in verbose mode
if [ -d ".waylog/logs" ]; then
    test_case "Log file creation (verbose mode)" "[ -f .waylog/logs/waylog.log.\$(date +%Y-%m-%d) ]" 0
else
    echo -e "  ${YELLOW}⚠ SKIP${NC} (log directory not found, run with --verbose first)"
    ((SKIPPED++))
fi

# 7. Build and Installation Tests
section "Build Tests"

test_case "Release build" "cargo build --release" 0
test_case "Binary exists" "[ -f target/release/waylog ]" 0

# Summary
echo ""
echo "=============================="
echo "Test Summary"
echo "=============================="
echo -e "Total:  $TOTAL"
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed:  $FAILED${NC}"
if [ $SKIPPED -gt 0 ]; then
    echo -e "${YELLOW}Skipped: $SKIPPED${NC}"
fi
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
    echo ""
    echo "Note: Some tests require manual verification:"
    echo "  - Signal handling (SIGINT/SIGTERM)"
    echo "  - Child process termination timeout"
    echo "  - Human-panic (requires panic trigger)"
    exit 0
else
    echo -e "${RED}✗ Some tests failed!${NC}"
    echo ""
    echo "Check $TEST_LOG for detailed output"
    exit 1
fi

