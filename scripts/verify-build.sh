#!/bin/bash

# Quick verification script for NymQuest builds

set -e

echo "🔍 Verifying NymQuest build..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust and Cargo."
    exit 1
fi

# Format check
print_status "Checking code formatting..."
cd server
if cargo fmt --check; then
    print_success "Server formatting is correct"
else
    print_error "Server formatting issues found"
    exit 1
fi

cd ../client
if cargo fmt --check; then
    print_success "Client formatting is correct"
else
    print_error "Client formatting issues found"
    exit 1
fi
cd ..

# Clippy check
print_status "Running clippy checks..."
cd server
if cargo clippy -- -D warnings; then
    print_success "Server clippy check passed"
else
    print_error "Server clippy check failed"
    exit 1
fi

cd ../client
if cargo clippy -- -D warnings; then
    print_success "Client clippy check passed"
else
    print_error "Client clippy check failed"
    exit 1
fi
cd ..

# Build check
print_status "Building projects..."
cd server
if cargo build; then
    print_success "Server builds successfully"
else
    print_error "Server build failed"
    exit 1
fi

cd ../client
if cargo build; then
    print_success "Client builds successfully"
else
    print_error "Client build failed"
    exit 1
fi
cd ..

# Test check
print_status "Running tests..."
cd server
if cargo test; then
    print_success "Server tests passed"
else
    print_error "Server tests failed"
    exit 1
fi

cd ../client
if cargo test; then
    print_success "Client tests passed"
else
    print_error "Client tests failed"
    exit 1
fi
cd ..

print_success "✅ All verification checks passed!"
print_status "The project is ready for release."
