#!/usr/bin/env bash

# Demonstration of DAG-based variable expansion in Claudius

echo "=== Claudius Variable Expansion Demo ==="
echo

# Example 1: Simple nested variable reference
echo "Example 1: Nested variable reference"
echo "Setting up environment variables:"
echo "  CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID=12345"
echo "  CLAUDIUS_SECRET_ANTHROPIC_BASE_URL=https://claudehub.api.cloudflare.com/\$CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID/v1"
echo

export CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID="12345"
# shellcheck disable=SC2016  # We want literal $CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID for claudius to expand
export CLAUDIUS_SECRET_ANTHROPIC_BASE_URL='https://claudehub.api.cloudflare.com/$CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID/v1'

echo "Running: claudius run -- sh -c 'echo \$ANTHROPIC_BASE_URL'"
# shellcheck disable=SC2016  # Variables will be available in the subshell environment
../target/release/claudius run -- sh -c 'echo "ANTHROPIC_BASE_URL=$ANTHROPIC_BASE_URL"'
echo

# Example 2: Multiple levels of nesting
echo "Example 2: Multiple levels of variable references"
echo "Setting up environment variables:"
echo "  CLAUDIUS_SECRET_HOST=api.example.com"
echo "  CLAUDIUS_SECRET_PORT=8443"
echo "  CLAUDIUS_SECRET_PROTOCOL=https"
echo "  CLAUDIUS_SECRET_SERVER_URL=\$CLAUDIUS_SECRET_PROTOCOL://\$CLAUDIUS_SECRET_HOST:\$CLAUDIUS_SECRET_PORT"
echo "  CLAUDIUS_SECRET_API_ENDPOINT=\$CLAUDIUS_SECRET_SERVER_URL/v2/production"
echo

export CLAUDIUS_SECRET_HOST="api.example.com"
export CLAUDIUS_SECRET_PORT="8443"
export CLAUDIUS_SECRET_PROTOCOL="https"
# shellcheck disable=SC2016  # We want literal variables for claudius to expand
export CLAUDIUS_SECRET_SERVER_URL='$CLAUDIUS_SECRET_PROTOCOL://$CLAUDIUS_SECRET_HOST:$CLAUDIUS_SECRET_PORT'
# shellcheck disable=SC2016  # We want literal variables for claudius to expand
export CLAUDIUS_SECRET_API_ENDPOINT='$CLAUDIUS_SECRET_SERVER_URL/v2/production'

echo "Running: claudius run -- sh -c 'echo \$API_ENDPOINT'"
# shellcheck disable=SC2016  # Variables will be available in the subshell environment
../target/release/claudius run -- sh -c 'echo "API_ENDPOINT=$API_ENDPOINT"'
echo

# Cleanup
unset CLAUDIUS_SECRET_CF_AIG_ACCOUNT_ID
unset CLAUDIUS_SECRET_ANTHROPIC_BASE_URL
unset CLAUDIUS_SECRET_HOST
unset CLAUDIUS_SECRET_PORT
unset CLAUDIUS_SECRET_PROTOCOL
unset CLAUDIUS_SECRET_SERVER_URL
unset CLAUDIUS_SECRET_API_ENDPOINT

echo "=== Demo Complete ==="
