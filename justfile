# Claudius Build and Development Commands
# https://github.com/casey/just

# Default recipe - show help
default:
    @just --list

# Build the project in release mode
build:
    cargo build --release

# Run all tests with limited parallelism to avoid race conditions
test:
    CLAUDIUS_TEST_MOCK_OP=1 RUST_TEST_THREADS=4 cargo test

# Run tests with statistics
test-stats:
    ./scripts/test-stats.sh

# Clean build artifacts and coverage files
clean:
    cargo clean
    rm -f lcov.info coverage.json cobertura.xml
    rm -rf target/llvm-cov

# Format all code using rustfmt
fmt:
    ./scripts/fmt.sh

# Alias for fmt
format: fmt

# Run all linting checks (clippy, format check, etc.)
lint:
    ./scripts/lint.sh

# Install git pre-commit hooks
install-hooks:
    ./scripts/install-hooks.sh

# Run format, lint, and tests
check: fmt lint test

# Install claudius locally
install: build
    cargo install --path .

# Run full coverage analysis with all formats (requires Rust 1.81+)
coverage:
    ./scripts/coverage.sh

# Generate HTML coverage report only
coverage-html:
    CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov --all-features --workspace --html
    @echo "HTML report available at: target/llvm-cov/html/index.html"

# Generate LCOV coverage report only
coverage-lcov:
    CLAUDIUS_TEST_MOCK_OP=1 cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
    @echo "LCOV report available at: lcov.info"

# Run detailed coverage with options
coverage-detailed *ARGS:
    ./scripts/coverage-detailed.sh {{ ARGS }}

# Run the development version
run *ARGS:
    cargo run -- {{ ARGS }}

# Watch for changes and run tests
watch:
    CLAUDIUS_TEST_MOCK_OP=1 cargo watch -x test

# Run clippy with automatic fixes
fix:
    cargo clippy --fix --allow-dirty --allow-staged
    cargo fmt

# Update dependencies
update:
    cargo update

# Check for outdated dependencies
outdated:
    cargo outdated

# Run security audit
audit:
    cargo audit

# Run coverage with cargo-tarpaulin
coverage-tarpaulin:
    CLAUDIUS_TEST_MOCK_OP=1 cargo tarpaulin --out Html --out Lcov --out Json

# Run mutation testing with cargo-mutants
mutation-test:
    cargo mutants --timeout 30

# Check for unused dependencies
check-deps:
    cargo machete

# Build documentation
doc:
    cargo doc --no-deps --open

# Initialize Claudius configuration
init:
    cargo run -- init

# Sync configurations
sync *ARGS:
    cargo run -- sync {{ ARGS }}

# Quick development test
dev-test:
    #!/usr/bin/env bash
    set -e
    echo "Running quick development test..."
    cargo check
    cargo clippy -- -D warnings
    CLAUDIUS_TEST_MOCK_OP=1 cargo test --lib

# Run benchmarks (if any)
bench:
    cargo bench

# Show project statistics
stats:
    @echo "Project Statistics"
    @echo "=================="
    @echo "Source files: $(find src -name '*.rs' | wc -l)"
    @echo "Test files:   $(find tests -name '*.rs' | wc -l)"
    @echo "Total LoC:    $(find src tests -name '*.rs' -exec cat {} \; | wc -l)"
    @echo ""
    tokei

# Create a new release
release version:
    #!/usr/bin/env bash
    set -e
    echo "Creating release {{ version }}..."
    # Update version in Cargo.toml
    sed -i 's/^version = ".*"/version = "{{ version }}"/' Cargo.toml
    # Commit changes
    git add Cargo.toml
    git commit -m "Release v{{ version }}"
    # Create tag
    git tag -a "v{{ version }}" -m "Release v{{ version }}"
    echo "Release created. Don't forget to push: git push && git push --tags"

# Run with verbose logging
verbose *ARGS:
    RUST_LOG=debug cargo run -- {{ ARGS }}

# Run with trace logging
trace *ARGS:
    RUST_LOG=trace cargo run -- {{ ARGS }}

# Profile the application with timing details
profile *ARGS:
    ./scripts/profile.sh {{ ARGS }}

# Profile with release build
profile-release *ARGS:
    ./scripts/profile.sh --release {{ ARGS }}

# Build with profiling features
build-profiling:
    cargo build --profile=profiling --features=profiling
