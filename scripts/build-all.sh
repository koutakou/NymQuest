#!/bin/bash

# Build script for NymQuest - builds both server and client for all supported platforms

set -e

echo "🚀 Building NymQuest for all supported platforms..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Supported targets
TARGETS=(
    "x86_64-unknown-linux-gnu"
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
)

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust and Cargo."
    exit 1
fi

print_status "Running quality checks..."

# Format check
print_status "Checking code formatting..."
cd server && cargo fmt --check
cd ../client && cargo fmt --check
cd ..
print_success "Code formatting check passed"

# Clippy check
print_status "Running clippy..."
cd server && cargo clippy -- -D warnings
cd ../client && cargo clippy -- -D warnings
cd ..
print_success "Clippy check passed"

# Tests
print_status "Running tests..."
cd server && cargo test
cd ../client && cargo test
cd ..
print_success "All tests passed"

# Install targets if not already installed
for target in "${TARGETS[@]}"; do
    print_status "Ensuring target $target is installed..."
    rustup target add "$target" || print_warning "Could not install target $target"
done

# Create release directory
rm -rf release
mkdir -p release

# Build for each target
for target in "${TARGETS[@]}"; do
    print_status "Building for $target..."
    
    # Determine the platform name for the release
    case "$target" in
        "x86_64-unknown-linux-gnu")
            platform="linux-x86_64"
            ;;
        "x86_64-pc-windows-msvc")
            platform="windows-x86_64"
            ;;
        "x86_64-apple-darwin")
            platform="macos-x86_64"
            ;;
        "aarch64-apple-darwin")
            platform="macos-aarch64"
            ;;
        *)
            platform="unknown"
            ;;
    esac
    
    # Create platform directory
    mkdir -p "release/$platform"
    
    # Build server
    print_status "Building server for $target..."
    cd server
    if cargo build --release --target "$target"; then
        print_success "Server built successfully for $target"
    else
        print_error "Failed to build server for $target"
        cd ..
        continue
    fi
    cd ..
    
    # Build client
    print_status "Building client for $target..."
    cd client
    if cargo build --release --target "$target"; then
        print_success "Client built successfully for $target"
    else
        print_error "Failed to build client for $target"
        cd ..
        continue
    fi
    cd ..
    
    # Copy binaries to release directory
    if [[ "$target" == *"windows"* ]]; then
        cp "server/target/$target/release/nym-mmorpg-server.exe" "release/$platform/"
        cp "client/target/$target/release/nym-mmorpg-client.exe" "release/$platform/"
    else
        cp "server/target/$target/release/nym-mmorpg-server" "release/$platform/"
        cp "client/target/$target/release/nym-mmorpg-client" "release/$platform/"
    fi
    
    # Copy documentation
    cp README.md "release/$platform/"
    cp LICENSE "release/$platform/"
    cp BUILD.md "release/$platform/"
    
    print_success "Binaries copied to release/$platform/"
done

# Create archives
print_status "Creating release archives..."
cd release

for dir in */; do
    dir=${dir%/}  # Remove trailing slash
    
    if [[ "$dir" == *"windows"* ]]; then
        # Create zip for Windows
        print_status "Creating archive for $dir..."
        zip -r "../nymquest-$dir.zip" "$dir"
        print_success "Created nymquest-$dir.zip"
    else
        # Create tar.gz for Unix-like systems
        print_status "Creating archive for $dir..."
        tar -czf "../nymquest-$dir.tar.gz" "$dir"
        print_success "Created nymquest-$dir.tar.gz"
    fi
done

cd ..

print_success "🎉 Build completed successfully!"
print_status "Release archives created:"
ls -la nymquest-*.{tar.gz,zip} 2>/dev/null || true

print_status "To test the builds locally, extract any archive and run the binaries."
print_status "For automated releases, push a git tag like 'v0.1.0' to trigger GitHub Actions."
