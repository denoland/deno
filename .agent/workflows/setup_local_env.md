---
description: Setup local development environment for Deno
---

# Setup Local Deno Development Environment

This guide helps you set up your system to build and contribute to Deno.

## Option 1: VS Code Dev Containers (Recommended)

If you use VS Code, the easiest way is to use the provided Dev Container
configuration.

1. Install [Docker Desktop](https://www.docker.com/products/docker-desktop).
2. Install the
   [Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
   in VS Code.
3. Open the project folder in VS Code.
4. Click "Reopen in Container" when prompted, or run the command
   `Dev Containers: Reopen in Container`.

This will automatically set up all dependencies (Rust, Python, CMake, Protobuf,
etc.).

## Option 2: Manual Setup

### 1. Install Prerequisites

#### Rust

Deno requires a specific version of Rust.

```bash
# Install rustup if you haven't
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the specific version required by Deno (currently 1.90.0)
rustup install 1.90.0
rustup default 1.90.0
rustup component add rustfmt clippy
```

#### Python 3

Ensure you have Python 3 installed and accessible as `python` or `python3`.

#### Protobuf Compiler

- **Mac:** `brew install protobuf`
- **Linux:** `apt install -y protobuf-compiler`
- **Windows:** Download binary release from GitHub.

#### CMake

- **Mac:** `brew install cmake`
- **Linux:** `apt install -y cmake`

#### Native Compilers

- **Mac:** XCode Command Line Tools (`xcode-select --install`)
- **Linux:** `apt install -y build-essential libglib2.0-dev`

### 2. Build Deno

```bash
# Clone with submodules if you haven't already
git submodule update --init --recursive

# Build
cargo build -vv
```

### 3. Verify Setup

Run the tests to ensure everything is working:

```bash
# Run unit tests
cargo test -vv

# Format code
./tools/format.js

# Lint code
./tools/lint.js
```
