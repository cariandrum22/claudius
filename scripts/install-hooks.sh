#!/usr/bin/env bash
# Install git hooks for the project

set -e

echo "Installing Git hooks for Claudius..."
echo "===================================="
echo ""

# Get the project root
PROJECT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
HOOKS_DIR="$PROJECT_ROOT/.githooks"
GIT_HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

# Check if we're in a git repository
if [ ! -d "$PROJECT_ROOT/.git" ]; then
    echo "Error: Not in a git repository"
    exit 1
fi

# Check if hooks directory exists
if [ ! -d "$HOOKS_DIR" ]; then
    echo "Error: Hooks directory not found at $HOOKS_DIR"
    exit 1
fi

# Install hooks
echo "Installing hooks..."
for hook in "$HOOKS_DIR"/*; do
    if [ -f "$hook" ]; then
        hook_name=$(basename "$hook")
        target="$GIT_HOOKS_DIR/$hook_name"

        # Check if hook already exists
        if [ -f "$target" ] || [ -L "$target" ]; then
            echo -n "Hook '$hook_name' already exists. Overwrite? [y/N] "
            read -r response
            if [[ ! "$response" =~ ^[Yy]$ ]]; then
                echo "Skipping $hook_name"
                continue
            fi
            rm -f "$target"
        fi

        # Create symlink
        ln -s "../../.githooks/$hook_name" "$target"
        echo "✓ Installed $hook_name"
    fi
done

echo ""
echo "Git hooks installed successfully!"
echo ""
echo "Hooks will run automatically on git operations."
echo "To skip pre-commit tests temporarily, use: SKIP_TESTS=1 git commit"
echo ""
echo "To uninstall hooks, run:"
echo "  rm $GIT_HOOKS_DIR/pre-commit"
