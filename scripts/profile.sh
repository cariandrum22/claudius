#!/usr/bin/env bash
# Script to run claudius with profiling enabled

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Claudius Profiling Tool${NC}"
echo "========================="
echo ""

# Parse arguments
BUILD_TYPE="debug"
PROFILE_FLAMEGRAPH=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --release)
            BUILD_TYPE="release"
            shift
            ;;
        --flamegraph)
            PROFILE_FLAMEGRAPH=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [options] -- <claudius args>"
            echo ""
            echo "Options:"
            echo "  --release      Build in release mode (recommended for profiling)"
            echo "  --flamegraph   Enable flamegraph generation (requires profiling feature)"
            echo "  -h, --help     Show this help message"
            echo ""
            echo "Environment variables:"
            echo "  CLAUDIUS_PROFILE=1  Enable detailed timing output"
            echo "  RUST_LOG=debug      Enable debug logging"
            echo ""
            echo "Examples:"
            echo "  # Profile the run command with timing details"
            echo "  $0 -- run -- echo hello"
            echo ""
            echo "  # Profile with release build and debug logging"
            echo "  $0 --release -- run -- npm start"
            echo ""
            echo "  # Generate flamegraph (requires profiling feature)"
            echo "  $0 --release --flamegraph -- run -- ./slow-command.sh"
            exit 0
            ;;
        --)
            shift
            break
            ;;
        *)
            break
            ;;
    esac
done

# Build the project
echo -e "${YELLOW}Building claudius in $BUILD_TYPE mode...${NC}"

if [ "$PROFILE_FLAMEGRAPH" = true ]; then
    echo -e "${BLUE}Building with flamegraph support...${NC}"
    cargo build --profile=profiling --features=profiling
    BINARY="$PROJECT_ROOT/target/profiling/claudius"
else
    if [ "$BUILD_TYPE" = "release" ]; then
        cargo build --release
        BINARY="$PROJECT_ROOT/target/release/claudius"
    else
        cargo build
        BINARY="$PROJECT_ROOT/target/debug/claudius"
    fi
fi

echo -e "${GREEN}Build complete!${NC}"
echo ""

# Set profiling environment variables
export CLAUDIUS_PROFILE=1
export RUST_LOG="${RUST_LOG:-info}"

echo -e "${YELLOW}Running with profiling enabled...${NC}"
echo -e "  CLAUDIUS_PROFILE=$CLAUDIUS_PROFILE"
echo -e "  RUST_LOG=$RUST_LOG"
echo ""

# Run the command
echo -e "${BLUE}Executing: $BINARY $*${NC}"
echo "================================="
echo ""

# Capture start time
START_TIME=$(date +%s)

# Run claudius with all arguments
"$BINARY" "$@"

# Capture end time
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo ""
echo "================================="
echo -e "${GREEN}Total execution time: ${DURATION}s${NC}"

# Check for flamegraph files
if [ "$PROFILE_FLAMEGRAPH" = true ]; then
    echo ""
    echo -e "${YELLOW}Checking for generated flamegraphs...${NC}"
    FLAMEGRAPHS=$(find . -name "flamegraph-*.svg" -mmin -1 2>/dev/null || true)
    if [ -n "$FLAMEGRAPHS" ]; then
        echo -e "${GREEN}Generated flamegraphs:${NC}"
        echo "$FLAMEGRAPHS"
    else
        echo -e "${RED}No flamegraphs generated. Make sure the code uses profile_flamegraph().${NC}"
    fi
fi

echo ""
echo -e "${BLUE}Profiling complete!${NC}"
