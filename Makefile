# Makefile for git-perf development tasks

.PHONY: help manpage generate-manpage validate-manpage test-manpage clean-manpage

help: ## Show this help message
	@echo "Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

manpage: generate-manpage ## Alias for generate-manpage

generate-manpage: ## Generate docs/manpage.md to match CI expectations
	@echo "🔧 Generating manpage.md..."
	@./scripts/generate-manpage-standardized.sh

validate-manpage: ## Validate that docs/manpage.md matches CI expectations
	@echo "🔍 Validating manpage.md..."
	@./scripts/validate-manpage-standardized.sh

test-manpage: ## Test manpage generation and validation
	@echo "🧪 Testing manpage generation..."
	@./scripts/generate-manpage-standardized.sh
	@./scripts/validate-manpage-standardized.sh
	@echo "✅ All manpage tests passed!"

clean-manpage: ## Clean generated manpage files
	@echo "🧹 Cleaning manpage files..."
	@rm -rf target/man/
	@echo "✅ Cleaned manpage files"

# Development targets
build: ## Build the project
	cargo build

test: ## Run tests
	cargo test

check: ## Run all checks (format, clippy, tests, manpage validation)
	cargo fmt --all --check
	cargo clippy -- -D warnings
	cargo test
	./scripts/validate-manpage-standardized.sh

install-deps: ## Install development dependencies
	@echo "📦 Installing development dependencies..."
	@if command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update && sudo apt-get install -y pandoc libfaketime; \
	elif command -v brew >/dev/null 2>&1; then \
		brew install pandoc libfaketime; \
	else \
		echo "❌ Package manager not found. Please install pandoc and libfaketime manually."; \
		exit 1; \
	fi