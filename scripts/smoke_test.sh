#!/usr/bin/env bash
set -euo pipefail

# Smoke test for pi-peline
#
# Runs a simple happy-path pipeline to verify the application works.
#
# Usage:
#   ./scripts/smoke_test.sh          # Build and run test
#   ./scripts/smoke_test.sh --no-build  # Run test without rebuilding

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
PIPELINE_BIN="$PROJECT_ROOT/target/debug/pipeline"

# Build by default
BUILD_FLAG=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --no-build)
            BUILD_FLAG="skip"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Build
if [ "$BUILD_FLAG" != "skip" ]; then
    echo "Building pi-peline..."
    cd "$PROJECT_ROOT"
    cargo build
fi

# Check if binary exists
if [ ! -f "$PIPELINE_BIN" ]; then
    echo "Error: Binary not found at $PIPELINE_BIN"
    exit 1
fi

# Check if pi CLI is available
if ! command -v pi &> /dev/null; then
    echo "Error: pi CLI not found in PATH"
    exit 1
fi

# Run simple pipeline
echo "Running smoke test..."
echo ""

$PIPELINE_BIN run -f "$PROJECT_ROOT/examples/simple.yaml" --no-history

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Smoke test passed"
else
    echo ""
    echo "❌ Smoke test failed"
    exit 1
fi
