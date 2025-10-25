# Run all tests
test:
    cargo test

# Run tests without markdown feature
test-no-markdown:
    cargo test --no-default-features

# Run tests with markdown feature
test-markdown:
    cargo test --features markdown

# Run tests with all features
test-all:
    cargo test --all-features

# Run tests with coverage report
coverage:
    cargo llvm-cov --all-features --html

# Run tests with lcov output for codecov
coverage-lcov:
    cargo llvm-cov --all-features --lcov --output-path lcov.info

# Format code
fmt:
    cargo fmt --all

# Lint code
lint:
    cargo clippy --all-targets --all-features -- -D warnings

# Build all packages
build:
    cargo build

# Build with all features
build-all:
    cargo build --all-features

# Build without markdown feature
build-no-markdown:
    cargo build --no-default-features

# Build with markdown feature only
build-markdown:
    cargo build --no-default-features --features markdown

# Build release
build-release:
    cargo build --release

# Build release with all features
build-release-all:
    cargo build --release --all-features

# Clean build artifacts
clean:
    cargo clean

# Install cargo-llvm-cov for coverage
install-coverage:
    cargo install cargo-llvm-cov

# Check coverage percentage
coverage-check:
    cargo llvm-cov --all-features --summary-only

# Generate coverage in JSON format
coverage-json:
    cargo llvm-cov --all-features --json --output-path coverage.json

# Generate coverage in XML format
coverage-xml:
    cargo llvm-cov --all-features --cobertura --output-path coverage.xml
