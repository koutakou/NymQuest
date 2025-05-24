# Building NymQuest

This document describes how to build NymQuest for different platforms and create releases.

## Prerequisites

- Rust and Cargo (latest stable version)
- Git

## Building Locally

### Server
```bash
cd server
cargo build --release
```

### Client
```bash
cd client  
cargo build --release
```

## Cross-Platform Building

### Linux (x86_64)
```bash
rustup target add x86_64-unknown-linux-gnu
cd server && cargo build --release --target x86_64-unknown-linux-gnu
cd ../client && cargo build --release --target x86_64-unknown-linux-gnu
```

### Windows (x86_64)
```bash
rustup target add x86_64-pc-windows-msvc
cd server && cargo build --release --target x86_64-pc-windows-msvc
cd ../client && cargo build --release --target x86_64-pc-windows-msvc
```

### macOS (Intel)
```bash
rustup target add x86_64-apple-darwin
cd server && cargo build --release --target x86_64-apple-darwin
cd ../client && cargo build --release --target x86_64-apple-darwin
```

### macOS (Apple Silicon)
```bash
rustup target add aarch64-apple-darwin
cd server && cargo build --release --target aarch64-apple-darwin
cd ../client && cargo build --release --target aarch64-apple-darwin
```

## Creating Releases

### Automated Releases (GitHub Actions)

1. Tag a new version:
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

2. GitHub Actions will automatically:
   - Build for all supported platforms (Linux x86_64, Windows x86_64, macOS x86_64, macOS ARM64)
   - Create release archives
   - Upload to GitHub Releases

### Manual Release Creation

For manual releases, follow these steps:

1. Build for all target platforms (see cross-platform building above)
2. Create release directories:
   ```bash
   mkdir -p release/linux-x86_64
   mkdir -p release/windows-x86_64
   mkdir -p release/macos-x86_64
   mkdir -p release/macos-aarch64
   ```

3. Copy binaries to appropriate directories
4. Include README.md and LICENSE in each release
5. Create archives:
   ```bash
   # For Unix-like systems
   tar -czf nymquest-linux-x86_64.tar.gz -C release/linux-x86_64 .
   tar -czf nymquest-macos-x86_64.tar.gz -C release/macos-x86_64 .
   tar -czf nymquest-macos-aarch64.tar.gz -C release/macos-aarch64 .
   
   # For Windows
   cd release/windows-x86_64 && zip -r ../../nymquest-windows-x86_64.zip .
   ```

## Quality Assurance

Before creating releases, always run:

```bash
# Quick verification (recommended)
./scripts/verify-build.sh

# Or manually run individual checks:
# Format check
cd server && cargo fmt --check
cd ../client && cargo fmt --check

# Linting
cd server && cargo clippy -- -D warnings
cd ../client && cargo clippy -- -D warnings

# Tests
cd server && cargo test
cd ../client && cargo test

# Build verification
cd server && cargo build --release
cd ../client && cargo build --release
```

## Automated Build Scripts

### Quick Verification
```bash
./scripts/verify-build.sh
```
Runs all quality checks (formatting, clippy, build, tests) to ensure the project is ready for release.

### Full Multi-Platform Build
```bash
./scripts/build-all.sh
```
Comprehensive script that:
- Runs all quality checks
- Builds for all supported platforms
- Creates release archives
- Provides colored output for easy monitoring

## Supported Platforms

- **Linux**: x86_64-unknown-linux-gnu
- **Windows**: x86_64-pc-windows-msvc
- **macOS**: x86_64-apple-darwin (Intel), aarch64-apple-darwin (Apple Silicon)

## Binary Names

- **Server**: `nym-mmorpg-server` (Unix) / `nym-mmorpg-server.exe` (Windows)
- **Client**: `nym-mmorpg-client` (Unix) / `nym-mmorpg-client.exe` (Windows)
