#!/usr/bin/env bash
# Comprehensive linting script for Claudius

set -e

echo "Running Claudius Linting Suite"
echo "=============================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track if any linting failed
LINT_FAILED=0

# Function to run a lint command and report status
run_lint() {
    local name=$1
    local command=$2

    echo -n "Running $name... "
    if eval "$command" > /tmp/lint_output_$$ 2>&1; then
        echo -e "${GREEN}✓ Passed${NC}"
    else
        echo -e "${RED}✗ Failed${NC}"
        echo -e "${YELLOW}Output:${NC}"
        cat /tmp/lint_output_$$
        echo ""
        LINT_FAILED=1
    fi
    rm -f /tmp/lint_output_$$
}

# Check if tools are installed
echo "Checking tools..."
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found${NC}"
    exit 1
fi

if ! command -v rustfmt &> /dev/null; then
    echo -e "${YELLOW}Warning: rustfmt not found, installing...${NC}"
    rustup component add rustfmt
fi

if ! cargo clippy --version &> /dev/null; then
    echo -e "${YELLOW}Warning: clippy not found, installing...${NC}"
    rustup component add clippy
fi

echo ""
echo "Running linters..."
echo "------------------"

# 1. Format check
run_lint "rustfmt (check)" "cargo fmt -- --check"

# 2. Clippy with all our custom lints
# Note: Using clippy with all custom lints for Rust 1.86.0
run_lint "clippy" "cargo clippy --all-targets --all-features -- -W clippy::all -W clippy::pedantic -A clippy::module-name-repetitions -A clippy::must-use-candidate -A clippy::missing-docs-in-private-items"

