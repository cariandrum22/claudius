#!/usr/bin/env bash
# Mock 1Password CLI for testing
# This script simulates the `op` command behavior

if [[ "$1" == "--version" ]]; then
    echo "2.20.0"
    exit 0
fi

if [[ "$1" == "read" && "$2" == "op://vault/test-item/api-key" ]]; then
    echo "secret-api-key-12345"
    exit 0
fi

if [[ "$1" == "read" && "$2" == "op://vault/database/password" ]]; then
    echo "db-password-xyz789"
    exit 0
fi

if [[ "$1" == "read" && "$2" == "op://invalid/reference/field" ]]; then
    echo "ERROR: Item not found" >&2
    exit 1
fi

# Default error
echo "ERROR: Unknown command or reference" >&2
exit 1
