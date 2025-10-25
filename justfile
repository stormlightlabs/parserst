# Run all tests
test:
    cargo test

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

# Build release
build-release:
    cargo build --release

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