# 3. Check for TODO/FIXME/HACK comments
echo -n "Checking for TODO/FIXME/HACK comments... "
TODO_COUNT=$(grep -rn "TODO\|FIXME\|HACK" src/ tests/ 2>/dev/null | wc -l || echo "0")
if [ "$TODO_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⚠ Found $TODO_COUNT TODO/FIXME/HACK comments${NC}"
    grep -rn "TODO\|FIXME\|HACK" src/ tests/ 2>/dev/null | head -10 || true
    if [ "$TODO_COUNT" -gt 10 ]; then
        echo "... and $((TODO_COUNT - 10)) more"
    fi
    echo ""
else
    echo -e "${GREEN}✓ No TODO/FIXME/HACK comments found${NC}"
fi

# 4. Check for unwrap() usage
echo -n "Checking for unwrap() usage... "
# Count unwrap() calls excluding test blocks
UNWRAP_COUNT=0
UNWRAP_EXAMPLES=""
while IFS= read -r file; do
    # Use awk to find unwrap calls outside of test blocks
    FILE_UNWRAPS=$(awk '
    BEGIN { in_test = 0 }
    /^[[:space:]]*#\[cfg\(test\)\]/ { in_test = 1 }
    /^[[:space:]]*mod tests/ { in_test = 1 }
    in_test && /^}/ && NF == 1 { in_test = 0; next }
    !in_test && /\.unwrap\(\)/ {
        print FILENAME ":" NR ":" $0
    }
    ' "$file")

    if [ -n "$FILE_UNWRAPS" ]; then
        FILE_COUNT=$(echo "$FILE_UNWRAPS" | wc -l)
        UNWRAP_COUNT=$((UNWRAP_COUNT + FILE_COUNT))
        if [ -z "$UNWRAP_EXAMPLES" ]; then
            UNWRAP_EXAMPLES="$FILE_UNWRAPS"
        else
            UNWRAP_EXAMPLES="$UNWRAP_EXAMPLES
$FILE_UNWRAPS"
        fi
    fi
done < <(find src/ -name "*.rs" -type f)

if [ "$UNWRAP_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⚠ Found $UNWRAP_COUNT unwrap() calls in non-test code${NC}"
    echo -e "${YELLOW}Consider using expect() or proper error handling${NC}"
    echo "$UNWRAP_EXAMPLES" | head -5
    if [ "$UNWRAP_COUNT" -gt 5 ]; then
        echo "... and $((UNWRAP_COUNT - 5)) more"
    fi
    echo ""
else
    echo -e "${GREEN}✓ No unwrap() calls in non-test code${NC}"
fi

# 5. Check documentation
echo -n "Checking documentation... "
DOC_WARNINGS=$(cargo doc --no-deps 2>&1 | grep -c "warning" || true)
DOC_WARNINGS=${DOC_WARNINGS:-0}
if [ "$DOC_WARNINGS" -gt 0 ]; then
    echo -e "${YELLOW}⚠ Found $DOC_WARNINGS documentation warnings${NC}"
    cargo doc --no-deps 2>&1 | grep "warning" | head -5
    echo ""
else
    echo -e "${GREEN}✓ Documentation builds without warnings${NC}"
fi

# 6. Check for long functions (excluding test modules)
echo -n "Checking for long functions... "

# Find long functions in production code (excluding test modules)
# shellcheck disable=SC2034  # Variable unused but command writes to file via tee
LONG_FUNCS_SRC=$(find src/ -name "*.rs" -exec awk '
BEGIN { in_test = 0; func_count = 0 }
/^[[:space:]]*#\[cfg\(test\)\]/ { in_test = 1 }
/^[[:space:]]*mod tests/ { in_test = 1 }
in_test && /^}/ && NF == 1 { in_test = 0; next }
!in_test && /^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+/ {
    start=NR; fname=$0
}
!in_test && /^}/ {
    if (start && NR-start > 50) {
        func_count++
        print FILENAME ":" start ": " fname " (" NR-start " lines)"
    }
}
END { exit func_count }
' {} \; 2>/dev/null | tee /tmp/long_funcs_src.txt)

# Count functions in production code
PROD_COUNT=$(wc -l < /tmp/long_funcs_src.txt 2>/dev/null || echo "0")

# Find long functions in test code
LONG_FUNCS_TEST=$(find tests/ -name "*.rs" -exec awk '
/^[[:space:]]*(pub[[:space:]]+)?(async[[:space:]]+)?fn[[:space:]]+/ { start=NR }
/^}/ { if (NR-start > 50) print FILENAME ":" start ": Function is " NR-start " lines long" }
' {} \; 2>/dev/null | wc -l || echo "0")

if [ "$PROD_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⚠ Found $PROD_COUNT functions longer than 50 lines in production code${NC}"
    head -5 /tmp/long_funcs_src.txt
    echo ""
elif [ "$LONG_FUNCS_TEST" -gt 0 ]; then
    echo -e "${GREEN}✓ All production functions are reasonably sized${NC}"
    echo -e "  (Note: $LONG_FUNCS_TEST long test functions found, which is acceptable)"
else
    echo -e "${GREEN}✓ All functions are reasonably sized${NC}"
fi

rm -f /tmp/long_funcs_src.txt

# 7. Dependency audit (if cargo-audit is installed)
if command -v cargo-audit &> /dev/null; then
    run_lint "dependency audit" "cargo audit"
else
    echo -e "${YELLOW}Skipping dependency audit (cargo-audit not installed)${NC}"
fi

echo ""
echo "Summary"
echo "-------"

if [ "$LINT_FAILED" -eq 0 ]; then
    echo -e "${GREEN}✓ All linting checks passed!${NC}"
    echo ""
    echo "Your code follows modern Rust best practices and functional programming style."
else
    echo -e "${RED}✗ Some linting checks failed!${NC}"
    echo ""
    echo "Please fix the issues above before committing."
    exit 1
fi

echo ""
echo "Tips:"
echo "- Run 'cargo fmt' to automatically fix formatting issues"
echo "- Run 'cargo clippy --fix' to automatically fix some clippy warnings"
echo "- Run 'cargo doc --open' to view your documentation"
