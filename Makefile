.PHONY: all demo mesh-demo demo-flow dev-check clean doc test bench udeps

all: dev-check test

# Run the federation demo
demo: demo-flow

# Run the mesh compute demo
mesh-demo:
	@echo "Running ICN mesh compute demo..."
	./run_mesh_demo.sh

# Run the standard demo flow
demo-flow:
	@echo "Running ICN demo flow..."
	./run_demo_flow.sh

# Development checks
dev-check: check fmt clippy udeps

# Check compilation
check:
	@echo "Running cargo check..."
	cargo check --workspace

# Format code
fmt:
	@echo "Running cargo fmt..."
	cargo fmt --all -- --check

# Run clippy lints
clippy:
	@echo "Running cargo clippy..."
	cargo clippy --workspace -- -D warnings

# Check for unused dependencies (requires cargo-udeps)
udeps:
	@echo "Checking for unused dependencies (cargo-udeps)..."
	@if command -v cargo-udeps >/dev/null 2>&1; then \
		cargo udeps --workspace --all-targets --backend=cargo-metadata; \
	else \
		echo "cargo-udeps is not installed. Run: cargo install cargo-udeps --locked"; \
		#(exit 1); # Optional: remove exit 1 to make it a soft check if not installed
	fi

# Run tests
test:
	@echo "Running tests..."
	cargo test --workspace

# Run benchmarks (requires criterion setup)
bench:
	@echo "Running benchmarks..."
	cargo bench

# Generate documentation
doc:
	@echo "Generating documentation..."
	cargo doc --workspace --no-deps --all-features

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Run doctor command to check environment
doctor:
	@echo "Running ICN doctor..."
	cargo run -p icn-cli -- doctor

# Run security audit
audit:
	@echo "Running security audit..."
	@if command -v cargo-audit >/dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "cargo-audit is not installed. Install with: cargo install cargo-audit"; \
		exit 1; \
	fi

# Add custom tasks below this line
# --------------------------------
# To add a new task:
# 1. Define the task name as a .PHONY target
# 2. Add the task implementation
# 3. Document the task in README.md
# 
# Example:
# custom-task:
#   @echo "Running custom task..."
#   ./scripts/custom_task.sh 